use thiserror::Error;

pub type Result<T> = std::result::Result<T, ServerError>;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("HTTP error: {0}")]
    Http(#[from] hyper::Error),
    
    #[error("HTTP parse error: {0}")]
    HttpParse(#[from] http::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Plugin error: {0}")]
    Plugin(String),
    
    #[error("Authentication error: {0}")]
    Auth(String),
    
    #[error("Session error: {0}")]
    Session(String),
    
    #[error("Handler error: {0}")]
    Handler(String),
    
    #[error("Template error: {0}")]
    Template(#[from] handlebars::RenderError),
    
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
    
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),
    
    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),
    
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Redis error: {0}")]
    Redis(String),
    
    #[error("File watch error: {0}")]
    FileWatch(#[from] notify::Error),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Bad request: {0}")]
    BadRequest(String),
    
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    
    #[error("Forbidden: {0}")]
    Forbidden(String),
    
    #[error("Internal server error: {0}")]
    Internal(String),
    
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
    
    #[error("Timeout")]
    Timeout,
    
    #[error("Unknown error: {0}")]
    Other(String),
}

impl ServerError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }
    
    pub fn plugin(msg: impl Into<String>) -> Self {
        Self::Plugin(msg.into())
    }
    
    pub fn auth(msg: impl Into<String>) -> Self {
        Self::Auth(msg.into())
    }
    
    pub fn session(msg: impl Into<String>) -> Self {
        Self::Session(msg.into())
    }
    
    pub fn handler(msg: impl Into<String>) -> Self {
        Self::Handler(msg.into())
    }
    
    pub fn database(msg: impl Into<String>) -> Self {
        Self::Database(msg.into())
    }
    
    pub fn redis(msg: impl Into<String>) -> Self {
        Self::Redis(msg.into())
    }
    
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }
    
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }
    
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::Unauthorized(msg.into())
    }
    
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }
    
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
    
    pub fn service_unavailable(msg: impl Into<String>) -> Self {
        Self::ServiceUnavailable(msg.into())
    }
    
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
    
    /// Convert error to HTTP status code
    pub fn to_status_code(&self) -> hyper::StatusCode {
        match self {
            Self::NotFound(_) => hyper::StatusCode::NOT_FOUND,
            Self::BadRequest(_) => hyper::StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => hyper::StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => hyper::StatusCode::FORBIDDEN,
            Self::Timeout => hyper::StatusCode::REQUEST_TIMEOUT,
            Self::ServiceUnavailable(_) => hyper::StatusCode::SERVICE_UNAVAILABLE,
            Self::Auth(_) => hyper::StatusCode::UNAUTHORIZED,
            _ => hyper::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
    
    /// Check if error should be logged as error level
    pub fn is_error_level(&self) -> bool {
        !matches!(self, 
            Self::NotFound(_) | 
            Self::BadRequest(_) | 
            Self::Unauthorized(_) | 
            Self::Forbidden(_)
        )
    }
}

#[cfg(feature = "sqlx")]
impl From<sqlx::Error> for ServerError {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(err.to_string())
    }
}

#[cfg(feature = "redis")]
impl From<redis::RedisError> for ServerError {
    fn from(err: redis::RedisError) -> Self {
        Self::Redis(err.to_string())
    }
}