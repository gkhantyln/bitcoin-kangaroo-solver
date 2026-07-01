use serde::{Deserialize, Serialize};

fn default_33() -> Vec<u8> {
    vec![0u8; 33]
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KangarooParams {
    pub puzzle_id: u32,
    pub start_range: [u8; 32],
    pub end_range: [u8; 32],
    pub distinguished_bit: u32,
    pub jump_table_size: usize,
    #[serde(default = "default_33")]
    pub target_point: Vec<u8>,
    pub num_threads: usize,
    pub use_gpu: bool,
    pub checkpoint_interval: u64,
    pub checkpoint_path: Option<String>,
    pub negation_map: bool,
    pub sota_mode: bool,
}

impl KangarooParams {
    pub fn new(
        puzzle_id: u32,
        start_range: [u8; 32],
        end_range: [u8; 32],
        target_point: [u8; 33],
        num_threads: usize,
        distinguished_bit: u32,
        checkpoint_path: Option<String>,
        checkpoint_interval: u64,
    ) -> Self {
        Self {
            puzzle_id,
            start_range,
            end_range,
            distinguished_bit,
            jump_table_size: 256,
            target_point: target_point.to_vec(),
            num_threads,
            use_gpu: false,
            checkpoint_interval,
            checkpoint_path,
            negation_map: true,
            sota_mode: true,
        }
    }

    pub fn with_negation_map(mut self, enabled: bool) -> Self {
        self.negation_map = enabled;
        self
    }

    pub fn with_sota_mode(mut self, enabled: bool) -> Self {
        self.sota_mode = enabled;
        self
    }

    pub fn range_width(&self) -> u128 {
        let mut start_val = 0u128;
        let mut end_val = 0u128;
        for i in 0..16 {
            start_val = (start_val << 8) | self.start_range[16 + i] as u128;
            end_val = (end_val << 8) | self.end_range[16 + i] as u128;
        }
        end_val.saturating_sub(start_val)
    }
}
