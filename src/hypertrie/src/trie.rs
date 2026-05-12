use crate::bloom_filter::BloomFilter;

const ALPHABET_SIZE: usize = 26;

const CHAR_TO_BIT: [i8; 256] = {
    let mut table = [-1i8; 256];
    let mut i = 0;
    while i < 26 {
        table[(b'a' + i) as usize] = i as i8;
        table[(b'A' + i) as usize] = i as i8;
        i += 1;
    }
    table
};

pub struct Trie {
    masks: Vec<u32>,
    indices: Vec<u32>, // Flat array of [u32; 26] for each node
    bloom_filter: BloomFilter,
}

impl Trie {
    pub fn new(size: usize, num_hashes: usize) -> Self {
        let optimized_size = size
            .checked_next_power_of_two()
            .expect("Next power of 2 usize overflow");

        // Estimate nodes: each word adds a few nodes, but many are shared.
        // For a dictionary, it's roughly 2-3x number of words.
        let node_estimate = size * 3 + 1024;
        let mut masks = Vec::with_capacity(node_estimate);
        let mut indices = Vec::with_capacity(node_estimate * ALPHABET_SIZE);

        // Root node
        masks.push(0);
        indices.extend_from_slice(&[0u32; ALPHABET_SIZE]);

        Trie {
            masks,
            indices,
            bloom_filter: BloomFilter::new(optimized_size, num_hashes),
        }
    }

    pub fn insert(&mut self, word: &str) {
        let mut current_idx = 0;
        let bytes = word.as_bytes();

        for &b in bytes {
            let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) };
            if bit_idx < 0 {
                continue;
            }
            let bit_idx = bit_idx as usize;

            let mask = unsafe { *self.masks.get_unchecked(current_idx) };
            if (mask & (1 << bit_idx)) == 0 {
                let new_node_idx = self.masks.len() as u32;
                self.masks.push(0);
                self.indices.extend_from_slice(&[0u32; ALPHABET_SIZE]);

                let mask_ref = unsafe { self.masks.get_unchecked_mut(current_idx) };
                *mask_ref |= 1 << bit_idx;
                let index_ref = unsafe {
                    self.indices
                        .get_unchecked_mut(current_idx * ALPHABET_SIZE + bit_idx)
                };
                *index_ref = new_node_idx;

                current_idx = new_node_idx as usize;
            } else {
                current_idx = unsafe {
                    *self
                        .indices
                        .get_unchecked(current_idx * ALPHABET_SIZE + bit_idx)
                        as usize
                };
            }
        }

        unsafe { *self.masks.get_unchecked_mut(current_idx) |= 1 << 31 };
        self.bloom_filter.insert(word);
    }

    #[inline(always)]
    pub fn contains(&self, word: &str) -> bool {
        // Bloom Filter is usually faster than a full Trie walk for non-members
        if !self.bloom_filter.contains(word) {
            return false;
        }

        let bytes = word.as_bytes();
        let mut current_idx = 0;
        for i in 0..bytes.len() {
            let b = unsafe { *bytes.get_unchecked(i) };
            let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) };
            if bit_idx < 0 {
                return false;
            }
            let bit_idx = bit_idx as usize;

            let mask = unsafe { *self.masks.get_unchecked(current_idx) };
            if (mask & (1 << bit_idx)) == 0 {
                return false;
            }
            current_idx = unsafe {
                *self
                    .indices
                    .get_unchecked(current_idx * ALPHABET_SIZE + bit_idx) as usize
            };
        }

        unsafe { (*self.masks.get_unchecked(current_idx) & (1 << 31)) != 0 }
    }

    pub fn print(&self) {
        // Start at index 0 (the root)
        self.debug_print(0, 0);
    }

    fn debug_print(&self, node_idx: usize, indent: usize) {
        let mask = self.masks[node_idx];
        let padding = "  ".repeat(indent);

        if node_idx == 0 {
            println!("Root");
        } else {
            println!(
                "{}Node index: {} (end_of_word: {})",
                padding,
                node_idx,
                (mask & (1 << 31)) != 0
            );
        }

        // Since we are using a bitmask and an index array, we iterate
        // through the alphabet and check the mask.
        for i in 0..26 {
            if (mask & (1 << i)) != 0 {
                let child_idx = self.indices[node_idx * ALPHABET_SIZE + i] as usize;
                self.debug_print(child_idx, indent + 1);
            }
        }
    }

    pub fn words_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut current_idx = 0; // Start at root
        let bytes = prefix.as_bytes();

        // 1. Navigate to the end of the prefix
        for &b in bytes {
            let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) };
            if bit_idx < 0 {
                return Vec::new();
            }
            let bit_idx = bit_idx as usize;

            let mask = unsafe { *self.masks.get_unchecked(current_idx) };
            // Use the bitmask to check if the path exists
            if (mask & (1 << bit_idx)) == 0 {
                return Vec::new(); // Prefix not found
            }
            current_idx = unsafe {
                *self
                    .indices
                    .get_unchecked(current_idx * ALPHABET_SIZE + bit_idx) as usize
            };
        }

        // 2. Collect all words starting from this node
        let mut results = Vec::new();
        // Pre-allocate the buffer with the prefix to avoid mid-search reallocations
        let mut buffer = prefix.to_ascii_lowercase().into_bytes();

        self.collect_words_from_node(current_idx, &mut buffer, &mut results);
        results
    }

    fn collect_words_from_node(
        &self,
        node_idx: usize,
        buffer: &mut Vec<u8>,
        results: &mut Vec<String>,
    ) {
        let mask = unsafe { *self.masks.get_unchecked(node_idx) };

        // If this node marks the end of a word, save the current buffer
        if (mask & (1 << 31)) != 0 {
            results.push(unsafe { std::str::from_utf8_unchecked(buffer) }.to_owned());
        }

        // Iterate through all possible children (a-z)
        for i in 0..26 {
            // Only recurse if the bitmask says a child exists
            if (mask & (1 << i)) != 0 {
                let child_idx =
                    unsafe { *self.indices.get_unchecked(node_idx * ALPHABET_SIZE + i) as usize };

                // Push the character for this branch
                buffer.push(b'a' + i as u8);
                self.collect_words_from_node(child_idx, buffer, results);
                buffer.pop(); // Backtrack for the next branch
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trie_insert_and_contains() {
        let mut trie = Trie::new(100, 3);
        trie.insert("hello");
        trie.insert("world");

        assert!(trie.contains("hello"));
        assert!(trie.contains("world"));
        assert!(!trie.contains("hell"));
        assert!(!trie.contains("word"));
    }

    #[test]
    fn test_words_with_prefix() {
        let mut trie = Trie::new(100, 3);
        trie.insert("apple");
        trie.insert("app");
        trie.insert("application");
        trie.insert("banana");

        let apps = trie.words_with_prefix("app");
        assert_eq!(apps.len(), 3);
        assert!(apps.contains(&"apple".to_string()));
        assert!(apps.contains(&"app".to_string()));
        assert!(apps.contains(&"application".to_string()));

        let banas = trie.words_with_prefix("ban");
        assert_eq!(banas.len(), 1);
        assert!(banas.contains(&"banana".to_string()));

        let unknowns = trie.words_with_prefix("unknown");
        assert!(unknowns.is_empty());
    }
}
