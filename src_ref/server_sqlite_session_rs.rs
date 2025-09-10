#[cfg(feature = "sqlite")]
use super::{Session, SessionStore};
#[cfg(feature = "sqlite")]
use crate::error::{ServerError, Result};
#[cfg(feature = "sqlite")]
use async_trait::async_trait;
#[cfg(feature = "sqlite")]
use sqlx::{sqlite::SqlitePool, Row, Sqlite};
#[cfg(feature = "sqlite")]
use std::path::Path;
#[cfg(feature = "sqlite")]
use std::time::SystemTime;

#[cfg(feature = "sqlite")]
/// SQLite-based session store
pub struct SqliteSessionStore {
    pool: SqlitePool,
}

#[cfg(feature = "sqlite")]
impl SqliteSessionStore {
    pub async fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let db_url = format!("sqlite:{}", db_path.as_ref().display());
        
        // Create database file if it doesn't exist
        if let Some(parent) = db_path.as_ref().parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        let pool = SqlitePool::connect(&db_url).await
            .map_err(|e| ServerError::database(format!("Failed to connect to SQLite: {}", e)))?;
        
        let store = Self { pool };
        store.create_table().await?;
        
        Ok(store)
    }
    
    /// Create sessions table if it doesn't exist
    async fn create_table(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                user_id TEXT,
                data TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                last_accessed INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ServerError::database(format!("Failed to create sessions table: {}", e)))?;
        
        // Create index on expires_at for cleanup efficiency
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at)",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ServerError::database(format!("Failed to create index: {}", e)))?;
        
        Ok(())
    }
    
    /// Convert SystemTime to Unix timestamp
    fn system_time_to_unix(time: SystemTime) -> i64 {
        time.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }
    
    /// Convert Unix timestamp to SystemTime
    fn unix_to_system_time(timestamp: i64) -> SystemTime {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(timestamp as u64)
    }
}

#[cfg(feature = "sqlite")]
#[async_trait]
impl SessionStore for SqliteSessionStore {
    async fn get(&self, session_id: &str) -> Result<Option<Session>> {
        let row = sqlx::query(
            "SELECT id, user_id, data, created_at, expires_at, last_accessed FROM sessions WHERE id = ?",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ServerError::database(format!("Failed to get session: {}", e)))?;
        
        if let Some(row) = row {
            let data_json: String = row.get("data");
            let data: std::collections::HashMap<String, serde_json::Value> = 
                serde_json::from_str(&data_json)?;
            
            let session = Session {
                id: row.get("id"),
                user_id: row.get("user_id"),
                data,
                created_at: Self::unix_to_system_time(row.get("created_at")),
                expires_at: Self::unix_to_system_time(row.get("expires_at")),
                last_accessed: Self::unix_to_system_time(row.get("last_accessed")),
            };
            
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }
    
    async fn save(&self, session: &Session) -> Result<()> {
        let data_json = serde_json::to_string(&session.data)?;
        
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO sessions 
            (id, user_id, data, created_at, expires_at, last_accessed)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&session.id)
        .bind(&session.user_id)
        .bind(data_json)
        .bind(Self::system_time_to_unix(session.created_at))
        .bind(Self::system_time_to_unix(session.expires_at))
        .bind(Self::system_time_to_unix(session.last_accessed))
        .execute(&self.pool)
        .await
        .map_err(|e| ServerError::database(format!("Failed to save session: {}", e)))?;
        
        Ok(())
    }
    
    async fn delete(&self, session_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|e| ServerError::database(format!("Failed to delete session: {}", e)))?;
        
        Ok(())
    }
    
    async fn cleanup_expired(&self) -> Result<usize> {
        let now = Self::system_time_to_unix(SystemTime::now());
        
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(|e| ServerError::database(format!("Failed to cleanup expired sessions: {}", e)))?;
        
        Ok(result.rows_affected() as usize)
    }
    
    async fn list_sessions(&self) -> Result<Vec<String>> {
        let rows = sqlx::query("SELECT id FROM sessions")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ServerError::database(format!("Failed to list sessions: {}", e)))?;
        
        let session_ids: Vec<String> = rows
            .into_iter()
            .map(|row| row.get("id"))
            .collect();
        
        Ok(session_ids)
    }
    
    async fn count(&self) -> Result<usize> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM sessions")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| ServerError::database(format!("Failed to count sessions: {}", e)))?;
        
        let count: i64 = row.get("count");
        Ok(count as usize)
    }
}

// Provide empty implementation when SQLite feature is disabled
#[cfg(not(feature = "sqlite"))]
pub struct SqliteSessionStore;

#[cfg(not(feature = "sqlite"))]
impl SqliteSessionStore {
    pub async fn new<P: AsRef<std::path::Path>>(_db_path: P) -> Result<Self> {
        Err(ServerError::other("SQLite support not compiled in"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_sqlite_session_store() {
        use tempfile::tempdir;
        use std::time::Duration;
        
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_sessions.db");
        
        let store = SqliteSessionStore::new(&db_path).await.unwrap();
        let session = Session::new("test-session".to_string(), Duration::from_secs(3600));
        
        // Save session
        store.save(&session).await.unwrap();
        
        // Get session
        let retrieved = store.get("test-session").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "test-session");
        
        // Count sessions
        let count = store.count().await.unwrap();
        assert_eq!(count, 1);
        
        // List sessions
        let sessions = store.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0], "test-session");
        
        // Delete session
        store.delete("test-session").await.unwrap();
        let deleted = store.get("test-session").await.unwrap();
        assert!(deleted.is_none());
    }
    
    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_sqlite_cleanup_expired() {
        use tempfile::tempdir;
        use std::time::Duration;
        
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_cleanup.db");
        
        let store = SqliteSessionStore::new(&db_path).await.unwrap();
        
        // Create expired session
        let mut expired_session = Session::new("expired".to_string(), Duration::from_secs(1));
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