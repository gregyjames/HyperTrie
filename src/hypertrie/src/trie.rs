use crate::bloom_filter::BloomFilter;

const NODE_WIDTH: usize = 32; // Power of two for alignment and faster indexing

const CHAR_TO_MASK: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 26 {
        table[(b'a' + i) as usize] = 1 << i;
        table[(b'A' + i) as usize] = 1 << i;
        i += 1;
    }
    table
};

pub struct Trie {
    // Flat array: each node occupies 32 u32 slots.
    // [0]: mask (bits 0-25 for children, bit 31 for end-of-word)
    // [1..27]: child indices
    // [27..31]: unused
    nodes: Vec<u32>,
    bloom_filter: BloomFilter,
}

impl Trie {
    pub fn new(size: usize, num_hashes: usize) -> Self {
        let optimized_size = size
            .checked_next_power_of_two()
            .expect("Next power of 2 usize overflow");

        let node_estimate = size * 3 + 1024;
        let mut nodes = Vec::with_capacity(node_estimate * NODE_WIDTH);

        // Root node (all zeros)
        nodes.extend_from_slice(&[0u32; NODE_WIDTH]);

        Trie {
            nodes,
            bloom_filter: BloomFilter::new(optimized_size, num_hashes),
        }
    }

    pub fn shrink(&mut self) {
        self.nodes.shrink_to_fit();
    }

    pub fn insert(&mut self, word: &str) {
        let mut current_idx = 0;
        let bytes = word.as_bytes();

        for &b in bytes {
            let mask_bit = unsafe { *CHAR_TO_MASK.get_unchecked(b as usize) };
            if mask_bit == 0 {
                continue;
            }
            let bit_idx = mask_bit.trailing_zeros() as usize;

            let node_offset = current_idx << 5;
            let mask = unsafe { *self.nodes.get_unchecked(node_offset) };

            if (mask & mask_bit) == 0 {
                let new_node_idx = (self.nodes.len() >> 5) as u32;
                self.nodes.extend_from_slice(&[0u32; NODE_WIDTH]);

                unsafe {
                    *self.nodes.get_unchecked_mut(node_offset) |= mask_bit;
                    *self.nodes.get_unchecked_mut(node_offset + 1 + bit_idx) = new_node_idx;
                }
                current_idx = new_node_idx as usize;
            } else {
                current_idx =
                    unsafe { *self.nodes.get_unchecked(node_offset + 1 + bit_idx) as usize };
            }
        }

        unsafe {
            *self.nodes.get_unchecked_mut(current_idx << 5) |= 1 << 31;
        }
        self.bloom_filter.insert(word);
    }

    #[inline(always)]
    pub fn contains(&self, word: &str) -> bool {
        if !self.bloom_filter.contains(word) {
            return false;
        }

        let bytes = word.as_bytes();
        let mut current_idx = 0;
        let ptr = self.nodes.as_ptr();

        for i in 0..bytes.len() {
            let b = unsafe { *bytes.get_unchecked(i) };
            let mask_bit = unsafe { *CHAR_TO_MASK.get_unchecked(b as usize) };
            if mask_bit == 0 {
                return false;
            }

            let node_ptr = unsafe { ptr.add(current_idx << 5) };
            let mask = unsafe { *node_ptr };
            if (mask & mask_bit) == 0 {
                return false;
            }
            let bit_idx = mask_bit.trailing_zeros() as usize;
            current_idx = unsafe { *node_ptr.add(1 + bit_idx) as usize };
        }

        unsafe { (*ptr.add(current_idx << 5) & (1 << 31)) != 0 }
    }

    pub fn print(&self) {
        // Start at index 0 (the root)
        self.debug_print(0, 0);
    }

    fn debug_print(&self, node_idx: usize, indent: usize) {
        let node_offset = node_idx << 5;
        let mask = unsafe { *self.nodes.get_unchecked(node_offset) };
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

        for i in 0..26 {
            if (mask & (1 << i)) != 0 {
                let child_idx = unsafe { *self.nodes.get_unchecked(node_offset + 1 + i) as usize };
                self.debug_print(child_idx, indent + 1);
            }
        }
    }

    pub fn words_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut current_idx = 0;
        let bytes = prefix.as_bytes();

        for &b in bytes {
            let mask_bit = unsafe { *CHAR_TO_MASK.get_unchecked(b as usize) };
            if mask_bit == 0 {
                return Vec::new();
            }
            let bit_idx = mask_bit.trailing_zeros() as usize;

            let node_offset = current_idx << 5;
            let mask = unsafe { *self.nodes.get_unchecked(node_offset) };
            if (mask & mask_bit) == 0 {
                return Vec::new();
            }
            current_idx = unsafe { *self.nodes.get_unchecked(node_offset + 1 + bit_idx) as usize };
        }

        let mut results = Vec::new();
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
        let node_offset = node_idx << 5;
        let mask = unsafe { *self.nodes.get_unchecked(node_offset) };

        if (mask & (1 << 31)) != 0 {
            results.push(unsafe { std::str::from_utf8_unchecked(buffer) }.to_owned());
        }

        for i in 0..26 {
            if (mask & (1 << i)) != 0 {
                let child_idx = unsafe { *self.nodes.get_unchecked(node_offset + 1 + i) as usize };
                buffer.push(b'a' + i as u8);
                self.collect_words_from_node(child_idx, buffer, results);
                buffer.pop();
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
