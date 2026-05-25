// Multi-agent ensemble + EIG question selection.

use anyhow::Result;

pub struct Ensemble {}

impl Ensemble {
    pub fn new() -> Self {
        Self {}
    }
    pub async fn decide(&self, _prompt: &str) -> Result<Decision> {
        Ok(Decision {
            consensus: String::new(),
            votes: vec![],
            confidence: 0.0,
        })
    }
}

impl Default for Ensemble {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Decision {
    pub consensus: String,
    pub votes: Vec<Vote>,
    pub confidence: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Vote {
    pub agent: String,
    pub position: String,
    pub rationale: String,
    pub weight: f32,
}
