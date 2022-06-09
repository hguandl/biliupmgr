use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RoomConfig {
    pub room_id: u64,
    pub user_cookie: String,
    pub studio_title: String,
    pub part_title: String,
    pub cover: String,
    pub description: String,
    pub tags: String,
    pub tid: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ManagerConfig {
    pub version: u32,
    pub host: String,
    pub port: u16,
    pub rec_dir: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_line")]
    pub line: String,
    pub rooms: HashMap<u64, RoomConfig>,
}

fn default_limit() -> usize {
    3
}

fn default_line() -> String {
    "AUTO".to_string()
}

impl ManagerConfig {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let f = std::fs::File::open(path)?;
        Ok(serde_yaml::from_reader(std::io::BufReader::new(f))?)
    }
}
