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
use crate::Result;

pub struct NoxServer {
    addr: SocketAddr,
}

impl NoxServer {
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr }
    }

    pub async fn run(self) -> Result<()> {
        let listener = TcpListener::bind(self.addr).await?;
        println!("NOX Server running on http://{}", self.addr);

        loop {
            let (stream, _) = listener.accept().await?;
            let io = TokioIo::new(stream);

            tokio::task::spawn(async move {
                if let Err(err) = http1::Builder::new()
                    .serve_connection(io, service_fn(handle_request))
                    .await
                {
                    eprintln!("Error serving connection: {:?}", err);
                }
            });
        }
    }
}

async fn handle_request(req: Request<Incoming>) -> std::result::Result<Response<Full<Bytes>>, Infallible> {
    let response = match req.uri().path() {
        "/" => Response::builder()
            .status(StatusCode::OK)
            .body(Full::new(Bytes::from("NOX Server - MVP")))
            .unwrap(),
        "/health" => Response::builder()
            .status(StatusCode::OK)
            .body(Full::new(Bytes::from("OK")))
            .unwrap(),
        _ => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from("Not Found")))
            .unwrap(),
    };
    
    Ok(response)
}