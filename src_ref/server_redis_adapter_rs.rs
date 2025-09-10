#[cfg(feature = "redis")]
use crate::adapters::Adapter;
#[cfg(feature = "redis")]
use crate::error::{ServerError, Result};
#[cfg(feature = "redis")]
use crate::session::{Session, SessionStore};
#[cfg(feature = "redis")]
use async_trait::async_trait;
#[cfg(feature = "redis")]
use redis::{AsyncCommands, Client, Connection};
#[cfg(feature = "redis")]
use std::time::SystemTime;

#[cfg(feature = "redis")]
/// Redis adapter for caching and session storage
pub struct RedisAdapter {
    client: Client,
    connection: Option<redis::aio::Connection>,
    url: String,
}

#[cfg(feature = "redis")]
impl RedisAdapter {
    pub fn new(url: impl Into<String>) -> Result<Self> {
        let url = url.into();
        let client = Client::open(url.clone())
            .map_err(|e| ServerError::redis(format!("Failed to create Redis client: {}", e)))?;
        
        Ok(Self {
            client,
            connection: None,
            url,
        })
    }
    
    /// Get Redis connection
    async fn get_connection(&mut self) -> Result<&mut redis::aio::Connection> {
        if self.connection.is_none() {
            let conn = self.client.get_async_connection().await
                .map_err(|e| ServerError::redis(format!("Failed to connect to Redis: {}", e)))?;
            self.connection = Some(conn);
        }
        
        Ok(self.connection.as_mut().unwrap())
    }
    
    /// Set a key-value pair with expiration
    pub async fn set_ex(&mut self, key: &str, value: &str, seconds: u64) -> Result<()> {
        let conn = self.get_connection().await?;
        conn.set_ex(key, value, seconds).await
            .map_err(|e| ServerError::redis(format!("Redis SET_EX failed: {}", e)))?;
        Ok(())
    }
    
    /// Get a value by key
    pub async fn get(&mut self, key: &str) -> Result<Option<String>> {
        let conn = self.get_connection().await?;
        let result: Option<String> = conn.get(key).await
            .map_err(|e| ServerError::redis(format!("Redis GET failed: {}", e)))?;
        Ok(result)
    }
    
    /// Delete a key
    pub async fn del(&mut self, key: &str) -> Result<()> {
        let conn = self.get_connection().await?;
        conn.del(key).await
            .map_err(|e| ServerError::redis(format!("Redis DEL failed: {}", e)))?;
        Ok(())
    }
    
    /// Check if key exists
    pub async fn exists(&mut self, key: &str) -> Result<bool> {
        let conn = self.get_connection().await?;
        let result: bool = conn.exists(key).await
            .map_err(|e| ServerError::redis(format!("Redis EXISTS failed: {}", e)))?;
        Ok(result)
    }
    
    /// Get all keys matching pattern
    pub async fn keys(&mut self, pattern: &str) -> Result<Vec<String>> {
        let conn = self.get_connection().await?;
        let result: Vec<String> = conn.keys(pattern).await
            .map_err(|e| ServerError::redis(format!("Redis KEYS failed: {}", e)))?;
        Ok(result)
    }
    
    /// Set TTL for a key
    pub async fn expire(&mut self, key: &str, seconds: u64) -> Result<()> {
        let conn = self.get_connection().await?;
        conn.expire(key, seconds as usize).await
            .map_err(|e| ServerError::redis(format!("Redis EXPIRE failed: {}", e)))?;
        Ok(())
    }
}

#[cfg(feature = "redis")]
#[async_trait]
impl Adapter for RedisAdapter {
    fn name(&self) -> &str {
        "redis"
    }
    
    async fn initialize(&mut self) -> Result<()> {
        // Test connection
        self.get_connection().await?;
        tracing::info!("Redis adapter initialized: {}", self.url);
        Ok(())
    }
    
    async fn health_check(&self) -> Result<bool> {
        // Try to create a new connection for health check
        match self.client.get_async_connection().await {
            Ok(mut conn) => {
                // Try a simple PING command
                match redis::cmd("PING").query_async::<_, String>(&mut conn).await {
                    Ok(response) => Ok(response == "PONG"),
                    Err(_) => Ok(false),
                }
            }
            Err(_) => Ok(false),
        }
    }
    
    async fn cleanup(&self) -> Result<()> {
        // Connection will be dropped automatically
        tracing::info!("Redis adapter cleaned up");
        Ok(())
    }
}

#[cfg(feature = "redis")]
/// Redis-based session store
pub struct RedisSessionStore {
    adapter: RedisAdapter,
    key_prefix: String,
}

#[cfg(feature = "redis")]
impl RedisSessionStore {
    pub async fn new(redis_url: &str) -> Result<Self> {
        let mut adapter = RedisAdapter::new(redis_url)?;
        adapter.initialize().await?;
        
        Ok(Self {
            adapter,
            key_prefix: "session:".to_string(),
        })
    }
    
    /// Get Redis key for session
    fn session_key(&self, session_id: &str) -> String {
        format!("{}{}", self.key_prefix, session_id)
    }
}

#[cfg(feature = "redis")]
#[async_trait]
impl SessionStore for RedisSessionStore {
    async fn get(&self, session_id: &str) -> Result<Option<Session>> {
        let key = self.session_key(session_id);
        
        // This is a simplified approach - we need to make the adapter mutable
        // In a real implementation, you'd use a connection pool
        let mut adapter = RedisAdapter::new(&self.adapter.url)?;
        
        if let Some(session_data) = adapter.get(&key).await? {
            let session: Session = serde_json::from_str(&session_data)?;
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }
    
    async fn save(&self, session: &Session) -> Result<()> {
        let key = self.session_key(&session.id);
        let session_data = serde_json::to_string(session)?;
        
        // Calculate TTL
        let ttl = session.expires_at
            .duration_since(SystemTime::now())
            .unwrap_or_default()
            .as_secs();
        
        let mut adapter = RedisAdapter::new(&self.adapter.url)?;
        adapter.set_ex(&key, &session_data, ttl).await?;
        
        Ok(())
    }
    
    async fn delete(&self, session_id: &str) -> Result<()> {
        let key = self.session_key(session_id);
        let mut adapter = RedisAdapter::new(&self.adapter.url)?;
        adapter.del(&key).await?;
        Ok(())
    }
    
    async fn cleanup_expired(&self) -> Result<usize> {
        // Redis automatically expires keys, so we don't need to do anything
        // Just return 0 for consistency
        Ok(0)
    }
    
    async fn list_sessions(&self) -> Result<Vec<String>> {
        let pattern = format!("{}*", self.key_prefix);
        let mut adapter = RedisAdapter::new(&self.adapter.url)?;
        let keys = adapter.keys(&pattern).await?;
        
        // Extract session IDs from keys
        let session_ids: Vec<String> = keys
            .into_iter()
            .filter_map(|key| {
                if key.starts_with(&self.key_prefix) {
                    Some(key[self.key_prefix.len()..].to_string())
                } else {
                    None
                }
            })
            .collect();
        
        Ok(session_ids)
    }
    
    async fn count(&self) -> Result<usize> {
        let session_ids = self.list_sessions().await?;
        Ok(session_ids.len())
    }
}

#[cfg(feature = "redis")]
/// Redis cache for general purpose caching
pub struct RedisCache {
    adapter: RedisAdapter,
}

#[cfg(feature = "redis")]
impl RedisCache {
    pub async fn new(redis_url: &str) -> Result<Self> {
        let mut adapter = RedisAdapter::new(redis_url)?;
        adapter.initialize().await?;
        
        Ok(Self { adapter })
    }
    
    /// Cache a value with TTL
    pub async fn set<T: serde::Serialize>(&mut self, key: &str, value: &T, ttl_seconds: u64) -> Result<()> {
        let json_value = serde_json::to_string(value)?;
        self.adapter.set_ex(key, &json_value, ttl_seconds).await
    }
    
    /// Get cached value
    pub async fn get<T: for<'de> serde::Deserialize<'de>>(&mut self, key: &str) -> Result<Option<T>> {
        if let Some(json_value) = self.adapter.get(key).await? {
            let value: T = serde_json::from_str(&json_value)?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }
    
    /// Delete cached value
    pub async fn delete(&mut self, key: &str) -> Result<()> {
        self.adapter.del(key).await
    }
    
    /// Check if key exists in cache
    pub async fn exists(&mut self, key: &str) -> Result<bool> {
        self.adapter.exists(key).await
    }
}

// Provide empty implementations when Redis feature is disabled
#[cfg(not(feature = "redis"))]
pub struct RedisAdapter;

#[cfg(not(feature = "redis"))]
impl RedisAdapter {
    pub fn new(_url: impl Into<String>) -> Result<Self> {
        Err(ServerError::other("Redis support not compiled in"))
    }
}

#[cfg(not(feature = "redis"))]
pub struct RedisSessionStore;

#[cfg(not(feature = "redis"))]
impl RedisSessionStore {
    pub async fn new(_redis_url: &str) -> Result<Self> {
        Err(ServerError::other("Redis support not compiled in"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[cfg(feature = "redis")]
    #[tokio::test]
    #[ignore] // Requires Redis server
    async fn test_redis_adapter() {
        let mut adapter = RedisAdapter::new("redis://127.0.0.1:6379").unwrap();
        adapter.initialize().await.unwrap();
        
        // Test basic operations
        adapter.set_ex("test_key", "test_value", 60).await.unwrap();
        
        let value = adapter.get("test_key").await.unwrap();
        assert_eq!(value, Some("test_value".to_string()));
        
        let exists = adapter.exists("test_key").await.unwrap();
        assert!(exists);
        
        adapter.del("test_key").await.unwrap();
        
        let value = adapter.get("test_key").await.unwrap();
        assert_eq!(value, None);
    }
    
    #[cfg(feature = "redis")]
    #[tokio::test]
    #[ignore] // Requires Redis server
    async fn test_redis_session_store() {
        use crate::session::Session;
        use std::time::Duration;
        
        let store = RedisSessionStore::new("redis://127.0.0.1:6379").await.unwrap();
        let session = Session::new("test-session".to_string(), Duration::from_secs(3600));
        
        // Save session
        store.save(&session).await.unwrap();
        
        // Get session
        let retrieved = store.get("test-session").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "test-session");
        
        // Delete session
        store.delete("test-session").await.unwrap();
        let deleted = store.get("test-session").await.unwrap();
        assert!(deleted.is_none());
    }
}