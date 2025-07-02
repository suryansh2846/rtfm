use std::collections::HashMap;

#[derive(Default)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    is_word: bool,
}

impl TrieNode {
    fn new() -> Self {
        Self {
            children: HashMap::new(),
            is_word: false,
        }
    }
}

pub struct Trie {
    root: TrieNode,
}

impl Trie {
    pub fn new() -> Self {
        Self {
            root: TrieNode::new(),
        }
    }

    pub fn insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for c in word.chars() {
            node = node.children.entry(c).or_insert_with(TrieNode::new);
        }
        node.is_word = true;
    }

    pub fn words_starting_with(&self, prefix: &str) -> Vec<String> {
        let mut results = Vec::new();
        if let Some(node) = self.get_node(prefix) {
            let mut buffer = prefix.to_string();
            Self::dfs_collect(node, &mut buffer, &mut results);
        }
        results
    }

    fn get_node(&self, prefix: &str) -> Option<&TrieNode> {
        let mut node = &self.root;
        for c in prefix.chars() {
            node = node.children.get(&c)?;
        }
        Some(node)
    }

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