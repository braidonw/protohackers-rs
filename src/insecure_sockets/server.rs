#![allow(dead_code)]

use super::protocol::Client;
use anyhow::Result;
use log::info;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alpha1, space1},
    multi::{many1, separated_list1},
    sequence::separated_pair,
    IResult,
};
use std::fmt::{Display, Formatter};
use std::net::SocketAddr;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};

pub struct Server {
    client: Option<Client>,
    address: SocketAddr,
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

impl Server {
    pub fn new(address: SocketAddr) -> Result<Self> {
        Ok(Self {
            client: None,
            address,
        })
    }

    fn handle_request(&mut self, bytes: &mut [u8]) -> Result<Vec<u8>> {
        let message = self.client.as_mut().expect("No client").decode(bytes)?;
        let response = handle_message(&message)?;
        let response_bytes = self.client.as_mut().expect("No client").encode(response)?;
        Ok(response_bytes)
    }

    pub async fn run(mut self, stream: TcpStream) -> Result<()> {
        info!("Running insecure sockets server for {}...", &self.address);
        let mut reader = BufReader::new(stream);
        let mut line = String::new();

        reader.read_line(&mut line).await?;
        info!("Received cipher: {}", line);
        let client = Client::new(line.as_bytes())?;
        info!("Initialized client with cipher: {:?}", client.cipher);
        self.client = Some(client);
        line.clear();

        while let Ok(num_bytes) = reader.read_line(&mut line).await {
            if num_bytes == 0 {
                break;
            }

            let response = self.handle_request(unsafe { line.as_bytes_mut() })?;

            reader.write_all(&response).await?;
            reader.write_u8(10).await?;
            line.clear();
        }

        Ok(())
    }
}

fn handle_message(message: &str) -> Result<String> {
    let mut jobs = parse_message(message)?;
    jobs.sort();
    let response: String = jobs.iter().take(1).map(|j| j.to_string()).collect();
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
    let (s, toy) = many1(alt((alpha1, space1)))(s)?;
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
}
