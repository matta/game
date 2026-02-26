use crate::types::{Choice, ChoicePromptId};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputJournal {
    pub format_version: u16,
    pub build_id: String,
    pub content_hash: u64,
    pub seed: u64,
    pub inputs: Vec<InputRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputRecord {
    pub seq: u64,
    pub payload: InputPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum InputPayload {
    Choice { prompt_id: ChoicePromptId, choice: Choice },
    // PolicyUpdate(...) would go here
}

impl InputJournal {
    pub fn new(seed: u64) -> Self {
        Self {
            format_version: 1,
            build_id: "dev".to_string(),
            content_hash: 0,
            seed,
            inputs: Vec::new(),
        }
    }

    pub fn append_choice(&mut self, prompt_id: ChoicePromptId, choice: Choice, seq: u64) {
        self.inputs.push(InputRecord { seq, payload: InputPayload::Choice { prompt_id, choice } });
    }
}
