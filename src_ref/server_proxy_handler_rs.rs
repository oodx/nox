use crate::config::{ProxyConfig, UpstreamConfig};
use crate::error::{ServerError, Result};
use crate::handlers::{Handler, HandlerResult};
use crate::plugins::PluginContext;
use async_trait::async_trait;
use hyper::{Body, Request, Response, StatusCode, Uri};
use reqwest::Client;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Upstream server information
#[derive(Debug, Clone)]
pub struct Upstream {
    pub config: UpstreamConfig,
    pub healthy: Arc<std::sync::atomic::AtomicBool>,
    pub request_count: Arc<AtomicUsize>,
}

impl Upstream {
    pub fn new(config: UpstreamConfig) -> Self {
        Self {
            config,
            healthy: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            request_count: Arc::new(AtomicUsize::new(0)),
        }
    }
    
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }
    
    pub fn set_healthy(&self, healthy: bool) {
        self.healthy.store(healthy, Ordering::Relaxed);
    }
    
    pub fn increment_requests(&self) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn get_request_count(&self) -> usize {
        self.request_count.load(Ordering::Relaxed)
    }
}

/// Load balancing strategies
#[derive(Debug, Clone)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    LeastConnections,
    WeightedRoundRobin,
    Random,
}

/// Proxy handler for forwarding requests to upstream servers
pub struct ProxyHandler {
    config: ProxyConfig,
    upstreams: Vec<Upstream>,
    client: Client,
    strategy: LoadBalancingStrategy,
    current_upstream: AtomicUsize,
}

impl ProxyHandler {
    pub fn new(config: ProxyConfig) -> Result<Self> {
        let upstreams = config
            .upstreams
            .iter()
            .map(|upstream_config| Upstream::new(upstream_config.clone()))
            .collect();
        
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout.unwrap_or(30)))
            .build()
            .map_err(|e| ServerError::other(format!("Failed to create HTTP client: {}", e)))?;
        
        Ok(Self {
            config,
            upstreams,
            client,
            strategy: LoadBalancingStrategy::RoundRobin,
            current_upstream: AtomicUsize::new(0),
        })
    }
    
    /// Select upstream server based on load balancing strategy
    fn select_upstream(&self) -> Option<&Upstream> {
        let healthy_upstreams: Vec<&Upstream> = self
            .upstreams
            .iter()
            .filter(|u| u.is_healthy())
            .collect();
        
        if healthy_upstreams.is_empty() {
            return None;
        }
        
        match self.strategy {
            LoadBalancingStrategy::RoundRobin => {
                let index = self.current_upstream.fetch_add(1, Ordering::Relaxed) % healthy_upstreams.len();
                Some(healthy_upstreams[index])
            }
            LoadBalancingStrategy::LeastConnections => {
                healthy_upstreams
                    .iter()
                    .min_by_key(|u| u.get_request_count())
                    .copied()
            }
            LoadBalancingStrategy::WeightedRoundRobin => {
                // Simplified weighted round robin - use first healthy upstream
                // In a full implementation, you'd implement proper weighted selection
                healthy_upstreams.first().copied()
            }
            LoadBalancingStrategy::Random => {
                let index = rand::random::<usize>() % healthy_upstreams.len();
                Some(healthy_upstreams[index])
            }
        }
    }
    
    /// Forward request to upstream server
    async fn forward_request(
        &self,
        upstream: &Upstream,
        original_request: &Request<Body>,
    ) -> Result<Response<Body>> {
        upstream.increment_requests();
        
        // Build upstream URL
        let upstream_url = format!("{}{}", upstream.config.url, original_request.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or(""));
        
        let upstream_uri: Uri = upstream_url.parse()
            .map_err(|e| ServerError::other(format!("Invalid upstream URL: {}", e)))?;
        
        // Convert hyper request to reqwest request
        let mut req_builder = match original_request.method() {
            &hyper::Method::GET => self.client.get(upstream_uri.to_string()),
            &hyper::Method::POST => self.client.post(upstream_uri.to_string()),
            &hyper::Method::PUT => self.client.put(upstream_uri.to_string()),
            &hyper::Method::DELETE => self.client.delete(upstream_uri.to_string()),
            &hyper::Method::PATCH => self.client.patch(upstream_uri.to_string()),
            &hyper::Method::HEAD => self.client.head(upstream_uri.to_string()),
            method => {
                return Err(ServerError::bad_request(format!("Unsupported method: {}", method)));
            }
        };
        
        // Copy headers (excluding hop-by-hop headers)
        for (name, value) in original_request.headers() {
            let header_name = name.as_str().to_lowercase();
            
            // Skip hop-by-hop headers
            if !is_hop_by_hop_header(&header_name) {
                if let Ok(value_str) = value.to_str() {
                    req_builder = req_builder.header(name.as_str(), value_str);
                }
            }
        }
        
        // For requests with body, we'd need to handle the body here
        // This is simplified - in a real implementation, you'd stream the body
        
        // Execute request with timeout
        let timeout_duration = Duration::from_secs(self.config.timeout.unwrap_or(30));
        let response = timeout(timeout_duration, req_builder.send())
            .await
            .map_err(|_| ServerError::Timeout)?
            .map_err(|e| ServerError::other(format!("Upstream request failed: {}", e)))?;
        
        // Convert reqwest response to hyper response
        self.convert_response(response).await
    }
    
    /// Convert reqwest response to hyper response
    async fn convert_response(&self, reqwest_response: reqwest::Response) -> Result<Response<Body>> {
        let status = reqwest_response.status();
        let headers = reqwest_response.headers().clone();
        
        // Get response body
        let body_bytes = reqwest_response.bytes().await
            .map_err(|e| ServerError::other(format!("Failed to read response body: {}", e)))?;
        
        // Build hyper response
        let mut response_builder = Response::builder().status(status.as_u16());
        
        // Copy headers (excluding hop-by-hop headers)
        for (name, value) in &headers {
            let header_name = name.as_str().to_lowercase();
            if !is_hop_by_hop_header(&header_name) {
                response_builder = response_builder.header(name, value);
            }
        }
        
        let response = response_builder
            .body(Body::from(body_bytes))
            .map_err(|e| ServerError::handler(format!("Failed to build response: {}", e)))?;
        
        Ok(response)
    }
    
    /// Handle request with retry logic
    async fn handle_with_retry(&self, request: &Request<Body>) -> Result<Response<Body>> {
        let max_retries = self.config.retry_attempts.unwrap_or(3);
        let mut last_error = None;
        
        for attempt in 0..=max_retries {
            if let Some(upstream) = self.select_upstream() {
                match self.forward_request(upstream, request).await {
                    Ok(response) => return Ok(response),
                    Err(e) => {
                        tracing::warn!(
                            "Request to upstream {} failed (attempt {}): {}",
                            upstream.config.name,
                            attempt + 1,
                            e
                        );
                        last_error = Some(e);
                        
                        // Mark upstream as unhealthy on certain errors
                        if matches!(e, ServerError::Timeout) {
                            upstream.set_healthy(false);
                        }
                    }
                }
            } else {
                return Err(ServerError::service_unavailable("No healthy upstreams available"));
            }
            
            // Wait before retry (exponential backoff)
            if attempt < max_retries {
                let delay = Duration::from_millis(100 * (2_u64.pow(attempt as u32)));
                tokio::time::sleep(delay).await;
            }
        }
        
        Err(last_error.unwrap_or_else(|| ServerError::service_unavailable("All retry attempts failed")))
    }
    
    /// Start health check background task
    pub async fn start_health_checks(&self) {
        if self.upstreams.is_empty() {
            return;
        }
        
        let upstreams = self.upstreams.clone();
        let client = self.client.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                for upstream in &upstreams {
                    if let Some(health_check_path) = &upstream.config.health_check {
                        let health_url = format!("{}{}", upstream.config.url, health_check_path);
                        
                        match client.get(&health_url).send().await {
                            Ok(response) if response.status().is_success() => {
                                upstream.set_healthy(true);
                                tracing::debug!("Upstream {} is healthy", upstream.config.name);
                            }
                            _ => {
                                upstream.set_healthy(false);
                                tracing::warn!("Upstream {} failed health check", upstream.config.name);
                            }
                        }
                    }
                }
            }
        });
    }
    
    /// Get proxy statistics
    pub fn get_stats(&self) -> ProxyStats {
        let upstream_stats: Vec<UpstreamStats> = self
            .upstreams
            .iter()
            .map(|u| UpstreamStats {
                name: u.config.name.clone(),
                url: u.config.url.clone(),
                healthy: u.is_healthy(),
                request_count: u.get_request_count(),
                weight: u.config.weight.unwrap_or(1),
            })
            .collect();
        
        ProxyStats {
            upstreams: upstream_stats,
            total_upstreams: self.upstreams.len(),
            healthy_upstreams: self.upstreams.iter().filter(|u| u.is_healthy()).count(),
        }
    }
}

#[async_trait]
impl Handler for ProxyHandler {
    async fn handle(&self, request: &Request<Body>, _context: &PluginContext) -> Result<HandlerResult> {
        if !self.config.enabled {
            return Ok(HandlerResult::NotFound);
        }
        
        match self.handle_with_retry(request).await {
            Ok(response) => Ok(HandlerResult::Response(response)),
            Err(e) => Ok(HandlerResult::Error(e)),
        }
    }
    
    fn name(&self) -> &str {
        "proxy"
    }
    
    fn description(&self) -> &str {
        "Proxies requests to upstream servers with load balancing"
    }
}

/// Check if header is hop-by-hop and should not be forwarded
fn is_hop_by_hop_header(header_name: &str) -> bool {
    matches!(
        header_name,
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "transfer-encoding"
            | "upgrade"
            | "host" // Host header should be set to upstream host
    )
}

/// Proxy statistics
#[derive(Debug, Clone)]
pub struct ProxyStats {
    pub upstreams: Vec<UpstreamStats>,
    pub total_upstreams: usize,
    pub healthy_upstreams: usize,
}

/// Upstream statistics
#[derive(Debug, Clone)]
pub struct UpstreamStats {
    pub name: String,
    pub url: String,
    pub healthy: bool,
    pub request_count: usize,
    pub weight: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_upstream_creation() {
        let config = UpstreamConfig {
            name: "test".to_string(),
            url: "http://localhost:3000".to_string(),
            weight: Some(1),
            health_check: Some("/health".to_string()),
        };
        
        let upstream = Upstream::new(config);
        assert!(upstream.is_healthy());
        assert_eq!(upstream.get_request_count(), 0);
        
        upstream.increment_requests();
        assert_eq!(upstream.get_request_count(), 1);
        
        upstream.set_healthy(false);
        assert!(!upstream.is_healthy());
    }
    
    #[test]
    fn test_hop_by_hop_headers() {
        assert!(is_hop_by_hop_header("connection"));
        assert!(is_hop_by_hop_header("host"));
        assert!(is_hop_by_hop_header("transfer-encoding"));
        
        assert!(!is_hop_by_hop_header("content-type"));
        assert!(!is_hop_by_hop_header("authorization"));
        assert!(!is_hop_by_hop_header("user-agent"));
    }
    
    #[tokio::test]
    async fn test_proxy_handler_creation() {
        let config = ProxyConfig {
            enabled: true,
            upstreams: vec![
                UpstreamConfig {
                    name: "backend1".to_string(),
                    url: "http://localhost:3001".to_string(),
                    weight: Some(1),
                    health_check: Some("/health".to_string()),
                },
            ],
            timeout: Some(30),
            retry_attempts: Some(3),
        };
        
        let handler = ProxyHandler::new(config);
        assert!(handler.is_ok());
        
        let handler = handler.unwrap();
        assert_eq!(handler.upstreams.len(), 1);
        assert_eq!(handler.upstreams[0].config.name, "backend1");
    }
    
    #[test]
    fn test_load_balancing_selection() {
        let config = ProxyConfig {
            enabled: true,
            upstreams: vec![
                UpstreamConfig {
                    name: "backend1".to_string(),
                    url: "http://localhost:3001".to_string(),
                    weight: Some(1),
                    health_check: None,
                },
                UpstreamConfig {
                    name: "backend2".to_string(),
                    url: "http://localhost:3002".to_string(),
                    weight: Some(1),
                    health_check: None,
                },
            ],
            timeout: Some(30),
            retry_attempts: Some(3),
        };
        
        let handler = ProxyHandler::new(config).unwrap();
        
        // Test round-robin selection
        let upstream1 = handler.select_upstream();
        assert!(upstream1.is_some());
        
        let upstream2 = handler.select_upstream();
        assert!(upstream2.is_some());
        
        // Should cycle through upstreams
        assert_ne!(upstream1.unwrap().config.name, upstream2.unwrap().config.name);
    }
}