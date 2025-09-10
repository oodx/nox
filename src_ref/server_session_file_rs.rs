use super::{Session, SessionStore};
use crate::error::{ServerError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::fs;
use tokio::sync::RwLock;

/// File-based session store that persists sessions to disk
pub struct FileSessionStore {
    file_path: PathBuf,
    sessions: RwLock<HashMap<String, Session>>,
    dirty: RwLock<bool>,
}

impl FileSessionStore {
    pub fn new<P: AsRef<Path>>(file_path: P) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            sessions: RwLock::new(HashMap::new()),
            dirty: RwLock::new(false),
        }
    }
    
    /// Load sessions from file
    async fn load_from_file(&self) -> Result<()> {
        if !self.file_path.exists() {
            return Ok(());
        }
        
        let content = fs::read_to_string(&self.file_path).await?;
        if content.trim().is_empty() {
            return Ok(());
        }
        
        let sessions: HashMap<String, Session> = serde_json::from_str(&content)
            .map_err(|e| ServerError::session(format!("Failed to parse sessions file: {}", e)))?;
        
        let mut store_sessions = self.sessions.write().await;
        *store_sessions = sessions;
        
        Ok(())
    }
    
    /// Save sessions to file
    async fn save_to_file(&self) -> Result<()> {
        let sessions = self.sessions.read().await;
        let content = serde_json::to_string_pretty(&*sessions)?;
        
        // Ensure parent directory exists
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        // Write to temporary file first, then rename for atomicity
        let temp_path = self.file_path.with_extension("tmp");
        fs::write(&temp_path, content).await?;
        fs::rename(&temp_path, &self.file_path).await?;
        
        let mut dirty = self.dirty.write().await;
        *dirty = false;
        
        Ok(())
    }
    
    /// Mark as dirty (needs saving)
    async fn mark_dirty(&self) {
        let mut dirty = self.dirty.write().await;
        *dirty = true;
    }
    
    /// Check if dirty and save if needed
    async fn save_if_dirty(&self) -> Result<()> {
        let dirty = *self.dirty.read().await;
        if dirty {
            self.save_to_file().await?;
        }
        Ok(())
    }
    
    /// Initialize the store by loading existing sessions
    pub async fn initialize(&self) -> Result<()> {
        self.load_from_file().await
    }
    
    /// Force save all sessions to disk
    pub async fn flush(&self) -> Result<()> {
        self.save_to_file().await
    }
}

#[async_trait]
impl SessionStore for FileSessionStore {
    async fn get(&self, session_id: &str) -> Result<Option<Session>> {
        // Load from file on first access
        if self.sessions.read().await.is_empty() {
            self.load_from_file().await?;
        }
        
        let sessions = self.sessions.read().await;
        Ok(sessions.get(session_id).cloned())
    }
    
    async fn save(&self, session: &Session) -> Result<()> {
        // Load from file if not already loaded
        if self.sessions.read().await.is_empty() {
            self.load_from_file().await?;
        }
        
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session.id.clone(), session.clone());
        }
        
        self.mark_dirty().await;
        self.save_if_dirty().await?;
        
        Ok(())
    }
    
    async fn delete(&self, session_id: &str) -> Result<()> {
        // Load from file if not already loaded
        if self.sessions.read().await.is_empty() {
            self.load_from_file().await?;
        }
        
        {
            let mut sessions = self.sessions.write().await;
            sessions.remove(session_id);
        }
        
        self.mark_dirty().await;
        self.save_if_dirty().await?;
        
        Ok(())
    }
    
    async fn cleanup_expired(&self) -> Result<usize> {
        // Load from file if not already loaded
        if self.sessions.read().await.is_empty() {
            self.load_from_file().await?;
        }
        
        let now = SystemTime::now();
        let removed_count;
        
        {
            let mut sessions = self.sessions.write().await;
            let initial_count = sessions.len();
            sessions.retain(|_, session| session.expires_at > now);
            removed_count = initial_count - sessions.len();
        }
        
        if removed_count > 0 {
            self.mark_dirty().await;
            self.save_if_dirty().await?;
        }
        
        Ok(removed_count)
    }
    
    async fn list_sessions(&self) -> Result<Vec<String>> {
        // Load from file if not already loaded
        if self.sessions.read().await.is_empty() {
            self.load_from_file().await?;
        }
        
        let sessions = self.sessions.read().await;
        Ok(sessions.keys().cloned().collect())
    }
    
    async fn count(&self) -> Result<usize> {
        // Load from file if not already loaded
        if self.sessions.read().await.is_empty() {
            self.load_from_file().await?;
        }
        
        let sessions = self.sessions.read().await;
        Ok(sessions.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_file_session_store() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("sessions.json");
        
        let store = FileSessionStore::new(&file_path);
        store.initialize().await.unwrap();
        
        let session = Session::new("test-456".to_string(), Duration::from_secs(3600));
        
        // Save session
        store.save(&session).await.unwrap();
        
        // File should exist and contain session data
        assert!(file_path.exists());
        
        // Create new store instance to test persistence
        let store2 = FileSessionStore::new(&file_path);
        store2.initialize().await.unwrap();
        
        // Get session from new store instance
        let retrieved = store2.get("test-456").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "test-456");
    }
    
    #[tokio::test]
    async fn test_file_session_persistence() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("sessions.json");
        
        // Create and save sessions
        {
            let store = FileSessionStore::new(&file_path);
            store.initialize().await.unwrap();
            
            let session1 = Session::new("session1".to_string(), Duration::from_secs(3600));
            let session2 = Session::new("session2".to_string(), Duration::from_secs(3600));
            
            store.save(&session1).await.unwrap();
            store.save(&session2).await.unwrap();
            
            store.flush().await.unwrap();
        }
        
        // Load sessions in new instance
        {
            let store = FileSessionStore::new(&file_path);
            store.initialize().await.unwrap();
            
            let count = store.count().await.unwrap();
            assert_eq!(count, 2);
            
            let session1 = store.get("session1").await.unwrap();
            let session2 = store.get("session2").await.unwrap();
            
            assert!(session1.is_some());
            assert!(session2.is_some());
        }
    }
}