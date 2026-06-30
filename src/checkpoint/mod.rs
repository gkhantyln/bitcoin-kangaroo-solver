use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_33() -> Vec<u8> {
    vec![0u8; 33]
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Checkpoint {
    pub puzzle_id: u32,
    pub start_range: [u8; 32],
    pub end_range: [u8; 32],
    #[serde(default = "default_33")]
    pub target_point: Vec<u8>,
    pub distinguished_points: Vec<DistinguishedPointEntry>,
    pub elapsed_seconds: u64,
    pub total_ops: u128,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DistinguishedPointEntry {
    pub x: [u8; 32],
    pub distance: [u8; 32],
    pub kangaroo_type: u8,
    pub thread_id: u32,
}

impl Checkpoint {
    pub fn new() -> Self {
        Self {
            puzzle_id: 0,
            start_range: [0u8; 32],
            end_range: [0u8; 32],
            target_point: vec![0u8; 33],
            distinguished_points: Vec::new(),
            elapsed_seconds: 0,
            total_ops: 0,
            timestamp: 0,
        }
    }

    pub fn save(&self, path: &PathBuf) -> Result<(), String> {
        let data = bincode::serialize(self).map_err(|e| format!("Serialization error: {}", e))?;
        std::fs::write(path, data).map_err(|e| format!("Write error: {}", e))?;
        Ok(())
    }

    pub fn load(path: &PathBuf) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|e| format!("Read error: {}", e))?;
        bincode::deserialize(&data).map_err(|e| format!("Deserialization error: {}", e))
    }
}

impl Default for Checkpoint {
    fn default() -> Self {
        Self::new()
    }
}
