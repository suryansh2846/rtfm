use std::collections::HashMap;

/// Node in the trie structure
struct TrieNode {
    children: HashMap<char, TrieNode>,
    is_word: bool,
}

impl Default for TrieNode {
    fn default() -> Self {
        Self::new()
    }
}

impl TrieNode {
    /// Creates a new TrieNode
    fn new() -> Self {
        Self {
            children: HashMap::new(),
            is_word: false,
        }
    }
}

/// Trie data structure for efficient prefix searches
pub struct Trie {
    root: TrieNode,
}

impl Trie {
    /// Creates a new empty Trie
    pub fn new() -> Self {
        Self {
            root: TrieNode::new(),
        }
    }

    /// Inserts a word into the trie
    pub fn insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for c in word.chars() {
            node = node.children.entry(c).or_insert_with(TrieNode::new);
        }
        node.is_word = true;
    }

    /// Finds all words starting with prefix
    pub fn words_starting_with(&self, prefix: &str) -> Vec<String> {
        let mut results = Vec::new();
        if let Some(node) = self.get_node(prefix) {
            let mut buffer = prefix.to_string();
            Self::dfs_collect(node, &mut buffer, &mut results);
        }
        results
    }

    /// Gets node for given prefix
    fn get_node(&self, prefix: &str) -> Option<&TrieNode> {
        let mut node = &self.root;
        for c in prefix.chars() {
            node = node.children.get(&c)?;
        }
        Some(node)
    }

    /// Depth-first search to collect words
    fn dfs_collect(node: &TrieNode, buffer: &mut String, results: &mut Vec<String>) {
        if node.is_word {
            results.push(buffer.clone());
        }

        for (c, child) in &node.children {
            buffer.push(*c);
            Self::dfs_collect(child, buffer, results);
            buffer.pop();
        }
    }
}
