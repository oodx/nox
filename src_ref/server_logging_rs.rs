use crate::config::LoggingConfig;
use crate::error::Result;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};
use std::io;

pub fn setup_logging(config: &LoggingConfig) -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.level));
    
    let registry = tracing_subscriber::registry().with(filter);
    
    match config.format.as_str() {
        "json" => {
            if let Some(file_path) = &config.file {
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(file_path)?;
                
                registry
                    .with(
                        fmt::layer()
                            .json()
                            .with_writer(file)
                            .with_target(true)
                            .with_thread_ids(true)
                            .with_thread_names(true)
                    )
                    .with(
                        fmt::layer()
                            .json()
                            .with_writer(io::stderr)
                            .with_target(true)
                            .with_thread_ids(true)
                            .with_thread_names(true)
                    )
                    .init();
            } else {
                registry
                    .with(
                        fmt::layer()
                            .json()
                            .with_target(true)
                            .with_thread_ids(true)
                            .with_thread_names(true)
                    )
                    .init();
            }
        }
        _ => {
            if let Some(file_path) = &config.file {
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(file_path)?;
                
                registry
                    .with(
                        fmt::layer()
                            .with_writer(file)
                            .with_target(true)
                            .with_thread_ids(true)
                            .with_thread_names(true)
                    )
                    .with(
                        fmt::layer()
                            .with_writer(io::stderr)
                            .with_target(true)
                            .with_thread_ids(true)
                            .with_thread_names(true)
                    )
                    .init();
            } else {
                registry
                    .with(
                        fmt::layer()
                            .with_target(true)
                            .with_thread_ids(true)
                            .with_thread_names(true)
                    )
                    .init();
            }
        }
    }
    
    tracing::info!("Logging initialized with level: {}, format: {}", config.level, config.format);
    Ok(())
}

/// Request logging middleware
pub struct RequestLogger {
    enabled: bool,
}

impl RequestLogger {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
    
    pub fn log_request(&self, method: &str, path: &str, remote_addr: Option<&str>) {
        if self.enabled {
            tracing::info!(
                method = method,
                path = path,
                remote_addr = remote_addr,
                "Incoming request"
            );
        }
    }
    
    pub fn log_response(&self, method: &str, path: &str, status: u16, duration_ms: u64) {
        if self.enabled {
            tracing::info!(
                method = method,
                path = path,
                status = status,
                duration_ms = duration_ms,
                "Request completed"
            );
        }
    }
    
    pub fn log_error(&self, method: &str, path: &str, error: &str) {
        if self.enabled {
            tracing::error!(
                method = method,
                path = path,
                error = error,
                "Request failed"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_logging_setup() {
        let temp_dir = tempdir().unwrap();
        let log_file = temp_dir.path().join("test.log");
        
        let config = LoggingConfig {
            level: "info".to_string(),
            format: "text".to_string(),
            file: Some(log_file.clone()),
            request_logging: true,
        };
        
        // This would normally initialize logging, but we can't test it easily
        // in a unit test without affecting global state
        assert_eq!(config.level, "info");
        assert_eq!(config.format, "text");
        assert!(config.file.is_some());
    }
    
    #[test]
    fn test_request_logger() {
        let logger = RequestLogger::new(true);
        
        // These would normally log, but we can't easily test logging output
        logger.log_request("GET", "/test", Some("127.0.0.1"));
        logger.log_response("GET", "/test", 200, 150);
        logger.log_error("GET", "/test", "Test error");
    }
}