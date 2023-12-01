#![allow(dead_code)]
use anyhow::Result;
use nom::{
    branch::alt,
    bytes::{self, complete::tag},
    multi, IResult,
};

const REVERSE_BITS: &[u8] = &[0x01];
const XOR: &[u8] = &[0x02];
const XOR_POSITION: &[u8] = &[0x03];
const ADD: &[u8] = &[0x04];
const ADD_POSITION: &[u8] = &[0x05];
const CIPHER_END: &[u8] = &[0x00];

#[derive(Debug, PartialEq, Clone)]
pub enum Operation {
    ReverseBits,
    Xor { n: u8 },
    XorPosition,
    Add { n: u8 },
    AddPosition,
    CipherEnd,
}

#[derive(Debug)]
pub struct Client {
    pub cipher: Vec<Operation>,
    pub incoming_position: usize,
    pub outgoing_position: usize,
}

impl Client {
    pub fn new(bytes: &[u8]) -> Result<Self> {
        let operations = parse_cipher_spec(bytes)?;
        Ok(Self {
            cipher: operations,
            incoming_position: 0,
            outgoing_position: 0,
        })
    }

    pub fn decode(&mut self, bytes: &mut [u8]) -> Result<String> {
        for byte in bytes.iter_mut() {
            *byte = self.decode_byte(*byte);
            self.incoming_position += 1;
        }
        Ok(String::from_utf8_lossy(bytes).to_string())
    }

    pub fn encode(&mut self, s: String) -> Result<Vec<u8>> {
        let mut bytes = s.into_bytes();
        for byte in bytes.iter_mut() {
            *byte = self.encode_byte(*byte);
            self.outgoing_position += 1;
        }
        Ok(bytes)
    }

    pub fn decode_byte(&mut self, byte: u8) -> u8 {
        let mut byte = byte;
        for operation in self.cipher.iter().rev() {
            match operation {
                Operation::ReverseBits => {
                    byte = byte.reverse_bits();
                }
                Operation::Xor { n } => {
                    byte ^= n;
                }
                Operation::XorPosition => {
                    byte ^= self.incoming_position as u8;
                }
                Operation::Add { n } => {
                    byte = byte.wrapping_add(*n);
                }
                Operation::AddPosition => {
                    byte = byte.wrapping_add(self.incoming_position as u8);
                }
                Operation::CipherEnd => {
                    break;
                }
            }
        }
        byte
    }

    pub fn encode_byte(&mut self, byte: u8) -> u8 {
        let mut byte = byte;
        for operation in self.cipher.iter() {
            match operation {
                Operation::ReverseBits => {
                    byte = byte.reverse_bits();
                }
                Operation::Xor { n } => {
                    byte ^= n;
                }
                Operation::XorPosition => {
                    byte ^= self.outgoing_position as u8;
                }
                Operation::Add { n } => {
                    byte = byte.wrapping_add(*n);
                }
                Operation::AddPosition => {
                    byte = byte.wrapping_add(self.outgoing_position as u8);
                }
                Operation::CipherEnd => {
                    break;
                }
            }
        }
        byte
    }
}

fn parse_cipher_spec(bytes: &[u8]) -> Result<Vec<Operation>> {
    let (_input, operations) = multi::many1(parse_operation)(bytes)
        .map_err(|_| anyhow::anyhow!("Failed to parse cipher spec"))?;
    Ok(operations)
}

fn parse_operation(bytes: &[u8]) -> IResult<&[u8], Operation> {
    alt((
        parse_reverse_bits,
        parse_xor,
        parse_xor_position,
        parse_add,
        parse_add_position,
        parse_cipher_end,
    ))(bytes)
}

fn parse_reverse_bits(bytes: &[u8]) -> IResult<&[u8], Operation> {
    let (input, _) = tag(REVERSE_BITS)(bytes)?;
    Ok((input, Operation::ReverseBits))
}

fn parse_xor(bytes: &[u8]) -> IResult<&[u8], Operation> {
    let (input, _) = tag(XOR)(bytes)?;
    let (input, n) = bytes::complete::take(1u8)(input)?;
    Ok((input, Operation::Xor { n: n[0] }))
}

fn parse_xor_position(bytes: &[u8]) -> IResult<&[u8], Operation> {
    let (input, _) = tag(XOR_POSITION)(bytes)?;
    Ok((input, Operation::XorPosition))
}

fn parse_add(bytes: &[u8]) -> IResult<&[u8], Operation> {
    let (input, _) = tag(ADD)(bytes)?;
    let (input, n) = bytes::complete::take(1u8)(input)?;
    Ok((input, Operation::Add { n: n[0] }))
}

fn parse_add_position(bytes: &[u8]) -> IResult<&[u8], Operation> {
    let (input, _) = tag(ADD_POSITION)(bytes)?;
    Ok((input, Operation::AddPosition))
}

fn parse_cipher_end(bytes: &[u8]) -> IResult<&[u8], Operation> {
    let (input, _) = tag(CIPHER_END)(bytes)?;
    Ok((input, Operation::CipherEnd))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn parse_cipher_spec_simple() {
        let bytes = vec![0x02, 0x01, 0x01, 0x00];
        let operations = parse_cipher_spec(&bytes).unwrap();
        assert_eq!(
            operations,
            vec![
                Operation::Xor { n: 0x01 },
                Operation::ReverseBits,
                Operation::CipherEnd
            ]
        );
    }

    #[test]
    pub fn parse_cipher_spec_example() {
        let bytes = vec![0x02, 0x7b, 0x05, 0x01, 0x00];
        let operations = parse_cipher_spec(&bytes).unwrap();
        assert_eq!(
            operations,
            vec![
                Operation::Xor { n: 123 },
                Operation::AddPosition,
                Operation::ReverseBits,
                Operation::CipherEnd
            ]
        );
    }

    #[test]
    pub fn decode_simple_message() {
        let mut client = Client::new(&[0x02, 0x01, 0x01, 0x00]).unwrap();
        let message_bytes = vec![0x68, 0x65, 0x6c, 0x6c, 0x6f];

        let decoded = client.decode(&mut message_bytes.clone()).unwrap();
        assert_eq!(decoded, "hello");
    }

    #[test]
    pub fn encode_simple_message() {
        let mut client = Client::new(&[0x02, 0x01, 0x01, 0x00]).unwrap();
        let message = "hello";

        let encoded = client.encode(message.to_string()).unwrap();
        assert_eq!(encoded, vec![0x96, 0x26, 0xb6, 0xb6, 0x76]);
    }

    pub fn encode_medium_message() {
        let mut client = Client::new(&[0x05, 0x05, 0x00]).unwrap();
        let message = "hello";

        let encoded = client.encode(message.to_string()).unwrap();
        assert_eq!(encoded, vec![0x68, 0x67, 0x70, 0x72, 0x77]);
    }
}
