use super::{AuthProvider, AuthResult, AuthUser};
use crate::error::Result;
use async_trait::async_trait;
use hyper::{Request, Body};
use std::collections::HashSet;

/// API key authentication provider
pub struct ApiKeyAuthProvider {
    keys: HashSet<String>,
    header_name: String,
}

impl ApiKeyAuthProvider {
    pub fn new(keys: Vec<String>, header_name: Option<String>) -> Self {
        Self {
            keys: keys.into_iter().collect(),
            header_name: header_name.unwrap_or_else(|| "X-API-Key".to_string()),
        }
    }
    
    /// Add an API key
    pub fn add_key(&mut self, key: impl Into<String>) {
        self.keys.insert(key.into());
    }
    
    /// Remove an API key
    pub fn remove_key(&mut self, key: &str) {
        self.keys.remove(key);
    }
    
    /// Verify API key
    fn verify_key(&self, key: &str) -> bool {
        self.keys.contains(key)
    }
    
    /// Extract API key from request
    fn extract_key(&self, request: &Request<Body>) -> Option<String> {
        super::utils::extract_api_key(request, &self.header_name)
    }
    
    /// Get header name
    pub fn header_name(&self) -> &str {
        &self.header_name
    }
}

#[async_trait]
impl AuthProvider for ApiKeyAuthProvider {
    fn name(&self) -> &str {
        "api_key"
    }
    
    async fn authenticate(&self, request: &Request<Body>) -> Result<AuthResult> {
        if let Some(key) = self.extract_key(request) {
            if self.verify_key(&key) {
                // Create a generic user for API key authentication
                // In a real implementation, you might want to map keys to specific users
                let user = AuthUser::new(
                    format!("api_key_{}", &key[..8.min(key.len())]), // Use first 8 chars as ID
                    format!("api_user_{}", &key[..8.min(key.len())]),
                ).with_roles(vec!["api_user".to_string()]);
                
                Ok(AuthResult::Success(user))
            } else {
                Ok(AuthResult::Failed("Invalid API key".to_string()))
            }
        } else {
            Ok(AuthResult::NoAuth)
        }
    }
    
    fn has_credentials(&self, request: &Request<Body>) -> bool {
        request
            .headers()
            .get(&self.header_name)
            .is_some()
    }
    
    fn scheme(&self) -> &str {
        "ApiKey"
    }
}

/// Enhanced API key provider that maps keys to specific users
pub struct UserMappedApiKeyProvider {
    key_users: std::collections::HashMap<String, AuthUser>,
    header_name: String,
}

impl UserMappedApiKeyProvider {
    pub fn new(header_name: Option<String>) -> Self {
        Self {
            key_users: std::collections::HashMap::new(),
            header_name: header_name.unwrap_or_else(|| "X-API-Key".to_string()),
        }
    }
    
    /// Add an API key for a specific user
    pub fn add_key_for_user(&mut self, key: impl Into<String>, user: AuthUser) {
        self.key_users.insert(key.into(), user);
    }
    
    /// Remove an API key
    pub fn remove_key(&mut self, key: &str) {
        self.key_users.remove(key);
    }
    
    /// Verify API key and get associated user
    fn verify_key(&self, key: &str) -> Option<&AuthUser> {
        self.key_users.get(key)
    }
    
    /// Extract API key from request
    fn extract_key(&self, request: &Request<Body>) -> Option<String> {
        super::utils::extract_api_key(request, &self.header_name)
    }
}

#[async_trait]
impl AuthProvider for UserMappedApiKeyProvider {
    fn name(&self) -> &str {
        "user_mapped_api_key"
    }
    
    async fn authenticate(&self, request: &Request<Body>) -> Result<AuthResult> {
        if let Some(key) = self.extract_key(request) {
            if let Some(user) = self.verify_key(&key) {
                Ok(AuthResult::Success(user.clone()))
            } else {
                Ok(AuthResult::Failed("Invalid API key".to_string()))
            }
        } else {
            Ok(AuthResult::NoAuth)
        }
    }
    
    fn has_credentials(&self, request: &Request<Body>) -> bool {
        request
            .headers()
            .get(&self.header_name)
            .is_some()
    }
    
    fn scheme(&self) -> &str {
        "ApiKey"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_api_key_auth_success() {
        let keys = vec!["valid-key-123".to_string(), "another-key-456".to_string()];
        let provider = ApiKeyAuthProvider::new(keys, Some("X-API-Key".to_string()));
        
        // Create request with valid API key
        let request = Request::builder()
            .header("X-API-Key", "valid-key-123")
            .body(Body::empty())
            .unwrap();
        
        let result = provider.authenticate(&request).await.unwrap();
        assert!(result.is_success());
        
        let user = result.user().unwrap();
        assert_eq!(user.id, "api_key_valid-ke");
        assert!(user.has_role("api_user"));
    }
    
    #[tokio::test]
    async fn test_api_key_auth_invalid_key() {
        let keys = vec!["valid-key-123".to_string()];
        let provider = ApiKeyAuthProvider::new(keys, None);
        
        // Create request with invalid API key
        let request = Request::builder()
            .header("X-API-Key", "invalid-key")
            .body(Body::empty())
            .unwrap();
        
        let result = provider.authenticate(&request).await.unwrap();
        assert!(result.is_failed());
        assert_eq!(result.error_message(), Some("Invalid API key"));
    }
    
    #[tokio::test]
    async fn test_api_key_auth_no_credentials() {
        let keys = vec!["valid-key-123".to_string()];
        let provider = ApiKeyAuthProvider::new(keys, None);
        
        // Create request without API key
        let request = Request::builder()
            .body(Body::empty())
            .unwrap();
        
        let result = provider.authenticate(&request).await.unwrap();
        assert!(matches!(result, AuthResult::NoAuth));
        assert!(!provider.has_credentials(&request));
    }
    
    #[tokio::test]
    async fn test_custom_header_name() {
        let keys = vec!["test-key".to_string()];
        let provider = ApiKeyAuthProvider::new(keys, Some("Authorization".to_string()));
        
        // Create request with custom header
        let request = Request::builder()
            .header("Authorization", "test-key")
            .body(Body::empty())
            .unwrap();
        
        let result = provider.authenticate(&request).await.unwrap();
        assert!(result.is_success());
        assert!(provider.has_credentials(&request));
    }
    
    #[test]
    fn test_key_management() {
        let mut provider = ApiKeyAuthProvider::new(vec![], None);
        
        // Add keys
        provider.add_key("key1");
        provider.add_key("key2");
        
        assert!(provider.verify_key("key1"));
        assert!(provider.verify_key("key2"));
        assert!(!provider.verify_key("nonexistent"));
        
        // Remove key
        provider.remove_key("key1");
        assert!(!provider.verify_key("key1"));
        assert!(provider.verify_key("key2"));
    }
    
    #[tokio::test]
    async fn test_user_mapped_api_key() {
        let mut provider = UserMappedApiKeyProvider::new(None);
        
        let user1 = AuthUser::new("1", "alice")
            .with_roles(vec!["admin".to_string()]);
        let user2 = AuthUser::new("2", "bob")
            .with_roles(vec!["user".to_string()]);
        
        provider.add_key_for_user("alice-key", user1);
        provider.add_key_for_user("bob-key", user2);
        
        // Test alice's key
        let request = Request::builder()
            .header("X-API-Key", "alice-key")
            .body(Body::empty())
            .unwrap();
        
        let result = provider.authenticate(&request).await.unwrap();
        assert!(result.is_success());
        
        let user = result.user().unwrap();
        assert_eq!(user.username, "alice");
        assert!(user.has_role("admin"));
        
        // Test bob's key
        let request = Request::builder()
            .header("X-API-Key", "bob-key")
            .body(Body::empty())
            .unwrap();
        
        let result = provider.authenticate(&request).await.unwrap();
        assert!(result.is_success());
        
        let user = result.user().unwrap();
        assert_eq!(user.username, "bob");
        assert!(user.has_role("user"));
    }
}