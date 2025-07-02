use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::task;
use crate::trie::Trie;
use anyhow::{Result, anyhow};
use regex::Regex;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct ManDb {
    commands: Vec<String>,
    man_map: HashMap<String, String>,
    man_cache: Arc<Mutex<HashMap<String, Arc<Vec<String>>>>>,
    trie: Arc<Trie>,
}

impl ManDb {
    pub fn load(section: u8) -> Result<Self> {
        let (commands, man_map) = Self::load_man_k(section)?;
        let mut trie = Trie::new();

        for cmd in &commands {
            trie.insert(cmd);
        }

        Ok(Self {
            commands,
            man_map,
            man_cache: Arc::new(Mutex::new(HashMap::new())),
            trie: Arc::new(trie),
        })
    }

    pub fn get_commands(&self) -> &Vec<String> {
        &self.commands
    }

    pub fn commands_starting_with(&self, prefix: &str) -> Vec<String> {
        self.trie.words_starting_with(prefix)
    }

    pub fn display_man_page(&self, command: &str) -> Result<()> {
        Command::new("man")
            .arg(command)
            .stdout(Stdio::inherit())
            .status()?;
        Ok(())
    }

    pub async fn get_man_page(&self, command: &str) -> Arc<Vec<String>> {
        // Проверяем кэш
        {
            let cache = self.man_cache.lock().await;
            if let Some(content) = cache.get(command) {
                return content.clone();
            }
        }

        // Загрузка man-страницы
        let command_str = command.to_string();

        let content = task::spawn_blocking(move || {
            Self::load_man_page(&command_str)
                .unwrap_or_else(|_| vec![format!("Failed to load man page: {}", command_str)])
        }).await.unwrap();

        let content_arc = Arc::new(content);

        // Обновляем кэш
        let mut cache = self.man_cache.lock().await;
        cache.insert(command.to_string(), content_arc.clone());

        content_arc
    }

    fn load_man_k(section: u8) -> Result<(Vec<String>, HashMap<String, String>)> {
        let output = Command::new("man")
            .arg("-k")
            .arg(".")
            .output()?;

        if !output.status.success() {
            return Err(anyhow!("Command failed"));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut man_map = HashMap::new();
        let mut commands = Vec::new();

        let re = Regex::new(r"\((\d)\)").unwrap();

        for line in output_str.lines() {
            if let Some((name, desc)) = line.split_once(" - ") {
                let name_part = name.trim();

                // Извлекаем номер раздела
                let mut section_match = None;
                if let Some(caps) = re.captures(name_part) {
                    if let Some(sec) = caps.get(1) {
                        if sec.as_str().parse::<u8>().unwrap_or(0) == section {
                            section_match = Some(sec.as_str());
                        }
                    }
                }

                // Если номер раздела соответствует фильтру
                if section_match.is_some() {
                    let cleaned_name = name_part
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    if !cleaned_name.is_empty() {
                        man_map.insert(cleaned_name.clone(), desc.trim().to_string());
                        commands.push(cleaned_name);
                    }
                }
            }
        }
        commands.sort_unstable();
        commands.dedup();
        Ok((commands, man_map))
    }

    fn load_man_page(command: &str) -> Result<Vec<String>> {
        let output = Command::new("man")
            .arg(command)
            .env("PAGER", "cat")
            .output()?;

        if !output.status.success() {
            return Err(anyhow!("man command failed"));
        }

        let content = String::from_utf8(output.stdout)?;
        Ok(content.lines().map(|s| s.to_string()).collect())
    }
}