mod protocol;
mod server;
use log::info;
use tokio::net::TcpListener;

use crate::insecure_sockets;

pub async fn run(port: &str) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    info!("Running prime time server on {}...", &addr);

    let listener = TcpListener::bind(&addr).await?;

    loop {
        let (stream, address) = listener.accept().await?;
        let server = insecure_sockets::server::Server::new(address)?;

        tokio::spawn(async move { server.run(stream).await });
    }
}
