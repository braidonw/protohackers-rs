use log::info;
use std::collections::BTreeMap;
use std::ops::Bound::Included;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

#[derive(Debug)]
enum Message {
    Insert { timestamp: u64, price: i64 },
    Query { from: u64, to: u64 },
    Unknown,
}

impl TryFrom<[u8; 9]> for Message {
    type Error = anyhow::Error;

    fn try_from(bytes: [u8; 9]) -> anyhow::Result<Self> {
        let message = match bytes[0] as char {
            'I' => {
                let timestamp = u64::from_be_bytes(bytes[1..5].try_into()?);
                let price = i64::from_be_bytes(bytes[5..9].try_into()?);
                Message::Insert { timestamp, price }
            }

            'Q' => {
                let from = u64::from_be_bytes(bytes[1..5].try_into()?);
                let to = u64::from_be_bytes(bytes[5..9].try_into()?);
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
    let mut db: BTreeMap<u64, i64> = BTreeMap::new();

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
                    writer.write_i64(0).await?;
                    continue;
                }

                // Otherwise, return the average of all prices between the min and max time

                let (count, price) = db
                    .range((Included(&from), Included(&to)))
                    .fold((0, 0), |(count, sum), (_timestamp, price)| {
                        (count + 1, sum + price)
                    });

                let average = if count > 0 { price / count } else { 0 };

                info!("query result: mean {}", average);
                writer.write_i64(average).await?;
            }

            Message::Unknown => {
                writer.write_all(b"Unknown\n").await?;
            }
        }
    }

    Ok(())
}
