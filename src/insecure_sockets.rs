mod protocol;
mod server;
use log::info;
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use crate::insecure_sockets;

pub async fn run(port: &str) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    info!("Running prime time server on {}...", &addr);

    let listener = TcpListener::bind(&addr).await?;

    loop {
        let (stream, address) = listener.accept().await?;
        info!("Accepted connection from {}", address);

        tokio::spawn(async move { session_handler(stream, address).await });
    }
}

pub async fn session_handler(mut stream: TcpStream, address: SocketAddr) -> anyhow::Result<()> {
    let (read, mut write) = stream.split();

    let (read, cipher) = {
        let mut read = BufReader::new(read);
        let mut cipher = Vec::new();

        read.read_until(0x00, &mut cipher).await?;

        info!("Received cipher: {:?}", cipher);

        (read.into_inner(), cipher)
    };

    let mut client = protocol::Client::new(&cipher)?;

    let mut line = String::new();
    let mut reader = BufReader::new(read);
    while let Ok(_num_bytes) = reader.read_line(&mut line).await {
        let message = client.decode(unsafe { line.as_bytes_mut() })?;
        info!("Received message: {:?}", message);
        let response = insecure_sockets::server::handle_message(&message)?;
        info!("Sending response: {:?}", response);
        let response_bytes = client.encode(response)?;

        write.write_all(&response_bytes).await?;
        write.write_u8(0x00).await?;
        line.clear();
        info!("Waiting for next message...");
    }

    Ok(())
}
