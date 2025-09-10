use crate::adapters::Adapter;
use crate::error::{ServerError, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;

/// File system storage adapter
pub struct FileSystemAdapter {
    base_path: PathBuf,
    initialized: bool,
}

impl FileSystemAdapter {
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
            initialized: false,
        }
    }
    
    /// Ensure a directory exists
    pub async fn ensure_dir<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let full_path = self.base_path.join(path);
        fs::create_dir_all(full_path).await?;
        Ok(())
    }
    
    /// Write data to a file
    pub async fn write_file<P: AsRef<Path>>(&self, path: P, data: &[u8]) -> Result<()> {
        let full_path = self.base_path.join(path);
        
        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        fs::write(full_path, data).await?;
        Ok(())
    }
    
    /// Read data from a file
    pub async fn read_file<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>> {
        let full_path = self.base_path.join(path);
        let data = fs::read(full_path).await?;
        Ok(data)
    }
    
    /// Check if file exists
    pub async fn file_exists<P: AsRef<Path>>(&self, path: P) -> bool {
        let full_path = self.base_path.join(path);
        full_path.exists()
    }
    
    /// Delete a file
    pub async fn delete_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let full_path = self.base_path.join(path);
        if full_path.exists() {
            fs::remove_file(full_path).await?;
        }
        Ok(())
    }
    
    /// List files in a directory
    pub async fn list_files<P: AsRef<Path>>(&self, dir_path: P) -> Result<Vec<PathBuf>> {
        let full_path = self.base_path.join(dir_path);
        let mut entries = fs::read_dir(full_path).await?;
        let mut files = Vec::new();
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                // Return relative path from base_path
                if let Ok(relative) = path.strip_prefix(&self.base_path) {
                    files.push(relative.to_path_buf());
                }
            }
        }
        
        Ok(files)
    }
    
    /// Get file metadata
    pub async fn file_metadata<P: AsRef<Path>>(&self, path: P) -> Result<std::fs::Metadata> {
        let full_path = self.base_path.join(path);
        let metadata = fs::metadata(full_path).await?;
        Ok(metadata)
    }
    
    /// Calculate directory size
    pub async fn directory_size<P: AsRef<Path>>(&self, dir_path: P) -> Result<u64> {
        let full_path = self.base_path.join(dir_path);
        self.calculate_dir_size(&full_path).await
    }
    
    async fn calculate_dir_size(&self, dir: &Path) -> Result<u64> {
        let mut total_size = 0u64;
        let mut entries = fs::read_dir(dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;
            if metadata.is_file() {
                total_size += metadata.len();
            } else if metadata.is_dir() {
                total_size += self.calculate_dir_size(&entry.path()).await?;
            }
        }
        
        Ok(total_size)
    }
    
    /// Clean up old files (older than specified duration)
    pub async fn cleanup_old_files<P: AsRef<Path>>(
        &self,
        dir_path: P,
        max_age: std::time::Duration,
    ) -> Result<usize> {
        let full_path = self.base_path.join(dir_path);
        let cutoff_time = std::time::SystemTime::now() - max_age;
        let mut removed_count = 0;
        
        let mut entries = fs::read_dir(full_path).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;
            if metadata.is_file() {
                if let Ok(modified) = metadata.modified() {
                    if modified < cutoff_time {
                        if fs::remove_file(entry.path()).await.is_ok() {
                            removed_count += 1;
                        }
                    }
                }
            }
        }
        
        Ok(removed_count)
    }
}

#[async_trait]
impl Adapter for FileSystemAdapter {
    fn name(&self) -> &str {
        "filesystem"
    }
    
    async fn initialize(&mut self) -> Result<()> {
        // Ensure base directory exists
        fs::create_dir_all(&self.base_path).await?;
        self.initialized = true;
        tracing::info!("FileSystem adapter initialized: {:?}", self.base_path);
        Ok(())
    }
    
    async fn health_check(&self) -> Result<bool> {
        // Check if base path exists and is writable
        if !self.base_path.exists() {
            return Ok(false);
        }
        
        // Try to write a test file
        let test_file = self.base_path.join(".health_check");
        match fs::write(&test_file, b"test").await {
            Ok(()) => {
                // Clean up test file
                let _ = fs::remove_file(&test_file).await;
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }
    
    async fn cleanup(&self) -> Result<()> {
        // Optionally clean up any temporary files
        tracing::info!("FileSystem adapter cleaned up");
        Ok(())
    }
}

/// In-memory storage adapter (for testing and temporary data)
pub struct MemoryStorageAdapter {
    storage: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, Vec<u8>>>>,
}

impl MemoryStorageAdapter {
    pub fn new() -> Self {
        Self {
            storage: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
    
    /// Store data with a key
    pub async fn put(&self, key: &str, data: Vec<u8>) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.insert(key.to_string(), data);
        Ok(())
    }
    
    /// Retrieve data by key
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let storage = self.storage.read().await;
        Ok(storage.get(key).cloned())
    }
    
    /// Delete data by key
    pub async fn delete(&self, key: &str) -> Result<bool> {
        let mut storage = self.storage.write().await;
        Ok(storage.remove(key).is_some())
    }
    
    /// List all keys
    pub async fn list_keys(&self) -> Result<Vec<String>> {
        let storage = self.storage.read().await;
        Ok(storage.keys().cloned().collect())
    }
    
    /// Get storage size
    pub async fn size(&self) -> Result<usize> {
        let storage = self.storage.read().await;
        Ok(storage.len())
    }
    
    /// Clear all data
    pub async fn clear(&self) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.clear();
        Ok(())
    }
}

#[async_trait]
impl Adapter for MemoryStorageAdapter {
    fn name(&self) -> &str {
        "memory_storage"
    }
    
    async fn initialize(&mut self) -> Result<()> {
        tracing::info!("Memory storage adapter initialized");
        Ok(())
    }
    
    async fn health_check(&self) -> Result<bool> {
        // Memory storage is always healthy if initialized
        Ok(true)
    }
    
    async fn cleanup(&self) -> Result<()> {
        self.clear().await?;
        tracing::info!("Memory storage adapter cleaned up");
        Ok(())
    }
}

impl Default for MemoryStorageAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_files: usize,
    pub total_size: u64,
    pub available_space: Option<u64>,
}

/// Storage manager that can work with different storage backends
pub struct StorageManager {
    adapters: std::collections::HashMap<String, Box<dyn Adapter>>,
    default_adapter: String,
}

impl StorageManager {
    pub fn new() -> Self {
        Self {
            adapters: std::collections::HashMap::new(),
            default_adapter: "filesystem".to_string(),
        }
    }
    
    /// Add a storage adapter
    pub fn add_adapter(&mut self, adapter: Box<dyn Adapter>) {
        let name = adapter.name().to_string();
        self.adapters.insert(name, adapter);
    }
    
    /// Set default adapter
    pub fn set_default_adapter(&mut self, name: &str) {
        self.default_adapter = name.to_string();
    }
    
    /// Get adapter by name
    pub fn get_adapter(&self, name: &str) -> Option<&dyn Adapter> {
        self.adapters.get(name).map(|a| a.as_ref())
    }
    
    /// Initialize all adapters
    pub async fn initialize_all(&mut self) -> Result<()> {
        for adapter in self.adapters.values_mut() {
            adapter.initialize().await?;
        }
        Ok(())
    }
    
    /// Health check all adapters
    pub async fn health_check_all(&self) -> std::collections::HashMap<String, bool> {
        let mut results = std::collections::HashMap::new();
        
        for (name, adapter) in &self.adapters {
            let healthy = adapter.health_check().await.unwrap_or(false);
            results.insert(name.clone(), healthy);
        }
        
        results
    }
}

impl Default for StorageManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_filesystem_adapter() {
        let temp_dir = tempdir().unwrap();
        let mut adapter = FileSystemAdapter::new(temp_dir.path());
        
        adapter.initialize().await.unwrap();
        
        // Test writing and reading
        let test_data = b"Hello, World!";
        adapter.write_file("test.txt", test_data).await.unwrap();
        
        let read_data = adapter.read_file("test.txt").await.unwrap();
        assert_eq!(read_data, test_data);
        
        // Test file existence
        assert!(adapter.file_exists("test.txt").await);
        assert!(!adapter.file_exists("nonexistent.txt").await);
        
        // Test listing files
        let files = adapter.list_files(".").await.unwrap();
        assert!(files.contains(&PathBuf::from("test.txt")));
        
        // Test deletion
        adapter.delete_file("test.txt").await.unwrap();
        assert!(!adapter.file_exists("test.txt").await);
        
        // Test health check
        assert!(adapter.health_check().await.unwrap());
    }
    
    #[tokio::test]
    async fn test_memory_storage_adapter() {
        let mut adapter = MemoryStorageAdapter::new();
        
        adapter.initialize().await.unwrap();
        
        // Test putting and getting
        let test_data = b"Hello, Memory!".to_vec();
        adapter.put("test_key", test_data.clone()).await.unwrap();
        
        let retrieved_data = adapter.get("test_key").await.unwrap();
        assert_eq!(retrieved_data, Some(test_data));
        
        // Test listing keys
        let keys = adapter.list_keys().await.unwrap();
        assert!(keys.contains(&"test_key".to_string()));
        
        // Test size
        let size = adapter.size().await.unwrap();
        assert_eq!(size, 1);
        
        // Test deletion
        let deleted = adapter.delete("test_key").await.unwrap();
        assert!(deleted);
        
        let after_delete = adapter.get("test_key").await.unwrap();
        assert_eq!(after_delete, None);
        
        // Test health check
        assert!(adapter.health_check().await.unwrap());
    }
    
    #[tokio::test]
    async fn test_storage_manager() {
        let mut manager = StorageManager::new();
        
        // Add adapters
        manager.add_adapter(Box::new(MemoryStorageAdapter::new()));
        
        // Initialize all
        manager.initialize_all().await.unwrap();
        
        // Health check all
        let health_results = manager.health_check_all().await;
        assert_eq!(health_results.get("memory_storage"), Some(&true));
        
        // Get adapter
        let adapter = manager.get_adapter("memory_storage");
        assert!(adapter.is_some());
        assert_eq!(adapter.unwrap().name(), "memory_storage");
    }
}