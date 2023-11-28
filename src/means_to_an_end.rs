use log::info;
use std::collections::BTreeMap;
use std::ops::Bound::Included;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

#[derive(Debug)]
enum Message {
    Insert { timestamp: u32, price: i32 },
    Query { from: u32, to: u32 },
    Unknown,
}

impl TryFrom<[u8; 9]> for Message {
    type Error = anyhow::Error;

    fn try_from(bytes: [u8; 9]) -> anyhow::Result<Self> {
        let message = match bytes[0] as char {
            'I' => {
                let timestamp = u32::from_be_bytes(bytes[1..5].try_into()?);
                let price = i32::from_be_bytes(bytes[5..9].try_into()?);
                Message::Insert { timestamp, price }
            }

            'Q' => {
                let from = u32::from_be_bytes(bytes[1..5].try_into()?);
                let to = u32::from_be_bytes(bytes[5..9].try_into()?);
                Message::Query { from, to }
            }

            _ => Message::Unknown,
        };

        Ok(message)
    }
}

pub async fn run(port: &str) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    info!("Running means to an end server on {}...", &addr);
    let listener = TcpListener::bind(&addr).await?;

    loop {
        let (stream, address) = listener.accept().await?;
        tokio::spawn(async move { handler(stream, address).await });
    }
}

async fn handler(mut stream: TcpStream, address: std::net::SocketAddr) -> anyhow::Result<()> {
    // Init DB
    let mut db: BTreeMap<u32, i32> = BTreeMap::new();

    let (read_half, mut writer) = stream.split();
    let mut reader = BufReader::new(read_half);

    let mut bytes = [0u8; 9];

    while let Ok(_num_bytes) = reader.read_exact(&mut bytes).await {
        let message = Message::try_from(bytes)?;

        match message {
            Message::Insert { timestamp, price } => {
                info!("Received insert message {:?} from {}", message, address);
                db.insert(timestamp, price);
            }

            Message::Query { from, to } => {
                info!("Received query message {:?} from {}", message, address);

                // If the min time is greater than the max time, return an error
                if from > to {
                    writer.write_i32(0).await?;
                    continue;
                }

                // Otherwise, return the average of all prices between the min and max time
                let mut count = 0;
                let mut total_price = 0;
                for (_time, price) in db.range((Included(&from), Included(&to))) {
                    count += 1;
                    total_price += price;
                }

                let mean = if count > 0 { total_price / count } else { 0 };

                info!(
                    "query result: sum: {} / count: {} = mean {}",
                    total_price, count, mean
                );
                writer.write_i32(mean).await?;
            }

            Message::Unknown => {
                writer.write_all(b"Unknown\n").await?;
            }
        }
    }

    Ok(())
}
