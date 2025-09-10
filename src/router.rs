use crate::config::{MockConfig, MockRoute, MockResponse};
use hyper::{Request, Response, Method, StatusCode};
use hyper::body::Incoming;
use http_body_util::Full;
use bytes::Bytes;
use std::convert::Infallible;

pub struct MockRouter {
    routes: Vec<RouteMatcher>,
}

#[derive(Debug, Clone)]
struct RouteMatcher {
    path_pattern: String,
    method: Method,
    response: MockResponse,
}

impl MockRouter {
    pub fn new() -> Self {
        let mut router = Self {
            routes: Vec::new(),
        };
        
        // Add default routes
        router.add_default_routes();
        router
    }

    fn add_default_routes(&mut self) {
        use crate::config::MockResponse;
        use std::collections::HashMap;

        // Default health endpoint
        self.routes.push(RouteMatcher {
            path_pattern: "/health".to_string(),
            method: Method::GET,
            response: MockResponse {
                status: 200,
                headers: None,
                body: "OK".to_string(),
            },
        });

        // Default root endpoint
        self.routes.push(RouteMatcher {
            path_pattern: "/".to_string(),
            method: Method::GET,
            response: MockResponse {
                status: 200,
                headers: Some({
                    let mut headers = HashMap::new();
                    headers.insert("X-Server".to_string(), "NOX".to_string());
                    headers
                }),
                body: "NOX Server - Mock Ready".to_string(),
            },
        });

        // Secret handshake endpoint for kick <-> nox identification
        self.routes.push(RouteMatcher {
            path_pattern: "/nox/handshake".to_string(),
            method: Method::GET,
            response: MockResponse {
                status: 200,
                headers: Some({
                    let mut headers = HashMap::new();
                    headers.insert("X-Server".to_string(), "NOX".to_string());
                    headers.insert("X-Handshake".to_string(), "kick-nox-v1".to_string());
                    headers
                }),
                body: r#"{"server":"nox","version":"0.1.0","handshake":"kick-nox-v1","capabilities":["mock","health","config"]}"#.to_string(),
            },
        });
    }

    pub fn from_config(config: &MockConfig) -> Self {
        let mut router = Self::new();
        
        for scenario in &config.scenarios {
            for route in &scenario.routes {
                router.add_route(route);
            }
        }
        
        router
    }

    fn add_route(&mut self, route: &MockRoute) {
        if let Ok(method) = route.method.parse::<Method>() {
            self.routes.push(RouteMatcher {
                path_pattern: route.path.clone(),
                method,
                response: route.response.clone(),
            });
        }
    }

    pub async fn handle_request(&self, req: Request<Incoming>) -> std::result::Result<Response<Full<Bytes>>, Infallible> {
        let path = req.uri().path();
        let method = req.method();

        // Try to match routes
        for route in &self.routes {
            if self.matches_route(&route, path, method) {
                return Ok(self.create_response(&route.response));
            }
        }

        // Default fallback
        Ok(self.create_not_found_response())
    }

    fn matches_route(&self, route: &RouteMatcher, path: &str, method: &Method) -> bool {
        if route.method != *method {
            return false;
        }

        // Simple exact match for now - can be enhanced with path parameters
        route.path_pattern == path
    }

    fn create_response(&self, mock_response: &MockResponse) -> Response<Full<Bytes>> {
        let mut builder = Response::builder()
            .status(StatusCode::from_u16(mock_response.status).unwrap_or(StatusCode::OK));

        // Add headers if configured
        if let Some(headers) = &mock_response.headers {
            for (key, value) in headers {
                builder = builder.header(key, value);
            }
        }

        builder
            .body(Full::new(Bytes::from(mock_response.body.clone())))
            .unwrap()
    }

    fn create_not_found_response(&self) -> Response<Full<Bytes>> {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from("Not Found")))
            .unwrap()
    }
}

impl Default for MockRouter {
    fn default() -> Self {
        Self::new()
    }
}