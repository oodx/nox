use nox::server::NoxServer;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> nox::Result<()> {
    let addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();
    let server = NoxServer::new(addr);
    server.run().await
}