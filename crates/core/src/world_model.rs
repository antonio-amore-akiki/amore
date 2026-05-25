// World model: persistent graph (projects + preferences + tool-reliability + threat-model).

use anyhow::Result;

pub struct WorldModel {}

impl WorldModel {
    pub fn new() -> Self {
        Self {}
    }
    pub async fn query(&self, _key: &str) -> Result<Option<serde_json::Value>> {
        Ok(None)
    }
}

impl Default for WorldModel {
    fn default() -> Self {
        Self::new()
    }
}
