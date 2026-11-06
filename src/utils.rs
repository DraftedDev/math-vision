use std::path::Path;

use serde::{Deserialize, Serialize};

pub fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for line in text.split('\n') {
        let mut current_line = String::new();
        for word in line.split_whitespace() {
            if current_line.len() + word.len() + 1 > max_chars {
                lines.push(current_line);
                current_line = word.to_string();
            } else {
                if !current_line.is_empty() {
                    current_line.push(' ');
                }
                current_line.push_str(word);
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    lines
}

pub trait Config: Sized + Default + Serialize + for<'a> Deserialize<'a> {
    fn load(p: impl AsRef<Path>) -> anyhow::Result<Self> {
        let p = p.as_ref();

        if p.exists() {
            let data = std::fs::read(p)?;

            Ok(serde_json::from_slice(&data)?)
        } else {
            let default = Self::default();

            default.save(p)?;

            Self::load(p)
        }
    }

    fn save(&self, p: impl AsRef<Path>) -> anyhow::Result<()> {
        let data = serde_json::to_string(self)?;
        std::fs::write(p, data)?;

        Ok(())
    }
}
