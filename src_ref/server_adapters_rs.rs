use crate::error::Result;
use async_trait::async_trait;

#[cfg(feature = "redis")]
pub mod redis;

#[cfg(feature = "sqlite")]
pub mod database;

pub mod storage;

/// Generic adapter trait for external services
#[async_trait]
pub trait Adapter: Send + Sync {
    /// Adapter name
    fn name(&self) -> &str;
    
    /// Initialize the adapter
    async fn initialize(&mut self) -> Result<()>;
    
    /// Check if the adapter is healthy/connected
    async fn health_check(&self) -> Result<bool>;
    
    /// Clean up resources
    async fn cleanup(&self) -> Result<()>;
}

/// Connection pool trait for adapters that manage connections
#[async_trait]
pub trait ConnectionPool: Send + Sync {
    type Connection;
    
    /// Get a connection from the pool
    async fn get_connection(&self) -> Result<Self::Connection>;
    
    /// Return a connection to the pool
    async fn return_connection(&self, connection: Self::Connection) -> Result<()>;
    
    /// Get pool statistics
    fn pool_stats(&self) -> PoolStats;
}

/// Pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub active_connections: usize,
    pub idle_connections: usize,
    pub total_connections: usize,
    pub max_connections: usize,
}

/// Adapter registry for managing multiple adapters
pub struct AdapterRegistry {
    adapters: std::collections::HashMap<String, Box<dyn Adapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: std::collections::HashMap::new(),
        }
    }
    
    /// Register an adapter
    pub fn register(&mut self, adapter: Box<dyn Adapter>) {
        let name = adapter.name().to_string();
        self.adapters.insert(name, adapter);
    }
    
    /// Get adapter by name
    pub fn get(&self, name: &str) -> Option<&dyn Adapter> {
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
    
    /// Cleanup all adapters
    pub async fn cleanup_all(&self) -> Result<()> {
        for adapter in self.adapters.values() {
            if let Err(e) = adapter.cleanup().await {
                tracing::warn!("Failed to cleanup adapter {}: {}", adapter.name(), e);
            }
        }
        Ok(())
    }
    
    /// List all registered adapters
    pub fn list_adapters(&self) -> Vec<&str> {
        self.adapters.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ServerError;
    
    struct MockAdapter {
        name: String,
        initialized: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }
    
    impl MockAdapter {
        fn new(name: impl Into<String>) -> Self {
            Self {
                name: name.into(),
                initialized: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            }
        }
    }
    
    #[async_trait]
    impl Adapter for MockAdapter {
        fn name(&self) -> &str {
            &self.name
        }
        
        async fn initialize(&mut self) -> Result<()> {
            self.initialized.store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        }
        
        async fn health_check(&self) -> Result<bool> {
            Ok(self.initialized.load(std::sync::atomic::Ordering::Relaxed))
        }
        
        async fn cleanup(&self) -> Result<()> {
            self.initialized.store(false, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_adapter_registry() {
        let mut registry = AdapterRegistry::new();
        
        // Register adapters
        registry.register(Box::new(MockAdapter::new("test1")));
        registry.register(Box::new(MockAdapter::new("test2")));
        
        // Check registration
        assert_eq!(registry.list_adapters().len(), 2);
        assert!(registry.get("test1").is_some());
        assert!(registry.get("test2").is_some());
        assert!(registry.get("nonexistent").is_none());
        
        // Initialize all
        registry.initialize_all().await.unwrap();
        
        // Health check all
        let health_results = registry.health_check_all().await;
        assert_eq!(health_results.len(), 2);
        assert_eq!(health_results.get("test1"), Some(&true));
        assert_eq!(health_results.get("test2"), Some(&true));
        
        // Cleanup all
        registry.cleanup_all().await.unwrap();
    }
}