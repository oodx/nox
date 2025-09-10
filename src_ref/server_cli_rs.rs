use crate::config::Config;
use crate::error::{ServerError, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

pub mod daemon;
pub mod commands;

pub use daemon::DaemonManager;
pub use commands::*;

/// Mini API Server CLI
#[derive(Parser)]
#[command(name = "mini-api-server")]
#[command(about = "A lightweight, modular HTTP API server")]
#[command(version)]
pub struct Cli {
    /// Configuration file path
    #[arg(short, long)]
    pub config: Option<PathBuf>,
    
    /// Log level override
    #[arg(short, long)]
    pub log_level: Option<String>,
    
    /// Bind address override
    #[arg(short, long)]
    pub bind: Option<String>,
    
    /// Port override
    #[arg(short, long)]
    pub port: Option<u16>,
    
    /// Run in foreground (don't daemonize)
    #[arg(short, long)]
    pub foreground: bool,
    
    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,
    
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the server
    Start {
        /// Run in foreground mode
        #[arg(short, long)]
        foreground: bool,
    },
    
    /// Stop the server
    Stop {
        /// Force stop (SIGKILL)
        #[arg(short, long)]
        force: bool,
    },
    
    /// Restart the server
    Restart {
        /// Run in foreground mode after restart
        #[arg(short, long)]
        foreground: bool,
    },
    
    /// Show server status
    Status,
    
    /// Reload configuration
    Reload,
    
    /// Show server logs
    Logs {
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
        
        /// Number of lines to show
        #[arg(short, long, default_value = "100")]
        lines: usize,
    },
    
    /// Validate configuration
    Config {
        /// Show effective configuration
        #[arg(short, long)]
        show: bool,
        
        /// Export default configuration
        #[arg(short, long)]
        export: bool,
        
        /// Configuration file to validate/export
        #[arg(short, long)]
        file: Option<PathBuf>,
    },
    
    /// List plugins and their status
    Plugins {
        /// Show detailed plugin information
        #[arg(short, long)]
        detailed: bool,
    },
    
    /// Show health information
    Health {
        /// Show detailed health information
        #[arg(short, long)]
        detailed: bool,
    },
    
    /// Manage sessions
    Sessions {
        #[command(subcommand)]
        action: SessionCommand,
    },
}

#[derive(Subcommand)]
pub enum SessionCommand {
    /// List active sessions
    List,
    
    /// Show session details
    Show {
        /// Session ID
        session_id: String,
    },
    
    /// Delete a session
    Delete {
        /// Session ID
        session_id: String,
    },
    
    /// Clean up expired sessions
    Cleanup,
    
    /// Show session statistics
    Stats,
}

impl Cli {
    /// Parse command line arguments
    pub fn parse_args() -> Self {
        Self::parse()
    }
    
    /// Load configuration with CLI overrides
    pub fn load_config(&self) -> Result<Config> {
        let mut config = if let Some(config_path) = &self.config {
            Config::load_from_file(config_path)?
        } else {
            Config::load()?
        };
        
        // Apply CLI overrides
        if let Some(log_level) = &self.log_level {
            config.logging.level = log_level.clone();
        }
        
        if let Some(bind) = &self.bind {
            config.server.host = bind.clone();
        }
        
        if let Some(port) = self.port {
            config.server.port = port;
        }
        
        // Validate configuration
        config.validate()?;
        
        Ok(config)
    }
    
    /// Check if running in foreground mode
    pub fn is_foreground(&self) -> bool {
        self.foreground || matches!(self.command, Commands::Start { foreground: true } | Commands::Restart { foreground: true })
    }
    
    /// Check if verbose mode is enabled
    pub fn is_verbose(&self) -> bool {
        self.verbose
    }
}

/// CLI application runner
pub struct CliApp {
    cli: Cli,
    daemon_manager: DaemonManager,
}

impl CliApp {
    pub fn new() -> Result<Self> {
        let cli = Cli::parse_args();
        let daemon_manager = DaemonManager::new()?;
        
        Ok(Self {
            cli,
            daemon_manager,
        })
    }
    
    /// Run the CLI application
    pub async fn run(self) -> Result<()> {
        let config = self.cli.load_config()?;
        
        // Setup logging based on CLI settings
        if self.cli.is_verbose() {
            println!("Configuration loaded from: {:?}", 
                self.cli.config.as_ref().unwrap_or(&PathBuf::from("default")));
            println!("Server will bind to: {}", config.bind_address());
        }
        
        match self.cli.command {
            Commands::Start { foreground } => {
                if foreground || self.cli.foreground {
                    start_foreground(config).await
                } else {
                    self.daemon_manager.start_daemon(config).await
                }
            }
            
            Commands::Stop { force } => {
                self.daemon_manager.stop_daemon(force).await
            }
            
            Commands::Restart { foreground } => {
                self.daemon_manager.stop_daemon(false).await?;
                
                if foreground || self.cli.foreground {
                    start_foreground(config).await
                } else {
                    self.daemon_manager.start_daemon(config).await
                }
            }
            
            Commands::Status => {
                show_status(&self.daemon_manager).await
            }
            
            Commands::Reload => {
                self.daemon_manager.reload_config().await
            }
            
            Commands::Logs { follow, lines } => {
                show_logs(follow, lines).await
            }
            
            Commands::Config { show, export, file } => {
                handle_config_command(config, show, export, file).await
            }
            
            Commands::Plugins { detailed } => {
                show_plugins(config, detailed).await
            }
            
            Commands::Health { detailed } => {
                show_health(config, detailed).await
            }
            
            Commands::Sessions { action } => {
                handle_session_command(config, action).await
            }
        }
    }
}

/// Start server in foreground mode
async fn start_foreground(config: Config) -> Result<()> {
    println!("Starting Mini API Server in foreground mode...");
    println!("Server will bind to: {}", config.bind_address());
    println!("Press Ctrl+C to stop");
    
    // Setup graceful shutdown
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
        println!("\nShutdown signal received, stopping server...");
        let _ = shutdown_tx.send(());
    });
    
    // Initialize server
    let server = crate::Server::new(config)?;
    
    // Start server in a separate task
    let server_handle = {
        let server = server.clone();
        tokio::spawn(async move {
            if let Err(e) = server.start().await {
                eprintln!("Server error: {}", e);
            }
        })
    };
    
    // Wait for shutdown signal
    let _ = shutdown_rx.await;
    
    // Graceful shutdown
    server.shutdown().await?;
    server_handle.abort();
    
    println!("Server stopped");
    Ok(())
}

impl Default for CliApp {
    fn default() -> Self {
        Self::new().expect("Failed to create CLI app")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cli_parsing() {
        // Test basic command parsing
        let cli = Cli::try_parse_from(&["mini-api-server", "start"]).unwrap();
        assert!(matches!(cli.command, Commands::Start { .. }));
        
        let cli = Cli::try_parse_from(&["mini-api-server", "stop"]).unwrap();
        assert!(matches!(cli.command, Commands::Stop { .. }));
        
        let cli = Cli::try_parse_from(&["mini-api-server", "status"]).unwrap();
        assert!(matches!(cli.command, Commands::Status));
    }
    
    #[test]
    fn test_cli_options() {
        let cli = Cli::try_parse_from(&[
            "mini-api-server",
            "--config", "/path/to/config.yaml",
            "--port", "9090",
            "--foreground",
            "start"
        ]).unwrap();
        
        assert_eq!(cli.config, Some(PathBuf::from("/path/to/config.yaml")));
        assert_eq!(cli.port, Some(9090));
        assert!(cli.foreground);
    }
    
    #[test]
    fn test_session_commands() {
        let cli = Cli::try_parse_from(&["mini-api-server", "sessions", "list"]).unwrap();
        if let Commands::Sessions { action } = cli.command {
            assert!(matches!(action, SessionCommand::List));
        } else {
            panic!("Expected Sessions command");
        }
        
        let cli = Cli::try_parse_from(&["mini-api-server", "sessions", "show", "session-123"]).unwrap();
        if let Commands::Sessions { action } = cli.command {
            if let SessionCommand::Show { session_id } = action {
                assert_eq!(session_id, "session-123");
            } else {
                panic!("Expected Show command");
            }
        } else {
            panic!("Expected Sessions command");
        }
    }
}