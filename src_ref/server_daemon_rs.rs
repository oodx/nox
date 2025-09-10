use crate::config::Config;
use crate::error::{ServerError, Result};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tokio::fs;
use tokio::time::{sleep, Duration};

/// Process status information
#[derive(Debug, Clone)]
pub struct ProcessStatus {
    pub pid: Option<u32>,
    pub running: bool,
    pub uptime: Option<Duration>,
    pub memory_usage: Option<u64>,
    pub cpu_usage: Option<f32>,
}

/// Daemon manager for controlling server process
pub struct DaemonManager {
    pid_file: PathBuf,
    log_file: PathBuf,
    sock_file: PathBuf,
}

impl DaemonManager {
    pub fn new() -> Result<Self> {
        let base_dir = dirs::runtime_dir()
            .or_else(|| dirs::cache_dir())
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("mini-api-server");
        
        Ok(Self {
            pid_file: base_dir.join("mini-api-server.pid"),
            log_file: base_dir.join("mini-api-server.log"),
            sock_file: base_dir.join("mini-api-server.sock"),
        })
    }
    
    /// Start server as daemon
    pub async fn start_daemon(&self, config: Config) -> Result<()> {
        // Check if already running
        if self.is_running().await? {
            return Err(ServerError::other("Server is already running"));
        }
        
        // Ensure directories exist
        self.ensure_directories().await?;
        
        // Save config to temporary file for daemon process
        let config_file = self.get_temp_config_path();
        config.save_to_file(&config_file)?;
        
        // Start daemon process
        let current_exe = std::env::current_exe()
            .map_err(|e| ServerError::other(format!("Failed to get current executable: {}", e)))?;
        
        let mut cmd = Command::new(current_exe);
        cmd.args(&[
            "--config", config_file.to_str().unwrap(),
            "--foreground",
            "start"
        ]);
        cmd.stdout(Stdio::from(std::fs::File::create(&self.log_file)?));
        cmd.stderr(Stdio::from(std::fs::File::create(&self.log_file)?));
        cmd.stdin(Stdio::null());
        
        // Spawn the daemon process
        let child = cmd.spawn()
            .map_err(|e| ServerError::other(format!("Failed to spawn daemon: {}", e)))?;
        
        let pid = child.id();
        
        // Save PID
        fs::write(&self.pid_file, pid.to_string()).await?;
        
        // Wait a moment and check if process is still running
        sleep(Duration::from_millis(500)).await;
        
        if self.is_running().await? {
            println!("Server started successfully (PID: {})", pid);
            println!("Log file: {:?}", self.log_file);
            Ok(())
        } else {
            Err(ServerError::other("Server failed to start (check logs for details)"))
        }
    }
    
    /// Stop daemon
    pub async fn stop_daemon(&self, force: bool) -> Result<()> {
        if let Some(pid) = self.get_pid().await? {
            if force {
                self.kill_process(pid, true).await?;
                println!("Server forcefully stopped");
            } else {
                self.kill_process(pid, false).await?;
                
                // Wait for graceful shutdown
                for _ in 0..30 {
                    if !self.is_running().await? {
                        println!("Server stopped gracefully");
                        self.cleanup().await?;
                        return Ok(());
                    }
                    sleep(Duration::from_millis(100)).await;
                }
                
                // Force kill if still running
                println!("Server did not stop gracefully, forcing shutdown...");
                self.kill_process(pid, true).await?;
                println!("Server forcefully stopped");
            }
            
            self.cleanup().await?;
        } else {
            println!("Server is not running");
        }
        
        Ok(())
    }
    
    /// Reload configuration
    pub async fn reload_config(&self) -> Result<()> {
        if let Some(pid) = self.get_pid().await? {
            // Send SIGHUP signal for config reload
            self.send_signal(pid, "HUP").await?;
            println!("Configuration reload signal sent to server");
            Ok(())
        } else {
            Err(ServerError::other("Server is not running"))
        }
    }
    
    /// Get server status
    pub async fn get_status(&self) -> Result<ProcessStatus> {
        let pid = self.get_pid().await?;
        let running = self.is_running().await?;
        
        if running && pid.is_some() {
            let uptime = self.get_uptime(pid.unwrap()).await.ok();
            let memory_usage = self.get_memory_usage(pid.unwrap()).await.ok();
            let cpu_usage = self.get_cpu_usage(pid.unwrap()).await.ok();
            
            Ok(ProcessStatus {
                pid,
                running,
                uptime,
                memory_usage,
                cpu_usage,
            })
        } else {
            Ok(ProcessStatus {
                pid,
                running: false,
                uptime: None,
                memory_usage: None,
                cpu_usage: None,
            })
        }
    }
    
    /// Check if server is running
    pub async fn is_running(&self) -> Result<bool> {
        if let Some(pid) = self.get_pid().await? {
            Ok(self.process_exists(pid).await)
        } else {
            Ok(false)
        }
    }
    
    /// Get PID from file
    async fn get_pid(&self) -> Result<Option<u32>> {
        if self.pid_file.exists() {
            let content = fs::read_to_string(&self.pid_file).await?;
            content.trim().parse().map(Some).map_err(|e| {
                ServerError::other(format!("Invalid PID file content: {}", e))
            })
        } else {
            Ok(None)
        }
    }
    
    /// Check if process exists
    async fn process_exists(&self, pid: u32) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            let output = Command::new("kill")
                .args(&["-0", &pid.to_string()])
                .output();
            
            match output {
                Ok(output) => output.status.success(),
                Err(_) => false,
            }
        }
        
        #[cfg(windows)]
        {
            let output = Command::new("tasklist")
                .args(&["/FI", &format!("PID eq {}", pid)])
                .output();
            
            match output {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    stdout.contains(&pid.to_string())
                },
                Err(_) => false,
            }
        }
    }
    
    /// Kill process
    async fn kill_process(&self, pid: u32, force: bool) -> Result<()> {
        #[cfg(unix)]
        {
            let signal = if force { "KILL" } else { "TERM" };
            self.send_signal(pid, signal).await
        }
        
        #[cfg(windows)]
        {
            let args = if force {
                vec!["/F", "/PID", &pid.to_string()]
            } else {
                vec!["/PID", &pid.to_string()]
            };
            
            let output = Command::new("taskkill")
                .args(&args)
                .output()
                .map_err(|e| ServerError::other(format!("Failed to kill process: {}", e)))?;
            
            if !output.status.success() {
                return Err(ServerError::other("Failed to kill process"));
            }
            
            Ok(())
        }
    }
    
    /// Send signal to process (Unix only)
    #[cfg(unix)]
    async fn send_signal(&self, pid: u32, signal: &str) -> Result<()> {
        let output = Command::new("kill")
            .args(&[&format!("-{}", signal), &pid.to_string()])
            .output()
            .map_err(|e| ServerError::other(format!("Failed to send signal: {}", e)))?;
        
        if !output.status.success() {
            return Err(ServerError::other("Failed to send signal"));
        }
        
        Ok(())
    }
    
    #[cfg(windows)]
    async fn send_signal(&self, _pid: u32, _signal: &str) -> Result<()> {
        // Windows doesn't have signals, so this is a no-op
        Ok(())
    }
    
    /// Get process uptime
    async fn get_uptime(&self, pid: u32) -> Result<Duration> {
        #[cfg(unix)]
        {
            let output = Command::new("ps")
                .args(&["-o", "etime=", "-p", &pid.to_string()])
                .output()
                .map_err(|e| ServerError::other(format!("Failed to get uptime: {}", e)))?;
            
            let uptime_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            self.parse_uptime(&uptime_str)
        }
        
        #[cfg(windows)]
        {
            // Simplified for Windows - return 0 for now
            Ok(Duration::from_secs(0))
        }
    }
    
    /// Parse uptime string from ps command
    fn parse_uptime(&self, uptime_str: &str) -> Result<Duration> {
        // ps etime format can be: MM:SS, HH:MM:SS, or DD-HH:MM:SS
        let parts: Vec<&str> = uptime_str.split(':').collect();
        
        match parts.len() {
            2 => {
                // MM:SS
                let minutes: u64 = parts[0].parse().unwrap_or(0);
                let seconds: u64 = parts[1].parse().unwrap_or(0);
                Ok(Duration::from_secs(minutes * 60 + seconds))
            }
            3 => {
                // HH:MM:SS or DD-HH:MM:SS
                if parts[0].contains('-') {
                    // DD-HH:MM:SS
                    let day_hour: Vec<&str> = parts[0].split('-').collect();
                    let days: u64 = day_hour[0].parse().unwrap_or(0);
                    let hours: u64 = day_hour[1].parse().unwrap_or(0);
                    let minutes: u64 = parts[1].parse().unwrap_or(0);
                    let seconds: u64 = parts[2].parse().unwrap_or(0);
                    Ok(Duration::from_secs(days * 86400 + hours * 3600 + minutes * 60 + seconds))
                } else {
                    // HH:MM:SS
                    let hours: u64 = parts[0].parse().unwrap_or(0);
                    let minutes: u64 = parts[1].parse().unwrap_or(0);
                    let seconds: u64 = parts[2].parse().unwrap_or(0);
                    Ok(Duration::from_secs(hours * 3600 + minutes * 60 + seconds))
                }
            }
            _ => Ok(Duration::from_secs(0)),
        }
    }
    
    /// Get memory usage in bytes
    async fn get_memory_usage(&self, pid: u32) -> Result<u64> {
        #[cfg(unix)]
        {
            let output = Command::new("ps")
                .args(&["-o", "rss=", "-p", &pid.to_string()])
                .output()
                .map_err(|e| ServerError::other(format!("Failed to get memory usage: {}", e)))?;
            
            let rss_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let rss_kb: u64 = rss_str.parse().unwrap_or(0);
            Ok(rss_kb * 1024) // Convert KB to bytes
        }
        
        #[cfg(windows)]
        {
            // Simplified for Windows
            Ok(0)
        }
    }
    
    /// Get CPU usage percentage
    async fn get_cpu_usage(&self, pid: u32) -> Result<f32> {
        #[cfg(unix)]
        {
            let output = Command::new("ps")
                .args(&["-o", "pcpu=", "-p", &pid.to_string()])
                .output()
                .map_err(|e| ServerError::other(format!("Failed to get CPU usage: {}", e)))?;
            
            let cpu_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(cpu_str.parse().unwrap_or(0.0))
        }
        
        #[cfg(windows)]
        {
            // Simplified for Windows
            Ok(0.0)
        }
    }
    
    /// Clean up daemon files
    async fn cleanup(&self) -> Result<()> {
        if self.pid_file.exists() {
            fs::remove_file(&self.pid_file).await?;
        }
        Ok(())
    }
    
    /// Ensure necessary directories exist
    async fn ensure_directories(&self) -> Result<()> {
        if let Some(parent) = self.pid_file.parent() {
            fs::create_dir_all(parent).await?;
        }
        Ok(())
    }
    
    /// Get temporary config file path
    fn get_temp_config_path(&self) -> PathBuf {
        self.pid_file.parent().unwrap().join("daemon-config.yaml")
    }
    
    /// Get log file path
    pub fn log_file(&self) -> &PathBuf {
        &self.log_file
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_uptime_parsing() {
        let manager = DaemonManager::new().unwrap();
        
        // Test MM:SS format
        let duration = manager.parse_uptime("05:30").unwrap();
        assert_eq!(duration, Duration::from_secs(5 * 60 + 30));
        
        // Test HH:MM:SS format
        let duration = manager.parse_uptime("01:30:45").unwrap();
        assert_eq!(duration, Duration::from_secs(1 * 3600 + 30 * 60 + 45));
        
        // Test DD-HH:MM:SS format
        let duration = manager.parse_uptime("2-03:15:20").unwrap();
        assert_eq!(duration, Duration::from_secs(2 * 86400 + 3 * 3600 + 15 * 60 + 20));
    }
    
    #[tokio::test]
    async fn test_daemon_manager_creation() {
        let manager = DaemonManager::new().unwrap();
        assert!(manager.pid_file.to_string_lossy().contains("mini-api-server.pid"));
        assert!(manager.log_file.to_string_lossy().contains("mini-api-server.log"));
    }
}