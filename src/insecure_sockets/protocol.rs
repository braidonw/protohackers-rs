#![allow(dead_code)]
use anyhow::Result;
use nom::{
    branch::alt,
    bytes::{self, complete::tag},
    multi, IResult,
};

const REVERSE_BITS: &[u8] = &[0x01]; //1
const XOR: &[u8] = &[0x02]; //2
const XOR_POSITION: &[u8] = &[0x03]; //3
const ADD: &[u8] = &[0x04]; //4
const ADD_POSITION: &[u8] = &[0x05]; //5
const CIPHER_END: &[u8] = &[0x00]; //6

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
pub struct Cipher {
    pub cipher: Vec<Operation>,
    pub incoming_position: usize,
    pub outgoing_position: usize,
}

impl Cipher {
    pub fn new(bytes: &[u8]) -> Result<Self> {
        let operations = parse_cipher_spec(bytes)?;
        Ok(Self {
            cipher: operations,
            incoming_position: 0,
            outgoing_position: 0,
        })
    }

    pub fn with_position(&mut self, incoming_position: usize, outgoing_position: usize) {
        self.incoming_position = incoming_position;
        self.outgoing_position = outgoing_position;
    }

    pub fn decode_byte(&mut self, input: u8) -> u8 {
        let mut byte = input;
        for operation in self.cipher.iter().rev() {
            match operation {
                Operation::ReverseBits => byte = byte.reverse_bits(),
                Operation::Xor { n } => byte ^= n,
                Operation::XorPosition => byte ^= self.incoming_position as u8,
                Operation::Add { n } => byte = byte.wrapping_sub(*n),
                Operation::AddPosition => byte = byte.wrapping_sub(self.incoming_position as u8),
                Operation::CipherEnd => {}
            }
        }
        self.incoming_position += 1;
        byte
    }

    pub fn encode_byte(&mut self, byte: u8) -> u8 {
        let mut byte = byte;
        for operation in self.cipher.iter() {
            match operation {
                Operation::ReverseBits => byte = byte.reverse_bits(),
                Operation::Xor { n } => byte ^= n,
                Operation::XorPosition => byte ^= self.outgoing_position as u8,
                Operation::Add { n } => byte = byte.wrapping_add(*n),
                Operation::AddPosition => byte = byte.wrapping_add(self.outgoing_position as u8),
                Operation::CipherEnd => {}
            }
        }
        self.outgoing_position += 1;
        byte
    }

    pub fn encode(&mut self, bytes: &[u8]) -> Result<Vec<u8>> {
        let out = bytes
            .iter()
            .map(|b| self.encode_byte(*b))
            .collect::<Vec<u8>>();

        if out == bytes {
            return Err(anyhow::anyhow!("Failed to encode bytes"));
        }

        Ok(out)
    }

    // For testing
    pub fn decode(&mut self, bytes: &[u8]) -> Result<Vec<u8>> {
        let out = bytes
            .iter()
            .map(|b| self.decode_byte(*b))
            .collect::<Vec<u8>>();

        if out == bytes {
            return Err(anyhow::anyhow!("Failed to decode bytes"));
        }

        Ok(out)
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
    pub fn encode_simple_message() {
        let mut client = Cipher::new(&[0x02, 0x01, 0x01, 0x00]).unwrap();
        let message = "hello";

        let encoded = message
            .as_bytes()
            .iter()
            .map(|byte| client.encode_byte(*byte))
            .collect::<Vec<u8>>();

        assert_eq!(encoded, vec![0x96, 0x26, 0xb6, 0xb6, 0x76]);
    }

    #[test]
    pub fn encode_medium_message() {
        let mut client = Cipher::new(&[0x05, 0x05, 0x00]).unwrap();
        let message = "hello";

        let encoded = message
            .as_bytes()
            .iter()
            .map(|byte| client.encode_byte(*byte))
            .collect::<Vec<u8>>();

        assert_eq!(encoded, vec![0x68, 0x67, 0x70, 0x72, 0x77]);
    }

    #[test]
    pub fn example_session() {
        let messages = [
            "55x life-size bear on a string,87x giant plastic mobile phone with carry case,40x pocket-size metal quadcopter simulator,34x soft rubber inflatable quadcopter with carry case,100x pocket-size cuddly pony toy\n",
            "21x small inflatable cow simulator,52x pocket-size plastic inflatable motorcycle toy,85x giant duck-billed platypus with remote-controlled train simulator,23x soft rubber pony,76x small soft rubber goat\n",
        ];

        let responses = [
            "100x pocket-size cuddly pony toy\n",
            "85x giant duck-billed platypus with remote-controlled train simulator\n",
        ];

        let mut client = Cipher::new(&[0x02, 0x01, 0x00]).unwrap();
        let mut server = Cipher::new(&[0x02, 0x01, 0x00]).unwrap();

        for i in 0..messages.len() {
            let mut encoded = client.encode(messages[i].as_bytes()).unwrap();
            let mut decoded = server.encode(&encoded).unwrap();
            assert_eq!(decoded, messages[i].as_bytes());

            encoded = server.encode(responses[i].as_bytes()).unwrap();
            decoded = client.encode(&encoded).unwrap();
            assert_eq!(decoded, responses[i].as_bytes());
        }
    }

    #[test]
    pub fn complex_cipher() -> anyhow::Result<()> {
        let messages = [
            "55x life-size bear on a string,87x giant plastic mobile phone with carry case,40x pocket-size metal quadcopter simulator,34x soft rubber inflatable quadcopter with carry case,100x pocket-size cuddly pony toy\n",
            "21x small inflatable cow simulator,52x pocket-size plastic inflatable motorcycle toy,85x giant duck-billed platypus with remote-controlled train simulator,23x soft rubber pony,76x small soft rubber goat\n",
        ];

        let responses = [
            "100x pocket-size cuddly pony toy\n",
            "85x giant duck-billed platypus with remote-controlled train simulator\n",
        ];

        let cipher_bytes = [2, 110, 4, 215, 3, 1, 4, 5, 1, 0];

        let mut client_cipher = Cipher::new(&cipher_bytes)?;
        let mut server_cipher = Cipher::new(&cipher_bytes)?;

        assert_eq!(
            client_cipher.cipher,
            vec![
                Operation::Xor { n: 110 },
                Operation::Add { n: 215 },
                Operation::XorPosition,
                Operation::ReverseBits,
                Operation::Add { n: 5 },
                Operation::ReverseBits,
                Operation::CipherEnd,
            ],
        );

        for i in 0..messages.len() {
            let mut encoded = client_cipher.encode(messages[i].as_bytes()).unwrap();
            let mut decoded = server_cipher.decode(&encoded).unwrap();
            dbg!(&server_cipher, &client_cipher);
            assert_eq!(decoded, messages[i].as_bytes());

            encoded = server_cipher.encode(responses[i].as_bytes()).unwrap();
            decoded = client_cipher.decode(&encoded).unwrap();
            dbg!(&server_cipher, &client_cipher);
            assert_eq!(decoded, responses[i].as_bytes());
        }

        Ok(())
    }

    #[test]
    fn test_applying_operations() {
        let message = b"hello";

        for operation in &[
            Operation::ReverseBits,
            Operation::Xor { n: 0x01 },
            Operation::XorPosition,
            Operation::Add { n: 0x01 },
            Operation::Add { n: 0x20 },
            Operation::Add { n: 0xFF },
            Operation::Add { n: 0xf0 },
            Operation::AddPosition,
            Operation::CipherEnd,
        ] {
            let operation = operation.clone();
            dbg!(&operation);
            let mut cipher = Cipher {
                cipher: vec![operation],
                incoming_position: 0,
                outgoing_position: 0,
            };

            let encoded = message
                .iter()
                .map(|byte| cipher.encode_byte(*byte))
                .collect::<Vec<u8>>();

            let decoded = encoded
                .iter()
                .map(|byte| cipher.decode_byte(*byte))
                .collect::<Vec<u8>>();

            assert_eq!(decoded, message);
        }
    }
}
