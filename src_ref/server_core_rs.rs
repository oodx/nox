use crate::config::{Config, ConfigManager};
use crate::error::{ServerError, Result};
use crate::handlers::{Handler, HandlerResult, RouterBuilder};
use crate::plugins::{PluginContext, PluginHook, PluginManager};
use crate::utils::logging::RequestLogger;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Body, Request, Response, StatusCode};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

pub mod router;

pub use router::{Router, Route, RouteMatch};

pub struct Server {
    config_manager: Arc<ConfigManager>,
    plugin_manager: Arc<RwLock<PluginManager>>,
    router: Arc<Router>,
    request_logger: Arc<RequestLogger>,
}

impl Server {
    pub fn new(config: Config) -> Result<Self> {
        let config_manager = Arc::new(ConfigManager::new(config.clone()));
        let plugin_manager = Arc::new(RwLock::new(PluginManager::new()));
        let router = Arc::new(Router::new());
        let request_logger = Arc::new(RequestLogger::new(config.logging.request_logging));
        
        Ok(Self {
            config_manager,
            plugin_manager,
            router,
            request_logger,
        })
    }
    
    pub fn with_hot_reload(config: Config, config_path: impl AsRef<std::path::Path>) -> Result<Self> {
        let config_manager = Arc::new(ConfigManager::with_hot_reload(config.clone(), config_path)?);
        let plugin_manager = Arc::new(RwLock::new(PluginManager::new()));
        let router = Arc::new(Router::new());
        let request_logger = Arc::new(RequestLogger::new(config.logging.request_logging));
        
        Ok(Self {
            config_manager,
            plugin_manager,
            router,
            request_logger,
        })
    }
    
    /// Register a plugin
    pub async fn register_plugin(&self, plugin: Arc<dyn crate::plugins::Plugin>) -> Result<()> {
        let mut manager = self.plugin_manager.write().await;
        manager.register_plugin(plugin)
    }
    
    /// Add a route handler
    pub async fn add_route(&self, route: Route, handler: Arc<dyn Handler>) -> Result<()> {
        self.router.add_route(route, handler).await
    }
    
    /// Start the server
    pub async fn start(&self) -> Result<()> {
        let config = self.config_manager.get().await;
        let addr: SocketAddr = config.bind_address().parse()
            .map_err(|e| ServerError::config(format!("Invalid bind address: {}", e)))?;
        
        tracing::info!("Starting server on {}", addr);
        
        // Initialize plugins
        let startup_context = PluginContext::new(PluginHook::OnStartup);
        {
            let plugin_manager = self.plugin_manager.read().await;
            plugin_manager.execute_startup(&startup_context).await?;
        }
        
        let listener = TcpListener::bind(addr).await?;
        tracing::info!("Server listening on {}", addr);
        
        loop {
            match listener.accept().await {
                Ok((stream, remote_addr)) => {
                    let server = Arc::new(self.clone());
                    tokio::spawn(async move {
                        if let Err(e) = server.handle_connection(stream, remote_addr).await {
                            tracing::error!("Connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("Failed to accept connection: {}", e);
                }
            }
        }
    }
    
    /// Handle individual connection
    async fn handle_connection(&self, stream: TcpStream, remote_addr: SocketAddr) -> Result<()> {
        let server = Arc::new(self.clone());
        
        let service = service_fn(move |req| {
            let server = Arc::clone(&server);
            async move {
                match server.handle_request(req, remote_addr).await {
                    Ok(response) => Ok::<_, Infallible>(response),
                    Err(e) => {
                        tracing::error!("Request handling error: {}", e);
                        let error_response = create_error_response(&e);
                        Ok(error_response)
                    }
                }
            }
        });
        
        http1::Builder::new()
            .serve_connection(stream, service)
            .await
            .map_err(ServerError::Http)?;
        
        Ok(())
    }
    
    /// Handle individual request
    async fn handle_request(&self, mut request: Request<Body>, remote_addr: SocketAddr) -> Result<Response<Body>> {
        let start_time = Instant::now();
        let method = request.method().to_string();
        let path = request.uri().path().to_string();
        
        // Log incoming request
        self.request_logger.log_request(&method, &path, Some(&remote_addr.to_string()));
        
        // Create plugin context
        let mut context = PluginContext::from_request(PluginHook::PreRequest, &request);
        
        // Execute pre-request plugins
        {
            let plugin_manager = self.plugin_manager.read().await;
            if let Some(response) = plugin_manager.execute_pre_request(&mut request, &context).await? {
                let duration = start_time.elapsed().as_millis() as u64;
                self.request_logger.log_response(&method, &path, response.status().as_u16(), duration);
                return Ok(response);
            }
        }
        
        // Route matching
        let route_match = self.router.find_route(&request).await;
        context.route_params = route_match.as_ref()
            .map(|m| m.params.clone())
            .unwrap_or_default();
        
        // Execute post-route plugins
        context.hook = PluginHook::PostRoute;
        {
            let plugin_manager = self.plugin_manager.read().await;
            if let Some(response) = plugin_manager.execute_post_route(&request, &context).await? {
                let duration = start_time.elapsed().as_millis() as u64;
                self.request_logger.log_response(&method, &path, response.status().as_u16(), duration);
                return Ok(response);
            }
        }
        
        // Execute pre-handler plugins
        context.hook = PluginHook::PreHandler;
        {
            let plugin_manager = self.plugin_manager.read().await;
            if let Some(response) = plugin_manager.execute_pre_handler(&request, &context).await? {
                let duration = start_time.elapsed().as_millis() as u64;
                self.request_logger.log_response(&method, &path, response.status().as_u16(), duration);
                return Ok(response);
            }
        }
        
        // Handle request
        let mut response = if let Some(route_match) = route_match {
            match route_match.handler.handle(&request, &context).await? {
                HandlerResult::Response(response) => response,
                HandlerResult::NotFound => create_not_found_response(),
                HandlerResult::Error(e) => return Err(e),
            }
        } else {
            create_not_found_response()
        };
        
        // Execute post-handler plugins
        context.hook = PluginHook::PostHandler;
        {
            let plugin_manager = self.plugin_manager.read().await;
            plugin_manager.execute_post_handler(&request, &mut response, &context).await?;
        }
        
        // Execute pre-response plugins
        context.hook = PluginHook::PreResponse;
        {
            let plugin_manager = self.plugin_manager.read().await;
            plugin_manager.execute_pre_response(&mut response, &context).await?;
        }
        
        let status = response.status().as_u16();
        let duration = start_time.elapsed().as_millis() as u64;
        
        // Execute post-response plugins
        context.hook = PluginHook::PostResponse;
        {
            let plugin_manager = self.plugin_manager.read().await;
            plugin_manager.execute_post_response(&response, &context).await?;
        }
        
        // Log response
        self.request_logger.log_response(&method, &path, status, duration);
        
        Ok(response)
    }
    
    /// Get current configuration
    pub async fn config(&self) -> Config {
        self.config_manager.get().await
    }
    
    /// Shutdown the server gracefully
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down server");
        
        let shutdown_context = PluginContext::new(PluginHook::OnShutdown);
        let plugin_manager = self.plugin_manager.read().await;
        plugin_manager.execute_shutdown(&shutdown_context).await?;
        
        tracing::info!("Server shutdown complete");
        Ok(())
    }
}

impl Clone for Server {
    fn clone(&self) -> Self {
        Self {
            config_manager: Arc::clone(&self.config_manager),
            plugin_manager: Arc::clone(&self.plugin_manager),
            router: Arc::clone(&self.router),
            request_logger: Arc::clone(&self.request_logger),
        }
    }
}

fn create_error_response(error: &ServerError) -> Response<Body> {
    let status = error.to_status_code();
    let body = serde_json::json!({
        "error": {
            "message": error.to_string(),
            "status": status.as_u16()
        }
    });
    
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap_or_default()))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })
}

fn create_not_found_response() -> Response<Body> {
    let body = serde_json::json!({
        "error": {
            "message": "Not Found",
            "status": 404
        }
    });
    
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap_or_default()))
        .unwrap()
}