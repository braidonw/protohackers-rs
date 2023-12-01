mod protocol;
mod server;
use log::info;
use std::cell::RefCell;
use std::net::SocketAddr;
use std::rc::Rc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::StreamExt;
use tokio_util::bytes::Bytes;
use tokio_util::io::{ReaderStream, StreamReader};

use crate::insecure_sockets;

use self::protocol::Client;

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

pub async fn connection_handler(mut stream: TcpStream, address: SocketAddr) -> anyhow::Result<()> {
    info!("Handling connection from {}", address);
    let (read_half, mut write_half) = stream.split();

    // Initalize the client
    let (read_half, cipher) = {
        let mut reader = BufReader::new(read_half);
        let mut cipher = Vec::new();
        reader.read_until(0x00, &mut cipher).await?;

        info!("Received cipher: {:?}", cipher);

        (reader.into_inner(), cipher)
    };

    let client = Rc::new(RefCell::new(Client::new(&cipher)?));
    info!(
        "Initialized client with cipher: {:?}",
        client.borrow().cipher
    );

    // Read messages from the client
    // Turn the read half of the stream into a tokio::ReaderStream to read byte by byte
    let byte_stream = ReaderStream::new(read_half);

    // Decode each byte
    let decoded_byte_stream = byte_stream.map(|chunk| {
        chunk.map(|bytes| {
            bytes
                .iter()
                .map(|b| client.borrow_mut().decode_byte(*b))
                .collect::<Bytes>()
        })
    });

    // Create StreamReader to read each decoded line
    let mut reader = StreamReader::new(decoded_byte_stream);

    let mut message = String::new();
    while let Ok(_num_bytes) = reader.read_line(&mut message).await {
        info!("Received message: {:?}", message);

        let response = insecure_sockets::server::handle_message(&message)?;
        info!("Sending response: {:?}", response);

        let response_bytes = client.borrow_mut().encode(response)?;

        write_half.write_all(&response_bytes).await?;
        write_half.write_u8(0x00).await?;

        message.clear();
    }

    Ok(())
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
    info!("Initialized client with cipher: {:?}", client.cipher);

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
