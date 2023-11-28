use primal::is_prime;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

pub async fn run(port: &str) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    println!("Running prime time server on {}...", &addr);

    let listener = TcpListener::bind(&addr).await?;

    loop {
        let (stream, address) = listener.accept().await?;
        tokio::spawn(async move { prime_handler(stream, address).await });
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Request {
    method: String,
    number: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Response {
    method: String,
    prime: bool,
}

async fn prime_handler(stream: TcpStream, address: std::net::SocketAddr) -> anyhow::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    while let Ok(num_bytes) = reader.read_line(&mut line).await {
        if num_bytes == 0 {
            break;
        }

        let request: Request = serde_json::from_str(line.trim())?;
        println!("Received {:?} from {}", request, address);

        let response = match request.method.as_str() {
            "isPrime" => handle_correct_request(request)?,

            _ => "Invalid request".to_string(),
        };

        reader.write_all(response.as_bytes()).await?;
        reader.write_u8(10).await?;
        line.clear();
    }

    Ok(())
}

fn handle_correct_request(request: Request) -> anyhow::Result<String> {
    let request_num_is_prime = is_prime(request.number as u64);
    let response = Response {
        method: request.method,
        is_prime: request_num_is_prime,
    };

    println!("Sending {:?}", &response);
    serde_json::to_string(&response).map_err(|e| e.into())
}
