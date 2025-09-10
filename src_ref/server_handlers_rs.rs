use crate::error::{ServerError, Result};
use crate::plugins::PluginContext;
use async_trait::async_trait;
use hyper::{Body, Request, Response};

pub mod static_files;
pub mod proxy;

pub use static_files::StaticFileHandler;
pub use proxy::ProxyHandler;

/// Result of handler execution
#[derive(Debug)]
pub enum HandlerResult {
    Response(Response<Body>),
    NotFound,
    Error(ServerError),
}

impl HandlerResult {
    pub fn ok(response: Response<Body>) -> Self {
        Self::Response(response)
    }
    
    pub fn not_found() -> Self {
        Self::NotFound
    }
    
    pub fn error(error: ServerError) -> Self {
        Self::Error(error)
    }
}

/// Main handler trait for processing requests
#[async_trait]
pub trait Handler: Send + Sync {
    /// Handle a request and return a response
    async fn handle(&self, request: &Request<Body>, context: &PluginContext) -> Result<HandlerResult>;
    
    /// Handler name for debugging and logging
    fn name(&self) -> &str {
        "unknown"
    }
    
    /// Handler description
    fn description(&self) -> &str {
        "No description"
    }
}

/// Simple function handler wrapper
pub struct FunctionHandler<F>
where
    F: Fn(&Request<Body>, &PluginContext) -> Result<HandlerResult> + Send + Sync,
{
    name: String,
    handler_fn: F,
}

impl<F> FunctionHandler<F>
where
    F: Fn(&Request<Body>, &PluginContext) -> Result<HandlerResult> + Send + Sync,
{
    pub fn new(name: impl Into<String>, handler_fn: F) -> Self {
        Self {
            name: name.into(),
            handler_fn,
        }
    }
}

#[async_trait]
impl<F> Handler for FunctionHandler<F>
where
    F: Fn(&Request<Body>, &PluginContext) -> Result<HandlerResult> + Send + Sync,
{
    async fn handle(&self, request: &Request<Body>, context: &PluginContext) -> Result<HandlerResult> {
        (self.handler_fn)(request, context)
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}

/// Async function handler wrapper
pub struct AsyncFunctionHandler<F, Fut>
where
    F: Fn(&Request<Body>, &PluginContext) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<HandlerResult>> + Send,
{
    name: String,
    handler_fn: F,
}

impl<F, Fut> AsyncFunctionHandler<F, Fut>
where
    F: Fn(&Request<Body>, &PluginContext) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<HandlerResult>> + Send,
{
    pub fn new(name: impl Into<String>, handler_fn: F) -> Self {
        Self {
            name: name.into(),
            handler_fn,
        }
    }
}

#[async_trait]
impl<F, Fut> Handler for AsyncFunctionHandler<F, Fut>
where
    F: Fn(&Request<Body>, &PluginContext) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<HandlerResult>> + Send,
{
    async fn handle(&self, request: &Request<Body>, context: &PluginContext) -> Result<HandlerResult> {
        (self.handler_fn)(request, context).await
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}

/// JSON response handler
pub struct JsonHandler {
    name: String,
    data: serde_json::Value,
    status: hyper::StatusCode,
}

impl JsonHandler {
    pub fn new(name: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            data,
            status: hyper::StatusCode::OK,
        }
    }
    
    pub fn with_status(mut self, status: hyper::StatusCode) -> Self {
        self.status = status;
        self
    }
}

#[async_trait]
impl Handler for JsonHandler {
    async fn handle(&self, _request: &Request<Body>, _context: &PluginContext) -> Result<HandlerResult> {
        let response = Response::builder()
            .status(self.status)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&self.data)?))
            .map_err(|e| ServerError::handler(format!("Failed to build JSON response: {}", e)))?;
        
        Ok(HandlerResult::Response(response))
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    fn description(&self) -> &str {
        "JSON response handler"
    }
}

/// Text response handler
pub struct TextHandler {
    name: String,
    content: String,
    content_type: String,
    status: hyper::StatusCode,
}

impl TextHandler {
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
            content_type: "text/plain".to_string(),
            status: hyper::StatusCode::OK,
        }
    }
    
    pub fn html(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
            content_type: "text/html".to_string(),
            status: hyper::StatusCode::OK,
        }
    }
    
    pub fn with_status(mut self, status: hyper::StatusCode) -> Self {
        self.status = status;
        self
    }
    
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = content_type.into();
        self
    }
}

#[async_trait]
impl Handler for TextHandler {
    async fn handle(&self, _request: &Request<Body>, _context: &PluginContext) -> Result<HandlerResult> {
        let response = Response::builder()
            .status(self.status)
            .header("content-type", &self.content_type)
            .body(Body::from(self.content.clone()))
            .map_err(|e| ServerError::handler(format!("Failed to build text response: {}", e)))?;
        
        Ok(HandlerResult::Response(response))
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    fn description(&self) -> &str {
        "Text response handler"
    }
}

/// Redirect handler
pub struct RedirectHandler {
    name: String,
    location: String,
    permanent: bool,
}

impl RedirectHandler {
    pub fn new(name: impl Into<String>, location: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            location: location.into(),
            permanent: false,
        }
    }
    
    pub fn permanent(mut self) -> Self {
        self.permanent = true;
        self
    }
}

#[async_trait]
impl Handler for RedirectHandler {
    async fn handle(&self, _request: &Request<Body>, _context: &PluginContext) -> Result<HandlerResult> {
        let status = if self.permanent {
            hyper::StatusCode::MOVED_PERMANENTLY
        } else {
            hyper::StatusCode::FOUND
        };
        
        let response = Response::builder()
            .status(status)
            .header("location", &self.location)
            .body(Body::empty())
            .map_err(|e| ServerError::handler(format!("Failed to build redirect response: {}", e)))?;
        
        Ok(HandlerResult::Response(response))
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    fn description(&self) -> &str {
        "Redirect handler"
    }
}

/// Not found handler
pub struct NotFoundHandler {
    name: String,
    message: String,
}

impl NotFoundHandler {
    pub fn new() -> Self {
        Self {
            name: "not_found".to_string(),
            message: "Not Found".to_string(),
        }
    }
    
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }
}

#[async_trait]
impl Handler for NotFoundHandler {
    async fn handle(&self, _request: &Request<Body>, _context: &PluginContext) -> Result<HandlerResult> {
        let body = serde_json::json!({
            "error": {
                "message": self.message,
                "status": 404
            }
        });
        
        let response = Response::builder()
            .status(hyper::StatusCode::NOT_FOUND)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body)?))
            .map_err(|e| ServerError::handler(format!("Failed to build not found response: {}", e)))?;
        
        Ok(HandlerResult::Response(response))
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    fn description(&self) -> &str {
        "Not found handler"
    }
}

impl Default for NotFoundHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock handler for testing
#[cfg(test)]
pub struct MockHandler {
    name: String,
    response: HandlerResult,
}

#[cfg(test)]
impl MockHandler {
    pub fn new() -> Self {
        Self {
            name: "mock".to_string(),
            response: HandlerResult::Response(
                Response::builder()
                    .status(hyper::StatusCode::OK)
                    .body(Body::from("Mock response"))
                    .unwrap()
            ),
        }
    }
    
    pub fn with_response(mut self, response: HandlerResult) -> Self {
        self.response = response;
        self
    }
}

#[cfg(test)]
#[async_trait]
impl Handler for MockHandler {
    async fn handle(&self, _request: &Request<Body>, _context: &PluginContext) -> Result<HandlerResult> {
        // Clone the response (simplified for testing)
        match &self.response {
            HandlerResult::Response(_) => Ok(HandlerResult::Response(
                Response::builder()
                    .status(hyper::StatusCode::OK)
                    .body(Body::from("Mock response"))
                    .unwrap()
            )),
            HandlerResult::NotFound => Ok(HandlerResult::NotFound),
            HandlerResult::Error(e) => Ok(HandlerResult::Error(ServerError::handler(e.to_string()))),
        }
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}

/// Convenience functions for creating handlers
pub fn json_handler(name: impl Into<String>, data: serde_json::Value) -> JsonHandler {
    JsonHandler::new(name, data)
}

pub fn text_handler(name: impl Into<String>, content: impl Into<String>) -> TextHandler {
    TextHandler::new(name, content)
}

pub fn html_handler(name: impl Into<String>, content: impl Into<String>) -> TextHandler {
    TextHandler::html(name, content)
}

pub fn redirect_handler(name: impl Into<String>, location: impl Into<String>) -> RedirectHandler {
    RedirectHandler::new(name, location)
}

pub fn not_found_handler() -> NotFoundHandler {
    NotFoundHandler::new()
}

/// Router builder for fluent route configuration
pub use crate::server::router::RouterBuilder;