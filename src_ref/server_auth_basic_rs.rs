use super::{AuthProvider, AuthResult, AuthUser};
use crate::error::Result;
use async_trait::async_trait;
use hyper::{Request, Body};
use std::collections::HashMap;

/// Basic HTTP authentication provider
pub struct BasicAuthProvider {
    users: HashMap<String, String>, // username -> password
    realm: String,
}

impl BasicAuthProvider {
    pub fn new(users: HashMap<String, String>, realm: Option<String>) -> Self {
        Self {
            users,
            realm: realm.unwrap_or_else(|| "API".to_string()),
        }
    }
    
    /// Add a user
    pub fn add_user(&mut self, username: impl Into<String>, password: impl Into<String>) {
        self.users.insert(username.into(), password.into());
    }
    
    /// Remove a user
    pub fn remove_user(&mut self, username: &str) {
        self.users.remove(username);
    }
    
    /// Verify credentials
    fn verify_credentials(&self, username: &str, password: &str) -> bool {
        self.users.get(username)
            .map(|stored_password| stored_password == password)
            .unwrap_or(false)
    }
    
    /// Extract basic auth credentials from request
    fn extract_credentials(&self, request: &Request<Body>) -> Option<(String, String)> {
        super::utils::extract_basic_auth(request)
    }
}

#[async_trait]
impl AuthProvider for BasicAuthProvider {
    fn name(&self) -> &str {
        "basic"
    }
    
    async fn authenticate(&self, request: &Request<Body>) -> Result<AuthResult> {
        if let Some((username, password)) = self.extract_credentials(request) {
            if self.verify_credentials(&username, &password) {
                let user = AuthUser::new(username.clone(), username);
                Ok(AuthResult::Success(user))
            } else {
                Ok(AuthResult::Failed("Invalid username or password".to_string()))
            }
        } else {
            Ok(AuthResult::NoAuth)
        }
    }
    
    fn has_credentials(&self, request: &Request<Body>) -> bool {
        request
            .headers()
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .map(|h| h.starts_with("Basic "))
            .unwrap_or(false)
    }
    
    fn scheme(&self) -> &str {
        "Basic"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::header::AUTHORIZATION;
    
    #[tokio::test]
    async fn test_basic_auth_success() {
        let mut users = HashMap::new();
        users.insert("admin".to_string(), "secret".to_string());
        users.insert("user".to_string(), "password".to_string());
        
        let provider = BasicAuthProvider::new(users, Some("Test Realm".to_string()));
        
        // Create request with valid credentials
        let credentials = base64::encode("admin:secret");
        let request = Request::builder()
            .header(AUTHORIZATION, format!("Basic {}", credentials))
            .body(Body::empty())
            .unwrap();
        
        let result = provider.authenticate(&request).await.unwrap();
        assert!(result.is_success());
        
        let user = result.user().unwrap();
        assert_eq!(user.username, "admin");
        assert_eq!(user.id, "admin");
    }
    
    #[tokio::test]
    async fn test_basic_auth_invalid_credentials() {
        let mut users = HashMap::new();
        users.insert("admin".to_string(), "secret".to_string());
        
        let provider = BasicAuthProvider::new(users, None);
        
        // Create request with invalid credentials
        let credentials = base64::encode("admin:wrong");
        let request = Request::builder()
            .header(AUTHORIZATION, format!("Basic {}", credentials))
            .body(Body::empty())
            .unwrap();
        
        let result = provider.authenticate(&request).await.unwrap();
        assert!(result.is_failed());
        assert_eq!(result.error_message(), Some("Invalid username or password"));
    }
    
    #[tokio::test]
    async fn test_basic_auth_no_credentials() {
        let users = HashMap::new();
        let provider = BasicAuthProvider::new(users, None);
        
        // Create request without credentials
        let request = Request::builder()
            .body(Body::empty())
            .unwrap();
        
        let result = provider.authenticate(&request).await.unwrap();
        assert!(matches!(result, AuthResult::NoAuth));
        assert!(!provider.has_credentials(&request));
    }
    
    #[tokio::test]
    async fn test_basic_auth_malformed_header() {
        let users = HashMap::new();
        let provider = BasicAuthProvider::new(users, None);
        
        // Create request with malformed authorization header
        let request = Request::builder()
            .header(AUTHORIZATION, "Basic invalid-base64")
            .body(Body::empty())
            .unwrap();
        
        let result = provider.authenticate(&request).await.unwrap();
        assert!(matches!(result, AuthResult::NoAuth));
    }
    
    #[test]
    fn test_user_management() {
        let mut provider = BasicAuthProvider::new(HashMap::new(), None);
        
        // Add users
        provider.add_user("user1", "pass1");
        provider.add_user("user2", "pass2");
        
        assert!(provider.verify_credentials("user1", "pass1"));
        assert!(provider.verify_credentials("user2", "pass2"));
        assert!(!provider.verify_credentials("user1", "wrong"));
        assert!(!provider.verify_credentials("nonexistent", "pass"));
        
        // Remove user
        provider.remove_user("user1");
        assert!(!provider.verify_credentials("user1", "pass1"));
        assert!(provider.verify_credentials("user2", "pass2"));
    }
}