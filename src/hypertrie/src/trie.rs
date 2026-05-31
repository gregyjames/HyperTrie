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

        let mut nodes = Vec::with_capacity(1024);
        nodes.push(Node::new(0));

        Trie {
            nodes,
            bloom_filter: BloomFilter::new(optimized_size, num_hashes),
        }
    }

    pub fn insert(&mut self, word: &str) {
        let mut current_idx = 0;
        let bytes = word.as_bytes();
        let mut normalized_bytes = Vec::with_capacity(bytes.len());

        for &b in bytes {
            let bit_idx = CHAR_TO_BIT[b as usize] as usize;
            if bit_idx > 25 {
                continue;
            }

            let char_val = b'a' + bit_idx as u8;
            normalized_bytes.push(char_val);

            // Check if child exists using bitmask
            if (self.nodes[current_idx].children_mask & (1 << bit_idx)) == 0 {
                let new_node_idx = self.nodes.len() as u32;
                self.nodes.push(Node::new(char_val));

                // Update parent
                let node = &mut self.nodes[current_idx];
                node.children_mask |= 1 << bit_idx;
                node.children_indices[bit_idx] = new_node_idx;

                current_idx = new_node_idx as usize;
            } else {
                current_idx = self.nodes[current_idx].children_indices[bit_idx] as usize;
            }
        }

        self.nodes[current_idx].end_of_word = true;
        self.bloom_filter.insert_bytes(&normalized_bytes);
    }

    pub fn contains(&self, word: &str) -> bool {
        let mut current_idx = 0;
        let bytes = word.as_bytes();
        let mut normalized_bytes = Vec::with_capacity(bytes.len());

        for &b in bytes {
            let bit_idx = CHAR_TO_BIT[b as usize] as usize;
            if bit_idx > 25 {
                continue;
            }

            let char_val = b'a' + bit_idx as u8;
            normalized_bytes.push(char_val);

            let node = &self.nodes[current_idx];
            if (node.children_mask & (1 << bit_idx)) == 0 {
                return false;
            }
            current_idx = node.children_indices[bit_idx] as usize;
        }

        if !self.nodes[current_idx].end_of_word {
            return false;
        }

        // Bloom Filter is usually faster than a full Trie walk for non-members
        // We check it last here because we already have normalized bytes
        self.bloom_filter.contains_bytes(&normalized_bytes)
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
        let mut current_idx = 0; // Start at root
        let bytes = prefix.as_bytes();
        let mut normalized_prefix = Vec::with_capacity(bytes.len());

        // 1. Navigate to the end of the prefix
        for &b in bytes {
            let bit_idx = CHAR_TO_BIT[b as usize] as usize;
            if bit_idx > 25 {
                continue;
            }

            normalized_prefix.push(b'a' + bit_idx as u8);

            let node = &self.nodes[current_idx];
            // Use the bitmask to check if the path exists
            if (node.children_mask & (1 << bit_idx)) == 0 {
                return Vec::new(); // Prefix not found
            }
            current_idx = node.children_indices[bit_idx] as usize;
        }

        if normalized_prefix.is_empty() && !prefix.is_empty() {
            return Vec::new();
        }
        let mut results = Vec::new();
        // Pre-allocate the buffer with the prefix to avoid mid-search reallocations
        let mut buffer = normalized_prefix;

        self.collect_words_from_node(current_idx, &mut buffer, &mut results);
        results
    }

    fn collect_words_from_node(
        &self,
        node_idx: usize,
        buffer: &mut Vec<u8>,
        results: &mut Vec<String>,
    ) {
        let node = &self.nodes[node_idx];

        // If this node marks the end of a word, save the current buffer
        if node.end_of_word {
            // Optimization: Use String::from_utf8_unchecked if you're 100% sure of ASCII
            results.push(String::from_utf8_lossy(buffer).into_owned());
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
    fn test_trie_invalid_chars() {
        let mut trie = Trie::new(100, 3);
        trie.insert("abc 123");
        // "abc 123" becomes "abc" because space and digits are ignored
        assert!(trie.contains("abc"));
        assert!(trie.contains("abc 123"));
        assert!(trie.contains("a b c"));

        trie.insert("def");
        assert!(trie.words_with_prefix("abc").contains(&"abc".to_string()));
        assert!(trie.words_with_prefix("abc ").contains(&"abc".to_string()));
        assert!(trie.words_with_prefix("1").is_empty());

        // Test extended ASCII
        trie.insert("x\u{00A0}y");
        assert!(trie.contains("xy"));
    }

    #[test]
    fn test_trie_case_insensitivity() {
        let mut trie = Trie::new(100, 3);
        trie.insert("Hello");
        assert!(trie.contains("hello"));
        assert!(trie.contains("HELLO"));
        assert!(trie.contains("HeLlO"));
    }
}
