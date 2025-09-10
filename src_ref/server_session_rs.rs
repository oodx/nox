use crate::error::{ServerError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

pub mod memory;
pub mod file;
#[cfg(feature = "sqlite")]
pub mod sqlite;

pub use memory::MemorySessionStore;
pub use file::FileSessionStore;
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteSessionStore;

/// Session data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: Option<String>,
    pub data: HashMap<String, serde_json::Value>,
    pub created_at: SystemTime,
    pub expires_at: SystemTime,
    pub last_accessed: SystemTime,
}

impl Session {
    pub fn new(id: String, timeout: Duration) -> Self {
        let now = SystemTime::now();
        Self {
            id,
            user_id: None,
            data: HashMap::new(),
            created_at: now,
            expires_at: now + timeout,
            last_accessed: now,
        }
    }
    
    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.expires_at
    }
    
    pub fn refresh(&mut self, timeout: Duration) {
        let now = SystemTime::now();
        self.last_accessed = now;
        self.expires_at = now + timeout;
    }
    
    pub fn get<T>(&self, key: &str) -> Option<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.data.get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
    
    pub fn set<T>(&mut self, key: &str, value: T) -> Result<()>
    where
        T: Serialize,
    {
        let json_value = serde_json::to_value(value)?;
        self.data.insert(key.to_string(), json_value);
        Ok(())
    }
    
    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        self.data.remove(key)
    }
    
    pub fn clear(&mut self) {
        self.data.clear();
    }
}

/// Session store trait for different storage backends
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Get session by ID
    async fn get(&self, session_id: &str) -> Result<Option<Session>>;
    
    /// Save or update session
    async fn save(&self, session: &Session) -> Result<()>;
    
    /// Delete session
    async fn delete(&self, session_id: &str) -> Result<()>;
    
    /// Clean up expired sessions
    async fn cleanup_expired(&self) -> Result<usize>;
    
    /// List all sessions (for debugging/admin)
    async fn list_sessions(&self) -> Result<Vec<String>>;
    
    /// Get session count
    async fn count(&self) -> Result<usize>;
}

/// Session manager that handles session lifecycle
pub struct SessionManager {
    store: Box<dyn SessionStore>,
    timeout: Duration,
    cookie_name: String,
    cookie_secure: bool,
    cookie_http_only: bool,
}

impl SessionManager {
    pub fn new(
        store: Box<dyn SessionStore>,
        timeout: Duration,
        cookie_name: String,
        cookie_secure: bool,
        cookie_http_only: bool,
    ) -> Self {
        Self {
            store,
            timeout,
            cookie_name,
            cookie_secure,
            cookie_http_only,
        }
    }
    
    /// Create a new session
    pub async fn create_session(&self) -> Result<Session> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let session = Session::new(session_id, self.timeout);
        self.store.save(&session).await?;
        Ok(session)
    }
    
    /// Get session by ID
    pub async fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        if let Some(mut session) = self.store.get(session_id).await? {
            if session.is_expired() {
                self.store.delete(session_id).await?;
                return Ok(None);
            }
            
            // Refresh session
            session.refresh(self.timeout);
            self.store.save(&session).await?;
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }
    
    /// Save session
    pub async fn save_session(&self, session: &Session) -> Result<()> {
        self.store.save(session).await
    }
    
    /// Delete session
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        self.store.delete(session_id).await
    }
    
    /// Get session from request cookies
    pub async fn get_session_from_request(&self, request: &hyper::Request<hyper::Body>) -> Result<Option<Session>> {
        if let Some(cookie_header) = request.headers().get("cookie") {
            if let Ok(cookie_str) = cookie_header.to_str() {
                for cookie_pair in cookie_str.split(';') {
                    let cookie_pair = cookie_pair.trim();
                    if let Some((name, value)) = cookie_pair.split_once('=') {
                        if name.trim() == self.cookie_name {
                            return self.get_session(value.trim()).await;
                        }
                    }
                }
            }
        }
        Ok(None)
    }
    
    /// Add session cookie to response
    pub fn add_session_cookie(&self, response: &mut hyper::Response<hyper::Body>, session: &Session) -> Result<()> {
        let mut cookie = cookie::Cookie::new(&self.cookie_name, &session.id);
        cookie.set_http_only(self.cookie_http_only);
        cookie.set_secure(self.cookie_secure);
        cookie.set_path("/");
        
        // Set expiration
        if let Ok(duration) = session.expires_at.duration_since(SystemTime::UNIX_EPOCH) {
            let expiry = time::OffsetDateTime::from_unix_timestamp(duration.as_secs() as i64)
                .unwrap_or_else(|_| time::OffsetDateTime::now_utc());
            cookie.set_expires(expiry);
        }
        
        response.headers_mut().append(
            "set-cookie",
            cookie.to_string().parse()
                .map_err(|e| ServerError::session(format!("Invalid cookie: {}", e)))?
        );
        
        Ok(())
    }
    
    /// Remove session cookie from response
    pub fn remove_session_cookie(&self, response: &mut hyper::Response<hyper::Body>) -> Result<()> {
        let mut cookie = cookie::Cookie::new(&self.cookie_name, "");
        cookie.set_http_only(self.cookie_http_only);
        cookie.set_secure(self.cookie_secure);
        cookie.set_path("/");
        cookie.set_expires(time::OffsetDateTime::UNIX_EPOCH);
        
        response.headers_mut().append(
            "set-cookie",
            cookie.to_string().parse()
                .map_err(|e| ServerError::session(format!("Invalid cookie: {}", e)))?
        );
        
        Ok(())
    }
    
    /// Clean up expired sessions
    pub async fn cleanup_expired(&self) -> Result<usize> {
        self.store.cleanup_expired().await
    }
    
    /// Get session statistics
    pub async fn get_stats(&self) -> Result<SessionStats> {
        let total_sessions = self.store.count().await?;
        Ok(SessionStats {
            total_sessions,
            active_sessions: total_sessions, // Simplified - expired sessions are cleaned up
        })
    }
    
    /// Get cookie name
    pub fn cookie_name(&self) -> &str {
        &self.cookie_name
    }
    
    /// Get timeout duration
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

/// Session statistics
#[derive(Debug, Clone, Serialize)]
pub struct SessionStats {
    pub total_sessions: usize,
    pub active_sessions: usize,
}

/// Factory for creating session stores
pub struct SessionStoreFactory;

impl SessionStoreFactory {
    pub fn create_store(
        storage_type: &str,
        config: &HashMap<String, serde_yaml::Value>,
    ) -> Result<Box<dyn SessionStore>> {
        match storage_type {
            "memory" => Ok(Box::new(MemorySessionStore::new())),
            "file" => {
                let file_path = config.get("file_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("./sessions.json");
                Ok(Box::new(FileSessionStore::new(file_path)))
            }
            #[cfg(feature = "sqlite")]
            "sqlite" => {
                let db_path = config.get("db_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("./sessions.db");
                Ok(Box::new(SqliteSessionStore::new(db_path).await?))
            }
            #[cfg(feature = "redis")]
            "redis" => {
                let redis_url = config.get("redis_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("redis://127.0.0.1:6379");
                Ok(Box::new(crate::adapters::redis::RedisSessionStore::new(redis_url).await?))
            }
            _ => Err(ServerError::session(format!("Unknown session storage type: {}", storage_type))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[test]
    fn test_session_creation() {
        let session = Session::new("test-id".to_string(), Duration::from_secs(3600));
        assert_eq!(session.id, "test-id");
        assert!(!session.is_expired());
        assert!(session.data.is_empty());
    }
    
    #[test]
    fn test_session_data() {
        let mut session = Session::new("test-id".to_string(), Duration::from_secs(3600));
        
        session.set("username", "john_doe").unwrap();
        session.set("age", 25u32).unwrap();
        
        assert_eq!(session.get::<String>("username"), Some("john_doe".to_string()));
        assert_eq!(session.get::<u32>("age"), Some(25));
        assert_eq!(session.get::<String>("nonexistent"), None);
    }
    
    #[test]
    fn test_session_expiration() {
        let mut session = Session::new("test-id".to_string(), Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(2));
        assert!(session.is_expired());
        
        // Refresh should extend expiration
        session.refresh(Duration::from_secs(3600));
        assert!(!session.is_expired());
    }
}