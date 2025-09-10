use crate::error::{ServerError, Result};
use crate::handlers::Handler;
use hyper::{Body, Method, Request};
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct Route {
    pub method: Option<Method>,
    pub path: String,
    pub path_regex: Option<String>,
    pub priority: i32,
}

impl Route {
    pub fn new(method: Method, path: impl Into<String>) -> Self {
        Self {
            method: Some(method),
            path: path.into(),
            path_regex: None,
            priority: 100,
        }
    }
    
    pub fn get(path: impl Into<String>) -> Self {
        Self::new(Method::GET, path)
    }
    
    pub fn post(path: impl Into<String>) -> Self {
        Self::new(Method::POST, path)
    }
    
    pub fn put(path: impl Into<String>) -> Self {
        Self::new(Method::PUT, path)
    }
    
    pub fn delete(path: impl Into<String>) -> Self {
        Self::new(Method::DELETE, path)
    }
    
    pub fn patch(path: impl Into<String>) -> Self {
        Self::new(Method::PATCH, path)
    }
    
    pub fn any(path: impl Into<String>) -> Self {
        Self {
            method: None,
            path: path.into(),
            path_regex: None,
            priority: 100,
        }
    }
    
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
    
    pub fn with_regex(mut self, pattern: impl Into<String>) -> Self {
        self.path_regex = Some(pattern.into());
        self
    }
}

#[derive(Debug)]
pub struct RouteMatch {
    pub route: Route,
    pub handler: Arc<dyn Handler>,
    pub params: HashMap<String, String>,
}

pub struct Router {
    routes: RwLock<Vec<(Route, Arc<dyn Handler>, Option<Regex>)>>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            routes: RwLock::new(Vec::new()),
        }
    }
    
    /// Add a route with handler
    pub async fn add_route(&self, route: Route, handler: Arc<dyn Handler>) -> Result<()> {
        let regex = if let Some(pattern) = &route.path_regex {
            Some(Regex::new(pattern)?)
        } else if route.path.contains('{') {
            // Auto-generate regex for path parameters like /users/{id}
            Some(self.path_to_regex(&route.path)?)
        } else {
            None
        };
        
        let mut routes = self.routes.write().await;
        routes.push((route, handler, regex));
        
        // Sort by priority (lower numbers first)
        routes.sort_by_key(|(route, _, _)| route.priority);
        
        Ok(())
    }
    
    /// Find matching route for request
    pub async fn find_route(&self, request: &Request<Body>) -> Option<RouteMatch> {
        let routes = self.routes.read().await;
        let method = request.method();
        let path = request.uri().path();
        
        for (route, handler, regex) in routes.iter() {
            // Check method match
            if let Some(route_method) = &route.method {
                if route_method != method {
                    continue;
                }
            }
            
            // Check path match
            let (matches, params) = if let Some(regex) = regex {
                self.matches_regex(regex, path)
            } else {
                (route.path == path, HashMap::new())
            };
            
            if matches {
                return Some(RouteMatch {
                    route: route.clone(),
                    handler: Arc::clone(handler),
                    params,
                });
            }
        }
        
        None
    }
    
    /// Check if path matches regex and extract parameters
    fn matches_regex(&self, regex: &Regex, path: &str) -> (bool, HashMap<String, String>) {
        if let Some(captures) = regex.captures(path) {
            let mut params = HashMap::new();
            
            // Extract named capture groups
            for name in regex.capture_names().flatten() {
                if let Some(matched) = captures.name(name) {
                    params.insert(name.to_string(), matched.as_str().to_string());
                }
            }
            
            (true, params)
        } else {
            (false, HashMap::new())
        }
    }
    
    /// Convert path pattern to regex (supports {param} syntax)
    fn path_to_regex(&self, path: &str) -> Result<Regex> {
        let mut regex_pattern = regex::escape(path);
        
        // Replace {param} with named capture groups
        let param_regex = Regex::new(r"\\\{([^}]+)\\\}")?;
        regex_pattern = param_regex.replace_all(&regex_pattern, r"(?P<$1>[^/]+)").to_string();
        
        // Add anchors
        regex_pattern = format!("^{}$", regex_pattern);
        
        Ok(Regex::new(&regex_pattern)?)
    }
    
    /// List all registered routes
    pub async fn list_routes(&self) -> Vec<(String, Option<String>, i32)> {
        let routes = self.routes.read().await;
        routes
            .iter()
            .map(|(route, _, _)| {
                (
                    route.path.clone(),
                    route.method.as_ref().map(|m| m.to_string()),
                    route.priority,
                )
            })
            .collect()
    }
    
    /// Remove all routes (useful for testing)
    pub async fn clear_routes(&self) {
        let mut routes = self.routes.write().await;
        routes.clear();
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating complex routing configurations
pub struct RouterBuilder {
    routes: Vec<(Route, Arc<dyn Handler>)>,
}

impl RouterBuilder {
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
        }
    }
    
    pub fn route(mut self, route: Route, handler: Arc<dyn Handler>) -> Self {
        self.routes.push((route, handler));
        self
    }
    
    pub fn get(self, path: impl Into<String>, handler: Arc<dyn Handler>) -> Self {
        self.route(Route::get(path), handler)
    }
    
    pub fn post(self, path: impl Into<String>, handler: Arc<dyn Handler>) -> Self {
        self.route(Route::post(path), handler)
    }
    
    pub fn put(self, path: impl Into<String>, handler: Arc<dyn Handler>) -> Self {
        self.route(Route::put(path), handler)
    }
    
    pub fn delete(self, path: impl Into<String>, handler: Arc<dyn Handler>) -> Self {
        self.route(Route::delete(path), handler)
    }
    
    pub fn patch(self, path: impl Into<String>, handler: Arc<dyn Handler>) -> Self {
        self.route(Route::patch(path), handler)
    }
    
    pub fn any(self, path: impl Into<String>, handler: Arc<dyn Handler>) -> Self {
        self.route(Route::any(path), handler)
    }
    
    /// Apply all routes to a router
    pub async fn build_into(self, router: &Router) -> Result<()> {
        for (route, handler) in self.routes {
            router.add_route(route, handler).await?;
        }
        Ok(())
    }
}

impl Default for RouterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::MockHandler;
    
    #[tokio::test]
    async fn test_router_exact_match() {
        let router = Router::new();
        let handler = Arc::new(MockHandler::new());
        
        router.add_route(Route::get("/test"), handler).await.unwrap();
        
        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        
        let route_match = router.find_route(&request).await;
        assert!(route_match.is_some());
    }
    
    #[tokio::test]
    async fn test_router_path_parameters() {
        let router = Router::new();
        let handler = Arc::new(MockHandler::new());
        
        router.add_route(Route::get("/users/{id}"), handler).await.unwrap();
        
        let request = Request::builder()
            .method(Method::GET)
            .uri("/users/123")
            .body(Body::empty())
            .unwrap();
        
        let route_match = router.find_route(&request).await;
        assert!(route_match.is_some());
        
        let route_match = route_match.unwrap();
        assert_eq!(route_match.params.get("id"), Some(&"123".to_string()));
    }
    
    #[tokio::test]
    async fn test_router_method_mismatch() {
        let router = Router::new();
        let handler = Arc::new(MockHandler::new());
        
        router.add_route(Route::get("/test"), handler).await.unwrap();
        
        let request = Request::builder()
            .method(Method::POST)
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        
        let route_match = router.find_route(&request).await;
        assert!(route_match.is_none());
    }
}