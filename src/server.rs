use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper::body::Incoming;
use http_body_util::Full;
use hyper_util::rt::TokioIo;
use bytes::Bytes;
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use std::sync::Arc;
use crate::Result;
use crate::router::MockRouter;

#[cfg(feature = "config")]
use crate::config::NoxConfig;

pub struct NoxServer {
    addr: SocketAddr,
    router: Arc<MockRouter>,
}

impl NoxServer {
    pub fn new(addr: SocketAddr) -> Self {
        Self { 
            addr,
            router: Arc::new(MockRouter::new()),
        }
    }

    #[cfg(feature = "config")]
    pub fn from_config(config: &NoxConfig) -> Self {
        let addr = format!("{}:{}", config.server.host, config.server.port)
            .parse()
            .unwrap_or_else(|_| "127.0.0.1:3000".parse().unwrap());
        
        let router = if let Some(mock_config) = &config.mock {
            Arc::new(MockRouter::from_config(mock_config))
        } else {
            Arc::new(MockRouter::new())
        };

        Self { addr, router }
    }

    pub async fn run(self) -> Result<()> {
        let listener = TcpListener::bind(self.addr).await?;
        println!("NOX Server running on http://{}", self.addr);

        loop {
            let (stream, _) = listener.accept().await?;
            let io = TokioIo::new(stream);
            let router = Arc::clone(&self.router);

            tokio::task::spawn(async move {
                let service = service_fn(move |req| {
                    let router = Arc::clone(&router);
                    async move { router.handle_request(req).await }
                });

                if let Err(err) = http1::Builder::new()
                    .serve_connection(io, service)
                    .await
                {
                    eprintln!("Error serving connection: {:?}", err);
                }
            });
        }
    }
}

