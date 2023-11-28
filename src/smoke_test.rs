use tokio::{
    io::copy,
    net::{TcpListener, TcpStream},
};

pub async fn run(port: &str) -> anyhow::Result<()> {
    println!("Running smoke test...");
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _address) = listener.accept().await?;
        tokio::spawn(async move { handle_stream(stream).await });
    }
}

async fn handle_stream(mut stream: TcpStream) -> anyhow::Result<()> {
    let (mut reader, mut writer) = stream.split();
    println!("Copying data...");
    copy(&mut reader, &mut writer).await?;
    Ok(())
}
