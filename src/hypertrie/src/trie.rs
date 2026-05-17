use crate::bloom_filter::BloomFilter;

const ALPHABET_SIZE: usize = 26;

/// Lookup table to map ASCII characters to their 0-255 bit indices.
/// 'a'-'z' and 'A'-'Z' map to 0-255. All others map to 255 (invalid).
static CHAR_TO_BIT: [u8; 256] = {
    let mut table = [255u8; 256];
    let mut i = 0u8;
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

    pub fn insert(&mut self, word: &[u8]) {
        let mut current_idx = 0;
        let mut stack_buf = [0u8; 64];
        let mut normalized_len = 0;
        let can_use_stack = word.len() <= 64;

        for &b in word {
            let bit_idx = CHAR_TO_BIT[b as usize];
            if bit_idx == 255 { continue; }
            let bit_idx = bit_idx as usize;

            let next_idx = self.nodes[current_idx].children_indices[bit_idx];
            if next_idx == 0 {
                let new_node_idx = self.nodes.len() as u32;
                let normalized_char = b'a' + bit_idx as u8;
                self.nodes.push(Node::new(normalized_char));

                let node = &mut self.nodes[current_idx];
                node.children_mask |= 1 << bit_idx;
                node.children_indices[bit_idx] = new_node_idx;
                current_idx = new_node_idx as usize;
            } else {
                current_idx = next_idx as usize;
            }

            if can_use_stack {
                stack_buf[normalized_len] = b'a' + bit_idx as u8;
                normalized_len += 1;
            }
        }

        self.nodes[current_idx].end_of_word = true;

        if can_use_stack {
            self.bloom_filter.insert(&stack_buf[..normalized_len]);
        } else {
            // Only allocate if word is long
            let normalized: Vec<u8> = word.iter()
                .map(|&b| CHAR_TO_BIT[b as usize])
                .filter(|&idx| idx != 255)
                .map(|idx| b'a' + idx)
                .collect();
            self.bloom_filter.insert(&normalized);
        }
    }

    pub fn contains(&self, word: &[u8]) -> bool {
        let mut stack_buf = [0u8; 64];
        let mut normalized_len = 0;

        if word.len() <= 64 {
            for &b in word {
                let bit_idx = CHAR_TO_BIT[b as usize];
                if bit_idx == 255 { return false; }
                stack_buf[normalized_len] = b'a' + bit_idx;
                normalized_len += 1;
            }
            let normalized = &stack_buf[..normalized_len];
            if !self.bloom_filter.contains(normalized) {
                return false;
            }

            let mut current_idx = 0;
            for &b in normalized {
                let bit_idx = (b - b'a') as usize;
                let node = unsafe { self.nodes.get_unchecked(current_idx) };
                if (node.children_mask & (1 << bit_idx)) == 0 {
                    return false;
                }
                current_idx = node.children_indices[bit_idx] as usize;
            }
            unsafe { self.nodes.get_unchecked(current_idx) }.end_of_word
        } else {
            let normalized: Vec<u8> = word.iter()
                .map(|&b| CHAR_TO_BIT[b as usize])
                .filter(|&idx| idx != 255)
                .map(|idx| b'a' + idx)
                .collect();

            if normalized.len() != word.len() { return false; } // Assuming non-ASCII/invalid means no match

            if !self.bloom_filter.contains(&normalized) {
                return false;
            }
            let mut current_idx = 0;
            for &b in &normalized {
                let bit_idx = (b - b'a') as usize;
                let node = &self.nodes[current_idx];
                if (node.children_mask & (1 << bit_idx)) == 0 {
                    return false;
                }
                current_idx = node.children_indices[bit_idx] as usize;
            }
            self.nodes[current_idx].end_of_word
        }
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
        let mut stack_buf = [0u8; 64];
        let mut normalized_len = 0;
        let can_use_stack = prefix.len() <= 64;

        let mut current_idx = 0;
        let mut buffer = if can_use_stack {
            for &b in prefix {
                let bit_idx = CHAR_TO_BIT[b as usize];
                if bit_idx == 255 { return Vec::new(); }
                stack_buf[normalized_len] = b'a' + bit_idx;
                normalized_len += 1;
            }
            stack_buf[..normalized_len].to_vec()
        } else {
             prefix.iter()
                .map(|&b| CHAR_TO_BIT[b as usize])
                .filter(|&idx| idx != 255)
                .map(|idx| b'a' + idx)
                .collect()
        };

        // 1. Navigate to the end of the prefix
        for &b in &buffer {
            let bit_idx = (b - b'a') as usize;
            let node = &self.nodes[current_idx];
            if (node.children_mask & (1 << bit_idx)) == 0 {
                return Vec::new();
            }
            current_idx = node.children_indices[bit_idx] as usize;
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
    fn test_case_insensitivity() {
        let mut trie = Trie::new(100, 3);
        trie.insert(b"Apple");
        assert!(trie.contains(b"apple"));
        assert!(trie.contains(b"APPLE"));
    }
}
