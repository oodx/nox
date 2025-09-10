use super::{AuthProvider, AuthResult, AuthUser};
use crate::error::Result;
use async_trait::async_trait;
use hyper::{Request, Body};
use std::collections::HashMap;

/// Bearer token authentication provider
pub struct BearerAuthProvider {
    tokens: HashMap<String, AuthUser>, // token -> user
}

impl BearerAuthProvider {
    pub fn new(tokens: HashMap<String, AuthUser>) -> Self {
        Self { tokens }
    }
    
    /// Add a token for a user
    pub fn add_token(&mut self, token: impl Into<String>, user: AuthUser) {
        self.tokens.insert(token.into(), user);
    }
    
    /// Remove a token
    pub fn remove_token(&mut self, token: &str) {
        self.tokens.remove(token);
    }
    
    /// Verify token and get associated user
    fn verify_token(&self, token: &str) -> Option<&AuthUser> {
        self.tokens.get(token)
    }
    
    /// Extract bearer token from request
    fn extract_token(&self, request: &Request<Body>) -> Option<String> {
        super::utils::extract_bearer_token(request)
    }
}

#[async_trait]
impl AuthProvider for BearerAuthProvider {
    fn name(&self) -> &str {
        "bearer"
    }
    
    async fn authenticate(&self, request: &Request<Body>) -> Result<AuthResult> {
        if let Some(token) = self.extract_token(request) {
            if let Some(user) = self.verify_token(&token) {
                Ok(AuthResult::Success(user.clone()))
            } else {
                Ok(AuthResult::Failed("Invalid token".to_string()))
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
            .map(|h| h.starts_with("Bearer "))
            .unwrap_or(false)
    }
    
    fn scheme(&self) -> &str {
        "Bearer"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::header::AUTHORIZATION;
    
    #[tokio::test]
    async fn test_bearer_auth_success() {
        let mut tokens = HashMap::new();
        let user = AuthUser::new("123", "john_doe")
            .with_roles(vec!["admin".to_string()]);
        tokens.insert("valid-token-123".to_string(), user);
        
        let provider = BearerAuthProvider::new(tokens);
        
        // Create request with valid token
        let request = Request::builder()
            .header(AUTHORIZATION, "Bearer valid-token-123")
            .body(Body::empty())
            .unwrap();
        
        let result = provider.authenticate(&request).await.unwrap();
        assert!(result.is_success());
        
        let user = result.user().unwrap();
        assert_eq!(user.username, "john_doe");
        assert_eq!(user.id, "123");
        assert!(user.has_role("admin"));
    }
    
    #[tokio::test]
    async fn test_bearer_auth_invalid_token() {
        let tokens = HashMap::new();
        let provider = BearerAuthProvider::new(tokens);
        
        // Create request with invalid token
        let request = Request::builder()
            .header(AUTHORIZATION, "Bearer invalid-token")
            .body(Body::empty())
            .unwrap();
        
        let result = provider.authenticate(&request).await.unwrap();
        assert!(result.is_failed());
        assert_eq!(result.error_message(), Some("Invalid token"));
    }
    
    #[tokio::test]
    async fn test_bearer_auth_no_credentials() {
        let tokens = HashMap::new();
        let provider = BearerAuthProvider::new(tokens);
        
        // Create request without credentials
        let request = Request::builder()
            .body(Body::empty())
            .unwrap();
        
        let result = provider.authenticate(&request).await.unwrap();
        assert!(matches!(result, AuthResult::NoAuth));
        assert!(!provider.has_credentials(&request));
    }
    
    #[tokio::test]
    async fn test_bearer_auth_malformed_header() {
        let tokens = HashMap::new();
        let provider = BearerAuthProvider::new(tokens);
        
        // Create request with malformed authorization header
        let request = Request::builder()
            .header(AUTHORIZATION, "Basic not-bearer")
            .body(Body::empty())
            .unwrap();
        
        let result = provider.authenticate(&request).await.unwrap();
        assert!(matches!(result, AuthResult::NoAuth));
        assert!(!provider.has_credentials(&request));
    }
    
    #[test]
    fn test_token_management() {
        let mut provider = BearerAuthProvider::new(HashMap::new());
        
        let user1 = AuthUser::new("1", "user1");
        let user2 = AuthUser::new("2", "user2");
        
        // Add tokens
        provider.add_token("token1", user1);
        provider.add_token("token2", user2);
        
        assert!(provider.verify_token("token1").is_some());
        assert!(provider.verify_token("token2").is_some());
        assert!(provider.verify_token("nonexistent").is_none());
        
        // Remove token
        provider.remove_token("token1");
        assert!(provider.verify_token("token1").is_none());
        assert!(provider.verify_token("token2").is_some());
    }
}