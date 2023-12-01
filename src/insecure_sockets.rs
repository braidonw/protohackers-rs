mod protocol;
mod session;
use anyhow::Result;
use log::info;
use tokio::net::{TcpListener, TcpStream};

pub async fn run(port: &str) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    info!("Running prime time server on {}...", &addr);
    let listener = TcpListener::bind(&addr).await?;

    loop {
        let (stream, address) = listener.accept().await?;
        info!("Accepted connection from {}", address);

        tokio::spawn(async move { handle_connection(stream).await });
    }
}

async fn handle_connection(stream: TcpStream) -> Result<()> {
    info!("Handling connection...");
    let mut session = session::Session::new(stream).await?;

    loop {
        let line = session.read_line().await?;
        let response = session::handle_message(&line)?;
        session.write_line(&response).await?;
    }
}
