mod protocol;
mod session;
use anyhow::Result;
use log::info;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

pub async fn run(port: &str) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    info!("Running insecure sockets server on {}...", &addr);
    let listener = TcpListener::bind(&addr).await?;

    loop {
        let (stream, address) = listener.accept().await?;

        tokio::spawn(async move {
            handle_connection(stream, address)
                .await
                .expect("Connection handler error")
        });
    }
}

async fn handle_connection(stream: TcpStream, address: SocketAddr) -> Result<()> {
    info!("Accepted connection from {}", address);
    let mut session = session::Session::new(stream).await?;

    loop {
        let line = session.read_line().await?;
        let response = session::handle_message(&line)?;
        info!("Sending response to address: {} -> {}", response, address);
        session.write_line(response).await?;
    }
}
