use crate::error::{ServerError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use notify::{Watcher, RecursiveMode, watcher, DebouncedEvent};
use std::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub logging: LoggingConfig,
    pub plugins: PluginConfig,
    pub mock: MockConfig,
    pub auth: AuthConfig,
    pub session: SessionConfig,
    pub static_files: Option<StaticFilesConfig>,
    pub proxy: Option<ProxyConfig>,
    pub health: HealthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: Option<usize>,
    pub max_connections: Option<usize>,
    pub request_timeout: Option<u64>, // seconds
    pub keep_alive_timeout: Option<u64>, // seconds
    pub tls: Option<TlsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String, // "json" or "text"
    pub file: Option<PathBuf>,
    pub request_logging: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub enabled: Vec<String>,
    pub config: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockConfig {
    pub scenarios: Vec<MockScenario>,
    pub default_delay: Option<u64>, // milliseconds
    pub record_requests: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockScenario {
    pub name: String,
    pub enabled: Option<bool>,
    pub routes: Vec<MockRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockRoute {
    pub path: String,
    pub method: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub query: Option<HashMap<String, String>>,
    pub body_pattern: Option<String>,
    pub response: MockResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockResponse {
    pub status: u16,
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<String>,
    pub body_file: Option<PathBuf>,
    pub delay: Option<u64>, // milliseconds
    pub template: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub strategy: String, // "none", "basic", "bearer", "api_key"
    pub realm: Option<String>,
    pub users: Option<HashMap<String, String>>, // username -> password/token
    pub api_keys: Option<Vec<String>>,
    pub header_name: Option<String>, // for api_key strategy
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub storage: String, // "memory", "file", "sqlite", "redis"
    pub timeout: u64, // seconds
    pub cookie_name: String,
    pub cookie_secure: bool,
    pub cookie_http_only: bool,
    pub storage_config: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticFilesConfig {
    pub enabled: bool,
    pub root_dir: PathBuf,
    pub index_files: Vec<String>,
    pub cache_control: Option<String>,
    pub prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub upstreams: Vec<UpstreamConfig>,
    pub timeout: Option<u64>, // seconds
    pub retry_attempts: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamConfig {
    pub name: String,
    pub url: String,
    pub weight: Option<u32>,
    pub health_check: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    pub enabled: bool,
    pub path: String,
    pub detailed: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                workers: None,
                max_connections: None,
                request_timeout: Some(30),
                keep_alive_timeout: Some(60),
                tls: None,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "text".to_string(),
                file: None,
                request_logging: true,
            },
            plugins: PluginConfig {
                enabled: vec!["mock".to_string(), "health".to_string()],
                config: HashMap::new(),
            },
            mock: MockConfig {
                scenarios: Vec::new(),
                default_delay: None,
                record_requests: false,
            },
            auth: AuthConfig {
                strategy: "none".to_string(),
                realm: None,
                users: None,
                api_keys: None,
                header_name: None,
            },
            session: SessionConfig {
                storage: "memory".to_string(),
                timeout: 3600, // 1 hour
                cookie_name: "session_id".to_string(),
                cookie_secure: false,
                cookie_http_only: true,
                storage_config: HashMap::new(),
            },
            static_files: Some(StaticFilesConfig {
                enabled: false,
                root_dir: PathBuf::from("./static"),
                index_files: vec!["index.html".to_string()],
                cache_control: Some("public, max-age=3600".to_string()),
                prefix: None,
            }),
            proxy: Some(ProxyConfig {
                enabled: false,
                upstreams: Vec::new(),
                timeout: Some(30),
                retry_attempts: Some(3),
            }),
            health: HealthConfig {
                enabled: true,
                path: "/health".to_string(),
                detailed: false,
            },
        }
    }
}

impl Config {
    /// Load configuration from file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let config: Config = serde_yaml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }
    
    /// Load configuration from multiple sources with priority
    pub fn load() -> Result<Self> {
        let mut config = Config::default();
        
        // Try loading from various locations
        let config_paths = [
            "./mini-api-server.yaml",
            "./config.yaml",
            dirs::config_dir()
                .map(|d| d.join("mini-api-server").join("config.yaml"))
                .unwrap_or_default(),
        ];
        
        for path in &config_paths {
            if path.exists() {
                tracing::info!("Loading configuration from {:?}", path);
                config = Self::load_from_file(path)?;
                break;
            }
        }
        
        // Override with environment variables
        config.apply_env_overrides();
        
        Ok(config)
    }
    
    /// Save configuration to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path.as_ref(), content)?;
        Ok(())
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate server config
        if self.server.port == 0 {
            return Err(ServerError::config("Server port cannot be 0"));
        }
        
        // Validate auth config
        match self.auth.strategy.as_str() {
            "none" | "basic" | "bearer" | "api_key" => {},
            _ => return Err(ServerError::config(format!(
                "Unknown auth strategy: {}", self.auth.strategy
            ))),
        }
        
        // Validate session storage
        match self.session.storage.as_str() {
            "memory" | "file" | "sqlite" | "redis" => {},
            _ => return Err(ServerError::config(format!(
                "Unknown session storage: {}", self.session.storage
            ))),
        }
        
        // Validate logging level
        match self.logging.level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {},
            _ => return Err(ServerError::config(format!(
                "Unknown logging level: {}", self.logging.level
            ))),
        }
        
        Ok(())
    }
    
    /// Apply environment variable overrides
    fn apply_env_overrides(&mut self) {
        if let Ok(host) = std::env::var("SERVER_HOST") {
            self.server.host = host;
        }
        
        if let Ok(port) = std::env::var("SERVER_PORT") {
            if let Ok(port) = port.parse() {
                self.server.port = port;
            }
        }
        
        if let Ok(level) = std::env::var("LOG_LEVEL") {
            self.logging.level = level;
        }
        
        if let Ok(auth) = std::env::var("AUTH_STRATEGY") {
            self.auth.strategy = auth;
        }
    }
    
    /// Get bind address
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
    
    /// Get request timeout as Duration
    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.server.request_timeout.unwrap_or(30))
    }
    
    /// Get keep alive timeout as Duration  
    pub fn keep_alive_timeout(&self) -> Duration {
        Duration::from_secs(self.server.keep_alive_timeout.unwrap_or(60))
    }
    
    /// Get session timeout as Duration
    pub fn session_timeout(&self) -> Duration {
        Duration::from_secs(self.session.timeout)
    }
}

/// Configuration manager with hot reload support
pub struct ConfigManager {
    config: Arc<RwLock<Config>>,
    _watcher: Option<notify::RecommendedWatcher>,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            _watcher: None,
        }
    }
    
    /// Create with hot reload support
    pub fn with_hot_reload(config: Config, config_path: impl AsRef<Path>) -> Result<Self> {
        let config_arc = Arc::new(RwLock::new(config));
        let config_clone = Arc::clone(&config_arc);
        let path = config_path.as_ref().to_path_buf();
        
        let (tx, rx) = mpsc::channel();
        let mut watcher = watcher(tx, Duration::from_secs(2))?;
        watcher.watch(&path, RecursiveMode::NonRecursive)?;
        
        // Spawn file watcher task
        let _handle = tokio::spawn(async move {
            while let Ok(event) = rx.recv() {
                match event {
                    DebouncedEvent::Write(_) | DebouncedEvent::Create(_) => {
                        match Config::load_from_file(&path) {
                            Ok(new_config) => {
                                tracing::info!("Configuration reloaded from {:?}", path);
                                let mut config = config_clone.write().await;
                                *config = new_config;
                            }
                            Err(e) => {
                                tracing::error!("Failed to reload configuration: {}", e);
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
        
        Ok(Self {
            config: config_arc,
            _watcher: Some(watcher),
        })
    }
    
    /// Get current configuration
    pub async fn get(&self) -> Config {
        self.config.read().await.clone()
    }
    
    /// Update configuration
    pub async fn update(&self, config: Config) {
        let mut current = self.config.write().await;
        *current = config;
    }
    
    /// Get configuration reference for reading
    pub fn config(&self) -> Arc<RwLock<Config>> {
        Arc::clone(&self.config)
    }
}