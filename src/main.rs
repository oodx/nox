use nox::server::NoxServer;
use std::net::SocketAddr;

#[cfg(feature = "config")]
use nox::config::NoxConfig;

#[cfg(feature = "config")]
use clap::{Arg, Command};

#[tokio::main]
async fn main() -> nox::Result<()> {
    #[cfg(feature = "config")]
    {
        let matches = Command::new("nox")
            .version("0.1.0")
            .about("NOX Mock Server")
            .arg(
                Arg::new("config")
                    .short('c')
                    .long("config")
                    .value_name("FILE")
                    .help("Configuration file path")
                    .required(false),
            )
            .get_matches();

        if let Some(config_path) = matches.get_one::<String>("config") {
            println!("Loading config from: {}", config_path);
            let config = NoxConfig::load_from_file(config_path)?;
            let server = NoxServer::from_config(&config);
            server.run().await
        } else {
            println!("No config file specified, using default settings");
            let config = NoxConfig::default();
            let server = NoxServer::from_config(&config);
            server.run().await
        }
    }

    #[cfg(not(feature = "config"))]
    {
        let addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();
        let server = NoxServer::new(addr);
        server.run().await
    }
}