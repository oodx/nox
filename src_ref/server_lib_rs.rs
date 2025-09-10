pub mod error;
pub mod config;
pub mod plugins;
pub mod server;
pub mod handlers;
pub mod session;
pub mod auth;
pub mod utils;
pub mod cli;
pub mod adapters;

pub use error::{ServerError, Result};
pub use config::Config;
pub use server::Server;
pub use plugins::{Plugin, PluginManager};

/// Re-export commonly used types
pub mod prelude {
    pub use crate::{
        error::{ServerError, Result},
        config::Config,
        server::Server,
        plugins::{Plugin, PluginManager, PluginContext, PluginHook},
        handlers::{Handler, HandlerResult},
        session::{Session, SessionManager},
        auth::{AuthProvider, AuthResult},
    };
    pub use async_trait::async_trait;
    pub use serde::{Deserialize, Serialize};
    pub use std::collections::HashMap;
    pub use hyper::{Request, Response, Body, Method, StatusCode};
    pub use bytes::Bytes;
}