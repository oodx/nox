use crate::error::{ServerError, Result};
use async_trait::async_trait;
use hyper::{Request, Response, Body};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

pub mod manager;
pub mod auth;
pub mod session;
pub mod mock;
pub mod health;
pub mod logging;
pub mod static_files;

pub use manager::PluginManager;

/// Plugin hook points in the request/response lifecycle
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PluginHook {
    OnStartup,
    OnShutdown,
    PreRequest,
    PostRoute,
    PreHandler,
    PostHandler,
    PreResponse,
    PostResponse,
    OnError,
}

/// Context passed to plugins containing request information and metadata
#[derive(Debug, Clone)]
pub struct PluginContext {
    pub hook: PluginHook,
    pub path: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub query: HashMap<String, String>,
    pub route_params: HashMap<String, String>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub session_id: Option<String>,
    pub user_id: Option<String>,
}

impl PluginContext {
    pub fn new(hook: PluginHook) -> Self {
        Self {
            hook,
            path: String::new(),
            method: String::new(),
            headers: HashMap::new(),
            query: HashMap::new(),
            route_params: HashMap::new(),
            metadata: HashMap::new(),
            session_id: None,
            user_id: None,
        }
    }
    
    pub fn from_request(hook: PluginHook, request: &Request<Body>) -> Self {
        let mut context = Self::new(hook);
        context.path = request.uri().path().to_string();
        context.method = request.method().to_string();
        
        // Extract headers
        for (name, value) in request.headers() {
            if let Ok(value_str) = value.to_str() {
                context.headers.insert(name.to_string(), value_str.to_string());
            }
        }
        
        // Extract query parameters
        if let Some(query) = request.uri().query() {
            for pair in query.split('&') {
                if let Some((key, value)) = pair.split_once('=') {
                    context.query.insert(
                        urlencoding::decode(key).unwrap_or_default().to_string(),
                        urlencoding::decode(value).unwrap_or_default().to_string(),
                    );
                }
            }
        }
        
        context
    }
    
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
    
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
    
    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }
    
    pub fn with_route_params(mut self, params: HashMap<String, String>) -> Self {
        self.route_params = params;
        self
    }
}

/// Plugin execution result
#[derive(Debug)]
pub enum PluginResult {
    Continue,
    Stop,
    Error(ServerError),
    Response(Response<Body>),
}

impl PluginResult {
    pub fn is_continue(&self) -> bool {
        matches!(self, Self::Continue)
    }
    
    pub fn is_stop(&self) -> bool {
        matches!(self, Self::Stop)
    }
    
    pub fn into_error(self) -> Option<ServerError> {
        match self {
            Self::Error(e) => Some(e),
            _ => None,
        }
    }
    
    pub fn into_response(self) -> Option<Response<Body>> {
        match self {
            Self::Response(r) => Some(r),
            _ => None,
        }
    }
}

/// Main plugin trait that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Plugin name
    fn name(&self) -> &str;
    
    /// Plugin version
    fn version(&self) -> &str;
    
    /// Plugin description
    fn description(&self) -> &str;
    
    /// Initialize the plugin with configuration
    async fn initialize(&mut self, config: &serde_yaml::Value) -> Result<()>;
    
    /// Check if plugin should handle this hook
    fn handles_hook(&self, hook: &PluginHook) -> bool;
    
    /// Plugin priority (lower numbers run first)
    fn priority(&self) -> i32 {
        100
    }
    
    /// Handle server startup
    async fn on_startup(&self, _context: &PluginContext) -> Result<PluginResult> {
        Ok(PluginResult::Continue)
    }
    
    /// Handle server shutdown
    async fn on_shutdown(&self, _context: &PluginContext) -> Result<PluginResult> {
        Ok(PluginResult::Continue)
    }
    
    /// Handle pre-request processing
    async fn pre_request(
        &self,
        _request: &mut Request<Body>,
        _context: &PluginContext,
    ) -> Result<PluginResult> {
        Ok(PluginResult::Continue)
    }
    
    /// Handle post-route processing (after route matching)
    async fn post_route(
        &self,
        _request: &Request<Body>,
        _context: &PluginContext,
    ) -> Result<PluginResult> {
        Ok(PluginResult::Continue)
    }
    
    /// Handle pre-handler processing
    async fn pre_handler(
        &self,
        _request: &Request<Body>,
        _context: &PluginContext,
    ) -> Result<PluginResult> {
        Ok(PluginResult::Continue)
    }
    
    /// Handle post-handler processing
    async fn post_handler(
        &self,
        _request: &Request<Body>,
        _response: &mut Response<Body>,
        _context: &PluginContext,
    ) -> Result<PluginResult> {
        Ok(PluginResult::Continue)
    }
    
    /// Handle pre-response processing
    async fn pre_response(
        &self,
        _response: &mut Response<Body>,
        _context: &PluginContext,
    ) -> Result<PluginResult> {
        Ok(PluginResult::Continue)
    }
    
    /// Handle post-response processing
    async fn post_response(
        &self,
        _response: &Response<Body>,
        _context: &PluginContext,
    ) -> Result<PluginResult> {
        Ok(PluginResult::Continue)
    }
    
    /// Handle error cases
    async fn on_error(
        &self,
        _error: &ServerError,
        _context: &PluginContext,
    ) -> Result<PluginResult> {
        Ok(PluginResult::Continue)
    }
}

/// Plugin information for registration and management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub priority: i32,
    pub enabled: bool,
    pub hooks: Vec<String>,
}

impl PluginInfo {
    pub fn from_plugin(plugin: &dyn Plugin) -> Self {
        let hooks = [
            PluginHook::OnStartup,
            PluginHook::OnShutdown,
            PluginHook::PreRequest,
            PluginHook::PostRoute,
            PluginHook::PreHandler,
            PluginHook::PostHandler,
            PluginHook::PreResponse,
            PluginHook::PostResponse,
            PluginHook::OnError,
        ]
        .iter()
        .filter(|hook| plugin.handles_hook(hook))
        .map(|hook| format!("{:?}", hook))
        .collect();
        
        Self {
            name: plugin.name().to_string(),
            version: plugin.version().to_string(),
            description: plugin.description().to_string(),
            priority: plugin.priority(),
            enabled: true,
            hooks,
        }
    }
}