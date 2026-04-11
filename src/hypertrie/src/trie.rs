use crate::bloom_filter::BloomFilter;

const ALPHABET_SIZE: usize = 26;

pub struct Node {
    letter: char,
    children: [Option<Box<Node>>; ALPHABET_SIZE],
    end_of_word: bool,
}

impl Node {
    fn new(letter: char) -> Self {
        Node {
            letter,
            children: Default::default(),
            end_of_word: false,
        }
    }

    #[inline(always)]
    fn char_to_index(c: char) -> usize {
        (c as u8 - b'a') as usize
    }
}

pub struct Trie {
    root: Box<Node>,
    bloom_filter: BloomFilter,
}

impl Trie {
    pub fn new(size: usize, num_hashes: usize) -> Self {
        Trie {
            root: Box::new(Node::new('\0')),
            bloom_filter: BloomFilter::new(size, num_hashes)
        }
    }

    pub fn insert(&mut self, word: &str) {
        let mut current = &mut self.root;

        for c in word.to_ascii_lowercase().chars() {
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

        for c in word.to_ascii_lowercase().chars() {
            let idx = Node::char_to_index(c);
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
        if node.letter != '\0' {
            println!("{}'{}' (end_of_word: {})", padding, node.letter, node.end_of_word);
        }
        for child in node.children.iter().flatten() {
            self.debug_print(child, indent + 1);
        }
    }

    pub fn words_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut current = &self.root;
        let mut results = Vec::new();
        let mut prefix_accum = String::new();

        for c in prefix.to_ascii_lowercase().chars() {
            let idx = Node::char_to_index(c);
            match &current.children[idx] {
                Some(node) => {
                    prefix_accum.push(c);
                    current = node;
                }
                None => return results,
            }
        }

        Self::collect_words_from_node(current, &mut prefix_accum, &mut results);
        results
    }

    #[inline(always)]
    fn collect_words_from_node(node: &Node, current_word: &mut String, results: &mut Vec<String>) {
        if node.end_of_word {
            results.push(current_word.clone());
        }

        for child_opt in node.children.iter().flatten() {
            let child = child_opt.as_ref();
            current_word.push(child.letter);
            Self::collect_words_from_node(child, current_word, results);
            current_word.pop();
        }
    }
} 