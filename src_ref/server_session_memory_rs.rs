use super::{Session, SessionStore};
use crate::error::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::SystemTime;
use tokio::sync::RwLock;

/// In-memory session store (data is lost on restart)
pub struct MemorySessionStore {
    sessions: RwLock<HashMap<String, Session>>,
}

impl MemorySessionStore {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl SessionStore for MemorySessionStore {
    async fn get(&self, session_id: &str) -> Result<Option<Session>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(session_id).cloned())
    }
    
    async fn save(&self, session: &Session) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id.clone(), session.clone());
        Ok(())
    }
    
    async fn delete(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
        Ok(())
    }
    
    async fn cleanup_expired(&self) -> Result<usize> {
        let mut sessions = self.sessions.write().await;
        let now = SystemTime::now();
        let initial_count = sessions.len();
        
        sessions.retain(|_, session| session.expires_at > now);
        
        let removed_count = initial_count - sessions.len();
        Ok(removed_count)
    }
    
    async fn list_sessions(&self) -> Result<Vec<String>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.keys().cloned().collect())
    }
    
    async fn count(&self) -> Result<usize> {
        let sessions = self.sessions.read().await;
        Ok(sessions.len())
    }
}

impl Default for MemorySessionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[tokio::test]
    async fn test_memory_session_store() {
        let store = MemorySessionStore::new();
        let session = Session::new("test-123".to_string(), Duration::from_secs(3600));
        
        // Save session
        store.save(&session).await.unwrap();
        
        // Get session
        let retrieved = store.get("test-123").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "test-123");
        
        // Delete session
        store.delete("test-123").await.unwrap();
        let deleted = store.get("test-123").await.unwrap();
        assert!(deleted.is_none());
    }
    
    #[tokio::test]
    async fn test_memory_session_cleanup() {
        let store = MemorySessionStore::new();
        
        // Create expired session
        let mut expired_session = Session::new("expired".to_string(), Duration::from_millis(1));
        expired_session.expires_at = SystemTime::now() - Duration::from_secs(1);
        store.save(&expired_session).await.unwrap();
        
        // Create valid session
        let valid_session = Session::new("valid".to_string(), Duration::from_secs(3600));
        store.save(&valid_session).await.unwrap();
        
        // Cleanup should remove expired session
        let removed = store.cleanup_expired().await.unwrap();
        assert_eq!(removed, 1);
        
        // Valid session should still exist
        let valid_exists = store.get("valid").await.unwrap();
        assert!(valid_exists.is_some());
        
        // Expired session should be gone
        let expired_exists = store.get("expired").await.unwrap();
        assert!(expired_exists.is_none());
    }
}