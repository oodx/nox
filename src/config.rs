use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NoxConfig {
    pub server: ServerConfig,
    pub mock: Option<MockConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MockConfig {
    pub scenarios: Vec<MockScenario>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MockScenario {
    pub name: String,
    pub routes: Vec<MockRoute>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MockRoute {
    pub path: String,
    pub method: String,
    pub response: MockResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MockResponse {
    pub status: u16,
    pub headers: Option<HashMap<String, String>>,
    pub body: String,
}

impl Default for NoxConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 3000,
            },
            mock: None,
        }
    }
}

impl NoxConfig {
    pub fn from_yaml(content: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(content)
    }

    pub fn load_from_file(path: &str) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_yaml(&content)?)
    }
}