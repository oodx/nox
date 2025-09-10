use crate::cli::{DaemonManager, SessionCommand};
use crate::config::Config;
use crate::error::Result;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};

/// Show server status
pub async fn show_status(daemon_manager: &DaemonManager) -> Result<()> {
    let status = daemon_manager.get_status().await?;
    
    println!("Mini API Server Status");
    println!("=====================");
    
    if status.running {
        println!("Status: Running");
        if let Some(pid) = status.pid {
            println!("PID: {}", pid);
        }
        
        if let Some(uptime) = status.uptime {
            let seconds = uptime.as_secs();
            let days = seconds / 86400;
            let hours = (seconds % 86400) / 3600;
            let minutes = (seconds % 3600) / 60;
            let secs = seconds % 60;
            
            if days > 0 {
                println!("Uptime: {}d {}h {}m {}s", days, hours, minutes, secs);
            } else if hours > 0 {
                println!("Uptime: {}h {}m {}s", hours, minutes, secs);
            } else if minutes > 0 {
                println!("Uptime: {}m {}s", minutes, secs);
            } else {
                println!("Uptime: {}s", secs);
            }
        }
        
        if let Some(memory) = status.memory_usage {
            let mb = memory as f64 / 1024.0 / 1024.0;
            println!("Memory: {:.1} MB", mb);
        }
        
        if let Some(cpu) = status.cpu_usage {
            println!("CPU: {:.1}%", cpu);
        }
        
        println!("Log file: {:?}", daemon_manager.log_file());
    } else {
        println!("Status: Not running");
    }
    
    Ok(())
}

/// Show server logs
pub async fn show_logs(follow: bool, lines: usize) -> Result<()> {
    let daemon_manager = DaemonManager::new()?;
    let log_file = daemon_manager.log_file();
    
    if !log_file.exists() {
        println!("Log file does not exist: {:?}", log_file);
        return Ok(());
    }
    
    if follow {
        // Follow log file (tail -f equivalent)
        follow_log_file(log_file, lines).await
    } else {
        // Show last N lines
        show_last_lines(log_file, lines).await
    }
}

/// Follow log file output
async fn follow_log_file(log_file: &PathBuf, initial_lines: usize) -> Result<()> {
    use tokio::time::{sleep, Duration};
    
    // Show initial lines
    show_last_lines(log_file, initial_lines).await?;
    
    let mut last_size = fs::metadata(log_file).await?.len();
    
    println!("Following log file... (Press Ctrl+C to exit)");
    
    loop {
        sleep(Duration::from_millis(500)).await;
        
        if let Ok(metadata) = fs::metadata(log_file).await {
            let current_size = metadata.len();
            
            if current_size > last_size {
                // Read new content
                let file = fs::File::open(log_file).await?;
                let mut reader = BufReader::new(file);
                
                // Skip to the last known position
                let mut buffer = Vec::new();
                tokio::io::AsyncSeekExt::seek(&mut reader, tokio::io::SeekFrom::Start(last_size)).await?;
                
                let mut lines = reader.lines();
                while let Some(line) = lines.next_line().await? {
                    println!("{}", line);
                }
                
                last_size = current_size;
            } else if current_size < last_size {
                // File was truncated or rotated
                println!("Log file was rotated or truncated");
                last_size = current_size;
            }
        }
    }
}

/// Show last N lines of log file
async fn show_last_lines(log_file: &PathBuf, lines: usize) -> Result<()> {
    let content = fs::read_to_string(log_file).await?;
    let all_lines: Vec<&str> = content.lines().collect();
    
    let start_index = if all_lines.len() > lines {
        all_lines.len() - lines
    } else {
        0
    };
    
    for line in &all_lines[start_index..] {
        println!("{}", line);
    }
    
    Ok(())
}

/// Handle configuration commands
pub async fn handle_config_command(
    config: Config,
    show: bool,
    export: bool,
    file: Option<PathBuf>,
) -> Result<()> {
    if export {
        let output_file = file.unwrap_or_else(|| PathBuf::from("mini-api-server.yaml"));
        config.save_to_file(&output_file)?;
        println!("Configuration exported to: {:?}", output_file);
    } else if show {
        let yaml = serde_yaml::to_string(&config)?;
        println!("Current Configuration:");
        println!("=====================");
        println!("{}", yaml);
    } else {
        // Validate configuration
        match config.validate() {
            Ok(()) => println!("Configuration is valid"),
            Err(e) => {
                eprintln!("Configuration validation failed: {}", e);
                std::process::exit(1);
            }
        }
    }
    
    Ok(())
}

/// Show plugin information
pub async fn show_plugins(config: Config, detailed: bool) -> Result<()> {
    println!("Plugin Information");
    println!("==================");
    
    if config.plugins.enabled.is_empty() {
        println!("No plugins enabled");
        return Ok(());
    }
    
    println!("Enabled plugins:");
    for plugin_name in &config.plugins.enabled {
        if detailed {
            println!("  - {}", plugin_name);
            if let Some(plugin_config) = config.plugins.config.get(plugin_name) {
                println!("    Configuration:");
                let yaml = serde_yaml::to_string(plugin_config)?;
                for line in yaml.lines() {
                    println!("      {}", line);
                }
            }
        } else {
            println!("  - {}", plugin_name);
        }
    }
    
    if detailed {
        println!("\nAvailable plugins:");
        println!("  - mock (Mock response generator)");
        println!("  - health (Health check endpoints)");
        println!("  - auth (Authentication provider)");
        println!("  - session (Session management)");
        println!("  - logging (Request/response logging)");
        println!("  - static_files (Static file serving)");
    }
    
    Ok(())
}

/// Show health information
pub async fn show_health(config: Config, detailed: bool) -> Result<()> {
    println!("Health Information");
    println!("==================");
    
    // Try to connect to the server's health endpoint
    let health_url = format!("http://{}{}", config.bind_address(), config.health.path);
    
    match reqwest::get(&health_url).await {
        Ok(response) => {
            if response.status().is_success() {
                let health_data: serde_json::Value = response.json().await
                    .unwrap_or_else(|_| serde_json::json!({"status": "unknown"}));
                
                if detailed {
                    println!("Health endpoint response:");
                    println!("{}", serde_json::to_string_pretty(&health_data)?);
                } else {
                    let status = health_data.get("status")
                        .and_then(|s| s.as_str())
                        .unwrap_or("unknown");
                    println!("Status: {}", status);
                }
            } else {
                println!("Health check failed: HTTP {}", response.status());
            }
        }
        Err(e) => {
            println!("Health check failed: {}", e);
            println!("Server may not be running or health endpoint may be disabled");
        }
    }
    
    Ok(())
}

/// Handle session management commands
pub async fn handle_session_command(config: Config, action: SessionCommand) -> Result<()> {
    // For this implementation, we'd need to create a session manager
    // and connect to the server's session store. This is simplified.
    
    println!("Session Management");
    println!("==================");
    
    match action {
        SessionCommand::List => {
            println!("Active sessions:");
            println!("(Session listing requires server connection - not implemented in CLI)");
        }
        
        SessionCommand::Show { session_id } => {
            println!("Session details for: {}", session_id);
            println!("(Session details require server connection - not implemented in CLI)");
        }
        
        SessionCommand::Delete { session_id } => {
            println!("Deleting session: {}", session_id);
            println!("(Session deletion requires server connection - not implemented in CLI)");
        }
        
        SessionCommand::Cleanup => {
            println!("Cleaning up expired sessions...");
            println!("(Session cleanup requires server connection - not implemented in CLI)");
        }
        
        SessionCommand::Stats => {
            println!("Session statistics:");
            println!("Storage type: {}", config.session.storage);
            println!("Timeout: {}s", config.session.timeout);
            println!("Cookie name: {}", config.session.cookie_name);
            println!("(Detailed stats require server connection - not implemented in CLI)");
        }
    }
    
    Ok(())
}

/// Format duration for human reading
pub fn format_duration(duration: std::time::Duration) -> String {
    let seconds = duration.as_secs();
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    
    if days > 0 {
        format!("{}d {}h {}m {}s", days, hours, minutes, secs)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

/// Format bytes for human reading
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    
    if bytes == 0 {
        return "0 B".to_string();
    }
    
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(3661)), "1h 1m 1s");
        assert_eq!(format_duration(Duration::from_secs(90061)), "1d 1h 1m 1s");
    }
    
    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(1073741824), "1.0 GB");
    }
}