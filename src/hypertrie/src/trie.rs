use crate::bloom_filter::BloomFilter;

const ALPHABET_SIZE: usize = 26;

static CHAR_TO_BIT: [u8; 256] = {
    let mut table = [255u8; 256];
    let mut i = 0;
    while i < 26 {
        table[(b'a' + i) as usize] = i;
        table[(b'A' + i) as usize] = i;
        i += 1;
    }
    table
};

pub struct Node {
    pub letter: u8,
    pub children_mask: u32,
    pub children_indices: [u32; 26],
    pub end_of_word: bool,
}

impl Node {
    fn new(letter: u8) -> Self {
        Node {
            letter,
            children_mask: 0,
            children_indices: [0; ALPHABET_SIZE],
            end_of_word: false,
        }
    }
}

pub struct Trie {
    nodes: Vec<Node>,
    bloom_filter: BloomFilter,
}

impl Trie {
    pub fn new(size: usize, num_hashes: usize) -> Self {
        let optimized_size = size
            .checked_next_power_of_two()
            .expect("Next power of 2 usize overflow");

        let mut nodes = Vec::with_capacity(size);
        nodes.push(Node::new(0));

        Trie {
            nodes,
            bloom_filter: BloomFilter::new(optimized_size, num_hashes),
        }
    }

    pub fn insert(&mut self, word: &[u8]) {
        let mut current_idx = 0;

        for &b in word {
            let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) } as usize;
            if bit_idx == 255 {
                continue;
            }

            // Check if child exists using bitmask
            if (unsafe { self.nodes.get_unchecked(current_idx) }.children_mask & (1 << bit_idx))
                == 0
            {
                let new_node_idx = self.nodes.len() as u32;
                self.nodes.push(Node::new(b'a' + bit_idx as u8));

                // Update parent
                let node = unsafe { self.nodes.get_unchecked_mut(current_idx) };
                node.children_mask |= 1 << bit_idx;
                node.children_indices[bit_idx] = new_node_idx;

                current_idx = new_node_idx as usize;
            } else {
                current_idx = unsafe {
                    self.nodes.get_unchecked(current_idx).children_indices[bit_idx] as usize
                };
            }
        }

        unsafe { self.nodes.get_unchecked_mut(current_idx) }.end_of_word = true;
        self.bloom_filter.insert(word);
    }

    pub fn contains(&self, word: &[u8]) -> bool {
        // Bloom Filter is usually faster than a full Trie walk for non-members
        if !self.bloom_filter.contains(word) {
            return false;
        }

        let mut current_idx = 0;
        for &b in word {
            let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) } as usize;
            if bit_idx == 255 {
                continue;
            }

            let node = unsafe { self.nodes.get_unchecked(current_idx) };
            if (node.children_mask & (1 << bit_idx)) == 0 {
                return false;
            }
            current_idx = node.children_indices[bit_idx] as usize;
        }

        unsafe { self.nodes.get_unchecked(current_idx) }.end_of_word
    }

    pub fn print(&self) {
        // Start at index 0 (the root)
        self.debug_print(0, 0);
    }

    fn debug_print(&self, node_idx: usize, indent: usize) {
        let node = &self.nodes[node_idx];
        let padding = "  ".repeat(indent);

        if node_idx == 0 {
            println!("Root");
        } else {
            println!(
                "{}'{}' (end_of_word: {})",
                padding, node.letter as char, node.end_of_word
            );
        }

        // Since we are using a bitmask and an index array, we iterate
        // through the alphabet and check the mask.
        for i in 0..26 {
            if (node.children_mask & (1 << i)) != 0 {
                let child_idx = node.children_indices[i] as usize;
                self.debug_print(child_idx, indent + 1);
            }
        }
    }

    pub fn words_with_prefix(&self, prefix: &[u8]) -> Vec<String> {
        let mut current_idx = 0; // Start at root

        // 1. Navigate to the end of the prefix
        for &b in prefix {
            let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) } as usize;
            if bit_idx == 255 {
                return Vec::new();
            }

            let node = unsafe { self.nodes.get_unchecked(current_idx) };
            // Use the bitmask to check if the path exists
            if (node.children_mask & (1 << bit_idx)) == 0 {
                return Vec::new(); // Prefix not found
            }
            current_idx = node.children_indices[bit_idx] as usize;
        }

        // 2. Collect all words starting from this node
        let mut results = Vec::new();
        // Pre-allocate the buffer with the prefix to avoid mid-search reallocations
        let mut buffer = Vec::with_capacity(prefix.len() + 10);
        for &b in prefix {
            let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) };
            // bit_idx is guaranteed to be < 26 because of the verification in Step 1
            buffer.push(b'a' + bit_idx);
        }

        self.collect_words_from_node(current_idx, &mut buffer, &mut results);
        results
    }

    fn collect_words_from_node(
        &self,
        node_idx: usize,
        buffer: &mut Vec<u8>,
        results: &mut Vec<String>,
    ) {
        let node = unsafe { self.nodes.get_unchecked(node_idx) };

        // If this node marks the end of a word, save the current buffer
        if node.end_of_word {
            // Optimization: Use String::from_utf8_unchecked because we know buffer contains ASCII
            results.push(unsafe { String::from_utf8_unchecked(buffer.clone()) });
        }

        // Iterate through all possible children (a-z)
        for i in 0..26 {
            // Only recurse if the bitmask says a child exists
            if (node.children_mask & (1 << i)) != 0 {
                let child_idx = node.children_indices[i] as usize;

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
        trie.insert(b"hello");
        trie.insert(b"world");

        assert!(trie.contains(b"hello"));
        assert!(trie.contains(b"world"));
        assert!(!trie.contains(b"hell"));
        assert!(!trie.contains(b"word"));
    }

    #[test]
    fn test_words_with_prefix() {
        let mut trie = Trie::new(100, 3);
        trie.insert(b"apple");
        trie.insert(b"app");
        trie.insert(b"application");
        trie.insert(b"banana");

        let apps = trie.words_with_prefix(b"app");
        assert_eq!(apps.len(), 3);
        assert!(apps.contains(&"apple".to_string()));
        assert!(apps.contains(&"app".to_string()));
        assert!(apps.contains(&"application".to_string()));

        let banas = trie.words_with_prefix(b"ban");
        assert_eq!(banas.len(), 1);
        assert!(banas.contains(&"banana".to_string()));

        let unknowns = trie.words_with_prefix(b"unknown");
        assert!(unknowns.is_empty());
    }

    #[test]
    fn test_long_string_stack_threshold() {
        let mut trie = Trie::new(100, 3);
        // A word longer than common stack buffers
        let long_word = "a".repeat(100);
        trie.insert(long_word.as_bytes());
        assert!(trie.contains(long_word.as_bytes()));

        // Also test bloom filter long path
        assert!(trie.bloom_filter.contains(long_word.as_bytes()));

        // test words with prefix long
        let results = trie.words_with_prefix(long_word.as_bytes());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], long_word);
    }

    #[test]
    fn test_case_insensitivity_and_invalid_chars() {
        let mut trie = Trie::new(100, 3);

        // Case insensitivity
        trie.insert(b"Hello");
        assert!(trie.contains(b"hello"));
        assert!(trie.contains(b"HELLO"));
        assert!(trie.contains(b"hElLo"));

        // Prefix search with case insensitivity
        let results = trie.words_with_prefix(b"HELL");
        assert!(results.contains(&"hello".to_string()));

        // Prefix search early return on invalid character
        assert!(trie.words_with_prefix(b"!").is_empty());
    }
}
