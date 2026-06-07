use crate::bloom_filter::{BloomFilter, SEED};
use gxhash::GxHasher;
use std::hash::Hasher;

const ALPHABET_SIZE: usize = 26;

static CHAR_TO_BIT: [u8; 256] = {
    let mut table = [255u8; 256];
    let mut i = 0;
    while i < 26 {
        table[(b'a' + i as u8) as usize] = i as u8;
        table[(b'A' + i as u8) as usize] = i as u8;
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

    pub fn insert(&mut self, word: &str) {
        let bytes = word.as_bytes();
        let mut stack_buf = [0u8; 64];
        let normalized = if bytes.len() <= 64 {
            &mut stack_buf[..bytes.len()]
        } else {
            return self.insert_slow(word);
        };

        let mut current_idx = 0;
        let mut count = 0;

        for &b in bytes {
            let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) as usize };
            if bit_idx == 255 {
                continue;
            }

            let char_val = b.to_ascii_lowercase();
            normalized[count] = char_val;
            count += 1;

            // Check if child exists using bitmask
            let node = unsafe { self.nodes.get_unchecked(current_idx) };
            if (node.children_mask & (1 << bit_idx)) == 0 {
                let new_node_idx = self.nodes.len() as u32;
                self.nodes.push(Node::new(char_val));

                // Update parent
                let node = unsafe { self.nodes.get_unchecked_mut(current_idx) };
                node.children_mask |= 1 << bit_idx;
                unsafe {
                    *node.children_indices.get_unchecked_mut(bit_idx) = new_node_idx;
                }

                current_idx = new_node_idx as usize;
            } else {
                current_idx = unsafe { *node.children_indices.get_unchecked(bit_idx) as usize };
            }
        }

        unsafe {
            self.nodes.get_unchecked_mut(current_idx).end_of_word = true;
        }

        let mut hasher = GxHasher::with_seed(SEED);
        hasher.write(&normalized[..count]);
        self.bloom_filter.insert_hash(hasher.finish());
    }

    #[inline(never)]
    fn insert_slow(&mut self, word: &str) {
        let mut current_idx = 0;
        let mut normalized = Vec::with_capacity(word.len());

        for &b in word.as_bytes() {
            let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) as usize };
            if bit_idx == 255 {
                continue;
            }

            let char_val = b.to_ascii_lowercase();
            normalized.push(char_val);

            // Check if child exists using bitmask
            let node = unsafe { self.nodes.get_unchecked(current_idx) };
            if (node.children_mask & (1 << bit_idx)) == 0 {
                let new_node_idx = self.nodes.len() as u32;
                self.nodes.push(Node::new(char_val));

                // Update parent
                let node = unsafe { self.nodes.get_unchecked_mut(current_idx) };
                node.children_mask |= 1 << bit_idx;
                unsafe {
                    *node.children_indices.get_unchecked_mut(bit_idx) = new_node_idx;
                }

                current_idx = new_node_idx as usize;
            } else {
                current_idx = unsafe { *node.children_indices.get_unchecked(bit_idx) as usize };
            }
        }

        unsafe {
            self.nodes.get_unchecked_mut(current_idx).end_of_word = true;
        }

        let mut hasher = GxHasher::with_seed(SEED);
        hasher.write(&normalized);
        self.bloom_filter.insert_hash(hasher.finish());
    }

    pub fn contains(&self, word: &str) -> bool {
        let bytes = word.as_bytes();
        let mut stack_buf = [0u8; 64];
        let normalized = if bytes.len() <= 64 {
            &mut stack_buf[..bytes.len()]
        } else {
            return self.contains_slow(word);
        };

        for (i, &b) in bytes.iter().enumerate() {
            let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) as usize };
            if bit_idx == 255 {
                return false;
            }
            normalized[i] = b.to_ascii_lowercase();
        }

        let mut hasher = GxHasher::with_seed(SEED);
        hasher.write(normalized);
        if !self.bloom_filter.contains_hash(hasher.finish()) {
            return false;
        }

        let mut current_idx = 0;
        for &mut char_val in normalized {
            let bit_idx = (char_val - b'a') as usize;

            let node = unsafe { self.nodes.get_unchecked(current_idx) };
            if (node.children_mask & (1 << bit_idx)) == 0 {
                return false;
            }
            current_idx = unsafe { *node.children_indices.get_unchecked(bit_idx) as usize };
        }

        unsafe { self.nodes.get_unchecked(current_idx).end_of_word }
    }

    #[inline(never)]
    fn contains_slow(&self, word: &str) -> bool {
        let mut normalized = Vec::with_capacity(word.len());
        for &b in word.as_bytes() {
            let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) as usize };
            if bit_idx == 255 {
                return false;
            }
            normalized.push(b.to_ascii_lowercase());
        }

        let mut hasher = GxHasher::with_seed(SEED);
        hasher.write(&normalized);
        if !self.bloom_filter.contains_hash(hasher.finish()) {
            return false;
        }

        let mut current_idx = 0;
        for &char_val in &normalized {
            let bit_idx = (char_val - b'a') as usize;

            let node = unsafe { self.nodes.get_unchecked(current_idx) };
            if (node.children_mask & (1 << bit_idx)) == 0 {
                return false;
            }
            current_idx = unsafe { *node.children_indices.get_unchecked(bit_idx) as usize };
        }

        unsafe { self.nodes.get_unchecked(current_idx).end_of_word }
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

    pub fn words_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut current_idx = 0;
        let bytes = prefix.as_bytes();

        let mut normalized_prefix = Vec::with_capacity(bytes.len());
        for &b in bytes {
            let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) as usize };
            if bit_idx == 255 {
                return Vec::new();
            }
            normalized_prefix.push(b.to_ascii_lowercase());
        }

        for &char_val in &normalized_prefix {
            let bit_idx = (char_val - b'a') as usize;
            let node = unsafe { self.nodes.get_unchecked(current_idx) };
            if (node.children_mask & (1 << bit_idx)) == 0 {
                return Vec::new();
            }
            current_idx = unsafe { *node.children_indices.get_unchecked(bit_idx) as usize };
        }

        let mut results = Vec::new();
        self.collect_words_from_node(current_idx, &mut normalized_prefix, &mut results);
        results
    }

    fn collect_words_from_node(
        &self,
        node_idx: usize,
        buffer: &mut Vec<u8>,
        results: &mut Vec<String>,
    ) {
        let node = unsafe { self.nodes.get_unchecked(node_idx) };

        if node.end_of_word {
            results.push(unsafe { String::from_utf8_unchecked(buffer.clone()) });
        }

        for i in 0..26 {
            if (node.children_mask & (1 << i)) != 0 {
                let child_idx = unsafe { *node.children_indices.get_unchecked(i) as usize };

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

    #[test]
    fn test_trie_case_insensitivity() {
        let mut trie = Trie::new(100, 3);
        trie.insert("Hello");
        assert!(trie.contains("hello"));
        assert!(trie.contains("HELLO"));
        assert!(trie.contains("Hello"));

        let results = trie.words_with_prefix("HELL");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "hello");
    }

    #[test]
    fn test_trie_invalid_chars() {
        let mut trie = Trie::new(100, 3);
        // "abc 123" -> only "abc" should be inserted
        trie.insert("abc 123");
        assert!(trie.contains("abc"));
        assert!(!trie.contains("abc 123"));
        assert!(!trie.contains("123"));

        // Searching with invalid chars should return false or empty
        assert!(!trie.contains("abc!"));
        assert!(trie.words_with_prefix("abc 1").is_empty());
    }

    #[test]
    fn test_trie_contains_bloom_filter_hit_but_trie_miss() {
        // Create a situation where bloom filter might have a false positive
        // or just ensure the trie walk correctly returns false.
        let mut trie = Trie::new(100, 3);
        trie.insert("apple");

        // "apply" shares "appl" with "apple", but 'y' is missing.
        // Bloom filter might or might not hit, but Trie walk must return false.
        assert!(!trie.contains("apply"));
    }

    #[test]
    fn test_trie_long_strings() {
        let mut trie = Trie::new(100, 3);
        // String longer than 64 bytes to trigger *_slow paths
        let long_word = "a".repeat(65);
        let long_word_with_invalid = "a".repeat(64) + "1" + &"a".repeat(10);

        trie.insert(&long_word);
        assert!(trie.contains(&long_word));
        assert!(!trie.contains(&"b".repeat(65)));

        trie.insert(&long_word_with_invalid);
        // normalization should have removed '1'
        assert!(trie.contains(&"a".repeat(74)));
        assert!(!trie.contains(&long_word_with_invalid));

        let results = trie.words_with_prefix(&"a".repeat(60));
        assert!(results.contains(&long_word));
        assert!(results.contains(&"a".repeat(74)));
    }
}
