use crate::config::StaticFilesConfig;
use crate::error::{ServerError, Result};
use crate::handlers::{Handler, HandlerResult};
use crate::plugins::PluginContext;
use async_trait::async_trait;
use hyper::{Body, Request, Response, StatusCode};
use mime_guess::from_path;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Static file handler for serving files from a directory
pub struct StaticFileHandler {
    config: StaticFilesConfig,
}

impl StaticFileHandler {
    pub fn new(config: StaticFilesConfig) -> Self {
        Self { config }
    }
    
    /// Resolve file path from request path
    fn resolve_path(&self, request_path: &str) -> Option<PathBuf> {
        // Remove prefix if configured
        let path = if let Some(prefix) = &self.config.prefix {
            if request_path.starts_with(prefix) {
                &request_path[prefix.len()..]
            } else {
                return None;
            }
        } else {
            request_path
        };
        
        // Remove leading slash
        let path = path.trim_start_matches('/');
        
        // Prevent directory traversal
        if path.contains("..") || path.contains("\\..") || path.starts_with('/') {
            return None;
        }
        
        let file_path = self.config.root_dir.join(path);
        
        // Ensure the resolved path is within root directory
        if let Ok(canonical_file) = file_path.canonicalize() {
            if let Ok(canonical_root) = self.config.root_dir.canonicalize() {
                if canonical_file.starts_with(canonical_root) {
                    return Some(canonical_file);
                }
            }
        }
        
        None
    }
    
    /// Check if path is a directory and try to serve index file
    async fn try_index_file(&self, dir_path: &Path) -> Option<PathBuf> {
        if !dir_path.is_dir() {
            return None;
        }
        
        for index_file in &self.config.index_files {
            let index_path = dir_path.join(index_file);
            if index_path.is_file() {
                return Some(index_path);
            }
        }
        
        None
    }
    
    /// Get MIME type for file
    fn get_mime_type(&self, path: &Path) -> String {
        from_path(path)
            .first_or_octet_stream()
            .to_string()
    }
    
    /// Create cache control header
    fn get_cache_control(&self) -> Option<&str> {
        self.config.cache_control.as_deref()
    }
    
    /// Serve a file
    async fn serve_file(&self, file_path: &Path) -> Result<Response<Body>> {
        // Check if file exists and is readable
        let metadata = fs::metadata(file_path).await
            .map_err(|_| ServerError::not_found("File not found"))?;
        
        if !metadata.is_file() {
            return Err(ServerError::not_found("Not a file"));
        }
        
        // Read file content
        let content = fs::read(file_path).await?;
        
        // Build response
        let mut response_builder = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", self.get_mime_type(file_path))
            .header("content-length", content.len());
        
        // Add cache control header if configured
        if let Some(cache_control) = self.get_cache_control() {
            response_builder = response_builder.header("cache-control", cache_control);
        }
        
        // Add last modified header
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                let timestamp = httpdate::fmt_http_date(modified);
                response_builder = response_builder.header("last-modified", timestamp);
                
                // Add ETag based on file size and modified time
                let etag = format!("\"{}:{}\"", metadata.len(), duration.as_secs());
                response_builder = response_builder.header("etag", etag);
            }
        }
        
        let response = response_builder
            .body(Body::from(content))
            .map_err(|e| ServerError::handler(format!("Failed to build response: {}", e)))?;
        
        Ok(response)
    }
    
    /// Check conditional headers (If-Modified-Since, If-None-Match)
    fn check_conditional_headers(&self, request: &Request<Body>, file_path: &Path) -> Result<Option<Response<Body>>> {
        // This is a simplified implementation
        // In a full implementation, you'd check If-Modified-Since and If-None-Match headers
        // and return 304 Not Modified if appropriate
        Ok(None)
    }
}

#[async_trait]
impl Handler for StaticFileHandler {
    async fn handle(&self, request: &Request<Body>, _context: &PluginContext) -> Result<HandlerResult> {
        if !self.config.enabled {
            return Ok(HandlerResult::NotFound);
        }
        
        // Only handle GET and HEAD requests
        match request.method() {
            &hyper::Method::GET | &hyper::Method::HEAD => {},
            _ => return Ok(HandlerResult::NotFound),
        }
        
        let request_path = request.uri().path();
        
        // Resolve file path
        let file_path = match self.resolve_path(request_path) {
            Some(path) => path,
            None => return Ok(HandlerResult::NotFound),
        };
        
        // Check if it's a directory and try index files
        let final_path = if file_path.is_dir() {
            match self.try_index_file(&file_path).await {
                Some(index_path) => index_path,
                None => return Ok(HandlerResult::NotFound),
            }
        } else {
            file_path
        };
        
        // Check conditional headers
        if let Some(response) = self.check_conditional_headers(request, &final_path)? {
            return Ok(HandlerResult::Response(response));
        }
        
        // Serve the file
        match self.serve_file(&final_path).await {
            Ok(mut response) => {
                // For HEAD requests, remove body but keep headers
                if request.method() == hyper::Method::HEAD {
                    *response.body_mut() = Body::empty();
                }
                Ok(HandlerResult::Response(response))
            }
            Err(ServerError::NotFound(_)) => Ok(HandlerResult::NotFound),
            Err(e) => Ok(HandlerResult::Error(e)),
        }
    }
    
    fn name(&self) -> &str {
        "static_files"
    }
    
    fn description(&self) -> &str {
        "Serves static files from a directory"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::collections::HashMap;
    
    fn create_test_config(root_dir: PathBuf) -> StaticFilesConfig {
        StaticFilesConfig {
            enabled: true,
            root_dir,
            index_files: vec!["index.html".to_string()],
            cache_control: Some("public, max-age=3600".to_string()),
            prefix: None,
        }
    }
    
    #[tokio::test]
    async fn test_static_file_handler() {
        let temp_dir = tempdir().unwrap();
        let root_path = temp_dir.path().to_path_buf();
        
        // Create test file
        let test_file = root_path.join("test.txt");
        fs::write(&test_file, "Hello, World!").await.unwrap();
        
        let config = create_test_config(root_path);
        let handler = StaticFileHandler::new(config);
        
        // Create test request
        let request = Request::builder()
            .method("GET")
            .uri("/test.txt")
            .body(Body::empty())
            .unwrap();
        
        let context = PluginContext::new(crate::plugins::PluginHook::PreHandler);
        let result = handler.handle(&request, &context).await.unwrap();
        
        match result {
            HandlerResult::Response(response) => {
                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(
                    response.headers().get("content-type").unwrap(),
                    "text/plain"
                );
            }
            _ => panic!("Expected response"),
        }
    }
    
    #[tokio::test]
    async fn test_directory_traversal_protection() {
        let temp_dir = tempdir().unwrap();
        let root_path = temp_dir.path().to_path_buf();
        
        let config = create_test_config(root_path);
        let handler = StaticFileHandler::new(config);
        
        // Try directory traversal
        let request = Request::builder()
            .method("GET")
            .uri("/../../../etc/passwd")
            .body(Body::empty())
            .unwrap();
        
        let context = PluginContext::new(crate::plugins::PluginHook::PreHandler);
        let result = handler.handle(&request, &context).await.unwrap();
        
        assert!(matches!(result, HandlerResult::NotFound));
    }
    
    #[tokio::test]
    async fn test_index_file_serving() {
        let temp_dir = tempdir().unwrap();
        let root_path = temp_dir.path().to_path_buf();
        
        // Create index file
        let index_file = root_path.join("index.html");
        fs::write(&index_file, "<html><body>Index Page</body></html>").await.unwrap();
        
        let config = create_test_config(root_path);
        let handler = StaticFileHandler::new(config);
        
        // Request directory
        let request = Request::builder()
            .method("GET")
            .uri("/")
            .body(Body::empty())
            .unwrap();
        
        let context = PluginContext::new(crate::plugins::PluginHook::PreHandler);
        let result = handler.handle(&request, &context).await.unwrap();
        
        match result {
            HandlerResult::Response(response) => {
                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(
                    response.headers().get("content-type").unwrap(),
                    "text/html"
                );
            }
            _ => panic!("Expected response"),
        }
    }
    
    #[test]
    fn test_path_resolution() {
        let temp_dir = tempdir().unwrap();
        let root_path = temp_dir.path().to_path_buf();
        let config = create_test_config(root_path.clone());
        let handler = StaticFileHandler::new(config);
        
        // Valid path
        let resolved = handler.resolve_path("/test.txt");
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap(), root_path.join("test.txt"));
        
        // Directory traversal attempt
        let resolved = handler.resolve_path("/../test.txt");
        assert!(resolved.is_none());
        
        // Another traversal attempt
        let resolved = handler.resolve_path("/subdir/../../test.txt");
        assert!(resolved.is_none());
    }
}