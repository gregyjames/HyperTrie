use crate::bloom_filter::BloomFilter;

const ALPHABET_SIZE: usize = 26;

const CHAR_TO_BIT: [u8; 256] = {
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

        // size is word count. A good heuristic for nodes is word count * some factor.
        // Let's use 'size' as initial capacity for nodes too, as we start with 1024 anyway.
        let mut nodes = Vec::with_capacity(size.max(1024));
        nodes.push(Node::new(0));

        Trie {
            nodes,
            bloom_filter: BloomFilter::new(optimized_size, num_hashes),
        }
    }

    pub fn insert(&mut self, word: &str) {
        let bytes = word.as_bytes();
        if bytes.len() <= 64 {
            self.insert_fast(bytes);
        } else {
            self.insert_slow(bytes);
        }
    }

    fn insert_fast(&mut self, bytes: &[u8]) {
        let mut current_idx = 0;
        let mut normalized = [0u8; 64];
        let mut n_len = 0;

        for &b in bytes {
            let bit_idx = CHAR_TO_BIT[b as usize];
            if bit_idx == 255 {
                continue;
            }

            let char_val = b'a' + bit_idx;
            normalized[n_len] = char_val;
            n_len += 1;

            let bit_idx = bit_idx as usize;
            if (self.nodes[current_idx].children_mask & (1 << bit_idx)) == 0 {
                let new_node_idx = self.nodes.len() as u32;
                self.nodes.push(Node::new(char_val));

                let node = &mut self.nodes[current_idx];
                node.children_mask |= 1 << bit_idx;
                node.children_indices[bit_idx] = new_node_idx;

                current_idx = new_node_idx as usize;
            } else {
                current_idx = self.nodes[current_idx].children_indices[bit_idx] as usize;
            }
        }

        self.nodes[current_idx].end_of_word = true;
        self.bloom_filter.insert_bytes(&normalized[..n_len]);
    }

    fn insert_slow(&mut self, bytes: &[u8]) {
        let mut current_idx = 0;
        let mut normalized = Vec::with_capacity(bytes.len());

        for &b in bytes {
            let bit_idx = CHAR_TO_BIT[b as usize];
            if bit_idx == 255 {
                continue;
            }

            let char_val = b'a' + bit_idx;
            normalized.push(char_val);

            let bit_idx = bit_idx as usize;
            if (self.nodes[current_idx].children_mask & (1 << bit_idx)) == 0 {
                let new_node_idx = self.nodes.len() as u32;
                self.nodes.push(Node::new(char_val));

                let node = &mut self.nodes[current_idx];
                node.children_mask |= 1 << bit_idx;
                node.children_indices[bit_idx] = new_node_idx;

                current_idx = new_node_idx as usize;
            } else {
                current_idx = self.nodes[current_idx].children_indices[bit_idx] as usize;
            }
        }

        self.nodes[current_idx].end_of_word = true;
        self.bloom_filter.insert_bytes(&normalized);
    }

    pub fn contains(&self, word: &str) -> bool {
        let bytes = word.as_bytes();
        if bytes.len() <= 64 {
            self.contains_fast(bytes)
        } else {
            self.contains_slow(bytes)
        }
    }

    fn contains_fast(&self, bytes: &[u8]) -> bool {
        let mut normalized = [0u8; 64];
        let mut n_len = 0;

        for &b in bytes {
            let bit_idx = CHAR_TO_BIT[b as usize];
            if bit_idx != 255 {
                normalized[n_len] = b'a' + bit_idx;
                n_len += 1;
            }
        }

        let normalized_slice = &normalized[..n_len];
        if !self.bloom_filter.contains_bytes(normalized_slice) {
            return false;
        }

        let mut current_idx = 0;
        for &char_val in normalized_slice {
            let bit_idx = (char_val - b'a') as usize;
            let node = &self.nodes[current_idx];
            if (node.children_mask & (1 << bit_idx)) == 0 {
                return false;
            }
            current_idx = node.children_indices[bit_idx] as usize;
        }

        self.nodes[current_idx].end_of_word
    }

    fn contains_slow(&self, bytes: &[u8]) -> bool {
        let mut normalized = Vec::with_capacity(bytes.len());
        for &b in bytes {
            let bit_idx = CHAR_TO_BIT[b as usize];
            if bit_idx != 255 {
                normalized.push(b'a' + bit_idx);
            }
        }

        if !self.bloom_filter.contains_bytes(&normalized) {
            return false;
        }

        let mut current_idx = 0;
        for &char_val in &normalized {
            let bit_idx = (char_val - b'a') as usize;
            let node = &self.nodes[current_idx];
            if (node.children_mask & (1 << bit_idx)) == 0 {
                return false;
            }
            current_idx = node.children_indices[bit_idx] as usize;
        }

        self.nodes[current_idx].end_of_word
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
            let bit_idx = CHAR_TO_BIT[b as usize];
            if bit_idx == 255 {
                continue;
            }
            let char_val = b'a' + bit_idx;
            normalized_prefix.push(char_val);

            let node = &self.nodes[current_idx];
            let bit_idx = bit_idx as usize;
            if (node.children_mask & (1 << bit_idx)) == 0 {
                return Vec::new();
            }
            current_idx = node.children_indices[bit_idx] as usize;
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
    fn test_case_insensitivity() {
        let mut trie = Trie::new(100, 3);
        trie.insert("Hello");
        assert!(
            trie.contains("hello"),
            "Should find 'hello' after inserting 'Hello'"
        );
        assert!(
            trie.contains("HELLO"),
            "Should find 'HELLO' after inserting 'Hello'"
        );
        assert!(
            trie.contains("Hello"),
            "Should find 'Hello' after inserting 'Hello'"
        );
    }

    #[test]
    fn test_long_string_slow_path() {
        let mut trie = Trie::new(100, 3);
        // String longer than 64 characters to trigger the slow path
        let long_word = "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz";
        assert!(long_word.len() > 64);

        trie.insert(long_word);
        assert!(trie.contains(long_word));
        assert!(!trie.contains(&(long_word.to_string() + "extra")));
    }

    #[test]
    fn test_invalid_characters_filtering() {
        let mut trie = Trie::new(100, 3);
        // Characters other than a-z and A-Z should be ignored
        trie.insert("hello-world!");
        assert!(trie.contains("helloworld"));
        assert!(trie.contains("H E L L O W O R L D"));
        assert!(trie.contains("hello-world")); // because '-' is filtered out in search too
        assert!(!trie.contains("hello"));
    }

    #[test]
    fn test_words_with_prefix_filtering() {
        let mut trie = Trie::new(100, 3);
        trie.insert("apple-pie");
        let results = trie.words_with_prefix("apple");
        assert!(results.contains(&"applepie".to_string()));
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
