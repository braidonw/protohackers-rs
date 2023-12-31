#![allow(dead_code)]

use super::protocol::Cipher;
use anyhow::Result;
use log::info;
use nom::{
    bytes::complete::{is_not, tag},
    multi::{many1, separated_list1},
    sequence::separated_pair,
    IResult,
};
use std::fmt::{Display, Formatter};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};

pub struct Session {
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
    cipher: Cipher,
}

#[derive(Debug, Eq, PartialEq)]
struct Job {
    toy: String,
    copies: usize,
}

impl Ord for Job {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.copies.cmp(&other.copies)
    }
}

impl PartialOrd for Job {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.copies.cmp(&other.copies))
    }
}

impl Job {
    fn new(toy: String, copies: usize) -> Self {
        Self { toy, copies }
    }
}

impl Display for Job {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x {}", self.copies, self.toy)
    }
}

impl Session {
    pub async fn new(stream: TcpStream) -> Result<Self> {
        let (read_half, write_half) = stream.into_split();
        let mut reader = BufReader::new(read_half);
        let mut buffer = Vec::new();
        let _bytes_read = reader.read_until(0x00, &mut buffer).await?;
        info!("Read buffer: {:?}", buffer);

        let cipher = Cipher::new(&buffer)?;
        info!("New cipher: {:?}", cipher);

        Ok(Self {
            reader,
            writer: write_half,
            cipher,
        })
    }

    pub async fn read_line(&mut self) -> Result<String> {
        let mut line = String::new();
        info!("Reading line with cipher: {:?}", self.cipher);
        loop {
            let byte = self.reader.read_u8().await?;
            let decoded_byte = self.cipher.decode_byte(byte);

            if decoded_byte == b'\n' {
                break;
            } else {
                line.push(decoded_byte as char);
            }
        }
        info!("Received line: {}", line);
        Ok(line)
    }

    pub async fn write_line(&mut self, mut line: String) -> Result<()> {
        line.push('\n');
        let encoded_bytes = self.cipher.encode(line.as_bytes())?;

        self.writer.write_all(&encoded_bytes).await?;
        self.writer.flush().await?;
        Ok(())
    }
}

pub fn handle_message(message: &str) -> Result<String> {
    let mut jobs = parse_message(message)?;
    jobs.sort();
    let response: String = jobs.iter().rev().take(1).map(|j| j.to_string()).collect();
    Ok(response)
}

fn parse_message(message: &str) -> Result<Vec<Job>> {
    let (_, jobs) = separated_list1(tag(","), parse_job)(message)
        .map_err(|_| anyhow::anyhow!("Failed to parse jobs from message"))?;

    Ok(jobs)
}

fn parse_job(s: &str) -> IResult<&str, Job> {
    let (s, (copies, toy)) = separated_pair(parse_copies, tag("x "), parse_toy)(s)?;
    Ok((s, Job::new(toy, copies)))
}

fn parse_copies(s: &str) -> IResult<&str, usize> {
    let (s, copies) = nom::character::complete::digit1(s)?;
    let copies = copies.parse::<usize>().unwrap();
    Ok((s, copies))
}

fn parse_toy(s: &str) -> IResult<&str, String> {
    let (s, toy) = many1(is_not(","))(s)?;
    let toy = toy.join("");
    Ok((s, toy.to_string()))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_message() {
        let message = "10x toy car,15x dog on a string,4x inflatable motorcycle";
        let jobs = parse_message(message).unwrap();

        dbg!(&jobs);

        assert!(jobs.len() == 3);
        assert!(jobs[0].copies == 10);
        assert!(jobs[0].toy == "toy car");
        assert!(jobs[1].copies == 15);
        assert!(jobs[1].toy == "dog on a string");
        assert!(jobs[2].copies == 4);
        assert!(jobs[2].toy == "inflatable motorcycle");
    }

    #[test]
    fn test_parse_message2() {
        let message = "10x toy car,15x dog on a string,4x inflatable motorcycle,95x test toy";
        let jobs = parse_message(message).unwrap();

        dbg!(&jobs);

        assert!(jobs.len() == 4);
        assert!(jobs[0].copies == 10);
        assert!(jobs[0].toy == "toy car");
        assert!(jobs[1].copies == 15);
        assert!(jobs[1].toy == "dog on a string");
        assert!(jobs[2].copies == 4);
        assert!(jobs[2].toy == "inflatable motorcycle");
        assert!(jobs[3].copies == 95);
        assert!(jobs[3].toy == "test toy");
    }

    #[test]
    fn test_handle_message() {
        let message = "10x toy car,15x dog on a string,4x inflatable motorcycle";
        let response = handle_message(message).unwrap();
        assert_eq!(response, "15x dog on a string");
    }
}
