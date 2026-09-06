use std::borrow::Cow;

use crate::bloom_filter::BloomFilter;

const ALPHABET_SIZE: usize = 26;
const END_OF_WORD_BIT: u32 = 1 << 31;

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
    pub children_mask: u32,
    pub children_indices: [u32; 26],
}

impl Node {
    #[inline(always)]
    fn new() -> Self {
        Node {
            children_mask: 0,
            children_indices: [0; ALPHABET_SIZE],
        }
    }

    #[inline(always)]
    fn is_end_of_word(&self) -> bool {
        (self.children_mask & END_OF_WORD_BIT) != 0
    }

    #[inline(always)]
    fn set_end_of_word(&mut self) {
        self.children_mask |= END_OF_WORD_BIT;
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

        // Heuristic: estimated nodes = size * avg word length (approx 7)
        let mut nodes = Vec::with_capacity(size.saturating_mul(7).max(1024));
        nodes.push(Node::new());

        Trie {
            nodes,
            bloom_filter: BloomFilter::new(optimized_size, num_hashes),
        }
    }

    pub fn insert(&mut self, word: &str) {
        let mut current_idx = 0;
        let bytes = word.as_bytes();
        let len = bytes.len();

        // Use stack buffer for normalization and filtering if word is short enough.
        // Store bit_idx directly (0..25) to avoid redundant ASCII math (+b'a' / -b'a').
        let mut stack_buf = [0u8; 64];
        let mut filtered_len = 0;
        let normalized: Cow<[u8]> = if len <= 64 {
            for &b in bytes {
                let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) };
                if bit_idx != 255 {
                    stack_buf[filtered_len] = bit_idx;
                    filtered_len += 1;
                }
            }
            Cow::Borrowed(&stack_buf[..filtered_len])
        } else {
            let mut v = Vec::with_capacity(len);
            for &b in bytes {
                let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) };
                if bit_idx != 255 {
                    v.push(bit_idx);
                }
            }
            Cow::Owned(v)
        };

        for &bit_idx_u8 in normalized.as_ref() {
            let bit_idx = bit_idx_u8 as usize;

            // Check if child exists using bitmask
            unsafe {
                let node = self.nodes.get_unchecked(current_idx);
                if (node.children_mask & (1 << bit_idx)) == 0 {
                    let new_node_idx = self.nodes.len() as u32;
                    self.nodes.push(Node::new());

                    // Update parent
                    let node = self.nodes.get_unchecked_mut(current_idx);
                    node.children_mask |= 1 << bit_idx;
                    node.children_indices[bit_idx] = new_node_idx;

                    current_idx = new_node_idx as usize;
                } else {
                    current_idx = *node.children_indices.get_unchecked(bit_idx) as usize;
                }
            }
        }

        unsafe {
            self.nodes.get_unchecked_mut(current_idx).set_end_of_word();
        }
        self.bloom_filter.insert(&normalized);
    }

    pub fn contains(&self, word: &str) -> bool {
        let bytes = word.as_bytes();
        let len = bytes.len();

        // Normalize once to a stack buffer with direct bit_idx values (0..25)
        let mut stack_buf = [0u8; 64];
        let mut filtered_len = 0;
        let normalized: Cow<[u8]> = if len <= 64 {
            for &b in bytes {
                let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) };
                if bit_idx != 255 {
                    stack_buf[filtered_len] = bit_idx;
                    filtered_len += 1;
                }
            }
            Cow::Borrowed(&stack_buf[..filtered_len])
        } else {
            let mut v = Vec::with_capacity(len);
            for &b in bytes {
                let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) };
                if bit_idx != 255 {
                    v.push(bit_idx);
                }
            }
            Cow::Owned(v)
        };

        // Bloom Filter is usually faster than a full Trie walk for non-members
        if !self.bloom_filter.contains(&normalized) {
            return false;
        }

        let mut current_idx = 0;
        for &bit_idx_u8 in normalized.as_ref() {
            let bit_idx = bit_idx_u8 as usize;

            let node = unsafe { self.nodes.get_unchecked(current_idx) };
            if (node.children_mask & (1 << bit_idx)) == 0 {
                return false;
            }
            current_idx = unsafe { *node.children_indices.get_unchecked(bit_idx) as usize };
        }

        unsafe { self.nodes.get_unchecked(current_idx).is_end_of_word() }
    }

    pub fn print(&self) {
        // Start at index 0 (the root)
        self.debug_print(0, None, 0);
    }

    fn debug_print(&self, node_idx: usize, letter: Option<char>, indent: usize) {
        let node = &self.nodes[node_idx];
        let padding = "  ".repeat(indent);

        if node_idx == 0 {
            println!("Root");
        } else if let Some(ch) = letter {
            println!(
                "{}'{}' (end_of_word: {})",
                padding,
                ch,
                node.is_end_of_word()
            );
        }

        // Since we are using a bitmask and an index array, we iterate
        // through the alphabet and check the mask.
        for i in 0..26 {
            if (node.children_mask & (1 << i)) != 0 {
                let child_idx = node.children_indices[i] as usize;
                let ch = (b'a' + i as u8) as char;
                self.debug_print(child_idx, Some(ch), indent + 1);
            }
        }
    }

    pub fn words_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut current_idx = 0; // Start at root
        let bytes = prefix.as_bytes();

        // Normalize prefix once
        let mut buffer = Vec::with_capacity(bytes.len() + 8);

        // 1. Navigate to the end of the prefix
        for &b in bytes {
            let bit_idx = unsafe { *CHAR_TO_BIT.get_unchecked(b as usize) };
            if bit_idx == 255 {
                continue;
            }
            let bit_idx = bit_idx as usize;

            let node = unsafe { self.nodes.get_unchecked(current_idx) };
            // Use the bitmask to check if the path exists
            if (node.children_mask & (1 << bit_idx)) == 0 {
                return Vec::new(); // Prefix not found
            }
            current_idx = unsafe { *node.children_indices.get_unchecked(bit_idx) as usize };
            buffer.push(b'a' + bit_idx as u8);
        }

        // 2. Collect all words starting from this node
        let mut results = Vec::new();
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
        if node.is_end_of_word() {
            // SAFETY: The trie only contains valid lowercase ASCII letters 'a'-'z'
            // and characters from the initial prefix (also normalized).
            unsafe {
                results.push(String::from_utf8_unchecked(buffer.clone()));
            }
        }

        // Iterate through all possible children (a-z)
        for i in 0..26 {
            // Only recurse if the bitmask says a child exists
            if (node.children_mask & (1 << i)) != 0 {
                let child_idx = unsafe { *node.children_indices.get_unchecked(i) as usize };

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
    fn test_case_insensitive_bloom_filter_bug() {
        let mut trie = Trie::new(100, 3);
        trie.insert("Hello");
        assert!(
            trie.contains("hello"),
            "Trie should be case-insensitive, but Bloom Filter blocked it."
        );
    }

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
    fn test_long_string_and_invalid_chars() {
        let mut trie = Trie::new(100, 3);

        // String > 64 bytes to exercise len > 64 heap-allocation path
        let long_word = "a".repeat(70) + "b";
        trie.insert(&long_word);
        assert!(trie.contains(&long_word));

        // String with invalid non-alphabet characters
        let word_with_symbols = "hello-world_123!";
        trie.insert(word_with_symbols);
        assert!(trie.contains("helloworld"));
        assert!(trie.contains(word_with_symbols));

        // Prefix with invalid characters
        let pref = trie.words_with_prefix("hel-lo!");
        assert!(!pref.is_empty());
    }

    #[test]
    fn test_debug_print() {
        let mut trie = Trie::new(100, 3);
        trie.insert("cat");
        trie.insert("car");
        trie.print();
    }
}
