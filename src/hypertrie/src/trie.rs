use crate::bloom_filter::BloomFilter;

const ALPHABET_SIZE: usize = 26;

pub struct Node {
    letter: u8,
    children: [Option<Box<Node>>; ALPHABET_SIZE],
    end_of_word: bool,
}

impl Node {
    fn new(letter: u8) -> Self {
        const NODE_NODE: Option<Box<Node>> = None;

        Node {
            letter,
            children: [NODE_NODE; ALPHABET_SIZE],
            end_of_word: false,
        }
    }

    #[inline(always)]
    fn char_to_index(c: u8) -> usize {
        (c.to_ascii_lowercase() - b'a') as usize
    }
}

pub struct Trie {
    root: Box<Node>,
    bloom_filter: BloomFilter,
}

impl Trie {
    pub fn new(size: usize, num_hashes: usize) -> Self {
        Trie {
            root: Box::new(Node::new(0)),
            bloom_filter: BloomFilter::new(size, num_hashes),
        }
    }

    pub fn insert(&mut self, word: &str) {
        let bytes = word.as_bytes();
        let mut current = &mut self.root;

        for &c in bytes {
            let idx = Node::char_to_index(c);
            if current.children[idx].is_none() {
                current.children[idx] = Some(Box::new(Node::new(c)));
            }
            current = current.children[idx].as_mut().unwrap();
        }

        current.end_of_word = true;
        self.bloom_filter.insert(word);
    }

    pub fn contains(&self, word: &str) -> bool {
        if !self.bloom_filter.contains(word) {
            return false;
        }

        let mut current = &self.root;

        for &b in word.as_bytes() {
            let idx = Node::char_to_index(b);
            match &current.children[idx] {
                Some(node) => current = node,
                None => return false,
            }
        }

        current.end_of_word
    }

    pub fn print(&self) {
        self.debug_print(&self.root, 0);
    }

    fn debug_print(&self, node: &Node, indent: usize) {
        let padding = "  ".repeat(indent);
        if indent == 0 {
            println!("Root");
        } else {
            println!(
                "{}'{}' (end_of_word: {})",
                padding, node.letter as char, node.end_of_word
            );
        }
        for child in node.children.iter().flatten() {
            self.debug_print(child, indent + 1);
        }
    }

    pub fn words_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut current = &self.root;
        let bytes = prefix.as_bytes();

        for &b in bytes {
            let idx = Node::char_to_index(b);
            match &current.children[idx] {
                Some(node) => current = node,
                None => return Vec::new(),
            }
        }

        let mut results = Vec::new();
        let mut buffer = prefix.to_ascii_lowercase().into_bytes();
        self.collect_words_from_node(current, &mut buffer, &mut results);
        results
    }

    #[inline(always)]
    fn collect_words_from_node(&self, node: &Node, buffer: &mut Vec<u8>, results: &mut Vec<String>) {
        if node.end_of_word {
            results.push(String::from_utf8_lossy(buffer).into_owned());
        }

        for (i, child_opt) in node.children.iter().enumerate() {
            if let Some(child) = child_opt {
                buffer.push(b'a' + i as u8);
                self.collect_words_from_node(child, buffer, results);
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
