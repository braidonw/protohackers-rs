use std::fmt::{Display, Formatter};
use std::{i32, str};

#[derive(Debug, PartialEq, Clone, Ord, PartialOrd, Eq)]
pub struct SessionId(pub i32);

impl Display for SessionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<&[u8]> for SessionId {
    type Error = anyhow::Error;

    fn try_from(bytes: &[u8]) -> anyhow::Result<Self> {
        dbg!(&bytes);
        let id_str = String::from_utf8_lossy(bytes);
        let id: i32 = str::parse(&id_str)?;
        Ok(SessionId(id))
    }
}

// Represents the data portion of the packet
#[derive(Debug, PartialEq, Clone)]
pub struct Data(Vec<u8>);

impl TryFrom<&[u8]> for Data {
    type Error = anyhow::Error;

    fn try_from(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(Data(bytes.to_vec()))
    }
}

impl AsRef<[u8]> for Data {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for Data {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Payload {
    Connect,
    Data { data: Data, pos: i32 },
    Ack { pos: i32 },
    Close,
}

impl Payload {
    pub fn to_kind_bytes(&self) -> &[u8] {
        match self {
            Payload::Connect => b"connect",
            Payload::Data { .. } => b"data",
            Payload::Ack { .. } => b"ack",
            Payload::Close => b"close",
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Packet {
    pub session_id: SessionId,
    pub payload: Payload,
}

impl Packet {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::new();

        bytes.push(b'/');
        bytes.extend(self.payload.to_kind_bytes());
        bytes.push(b'/');
        bytes.extend(self.session_id.to_string().as_bytes());

        match &self.payload {
            Payload::Data { data, pos } => {
                bytes.push(b'/');
                bytes.extend(pos.to_string().as_bytes());
                bytes.push(b'/');
                bytes.extend(data.as_ref());
            }
            Payload::Ack { pos } => {
                bytes.push(b'/');
                bytes.extend(pos.to_string().as_bytes());
            }
            _ => {}
        };

        bytes.push(b'/');
        bytes
    }
}

impl TryFrom<&[u8]> for Packet {
    type Error = anyhow::Error;

    fn try_from(bytes: &[u8]) -> anyhow::Result<Self> {
        dbg!(&bytes);
        let len = bytes.len();
        if bytes.is_empty() {
            return Err(anyhow::anyhow!("Empty packet"));
        }

        if bytes[0] != b'/' || bytes[len - 1] != b'/' {
            return Err(anyhow::anyhow!("Invalid packet"));
        }

        // Get the packet type
        let second_slash = 1 + bytes[1..]
            .iter()
            .position(|&x| x == b'/')
            .ok_or(anyhow::anyhow!("Invalid packet: second slash not found"))?;

        let kind = &bytes[1..second_slash];
        let kind_str = String::from_utf8_lossy(kind);
        dbg!(kind_str);

        let third_slash = 1
            + second_slash
            + bytes[second_slash + 1..]
                .iter()
                .position(|&x| x == b'/')
                .ok_or(anyhow::anyhow!("Invalid packet: third slash not found"))?;

        let session_id = SessionId::try_from(&bytes[second_slash + 1..third_slash])?;

        let payload = match &bytes[1..second_slash] {
            b"connect" => Some(Payload::Connect),
            // "/data/sessionid/pos/data/"
            b"data" => {
                let fourth_slash = 1
                    + third_slash
                    + bytes[third_slash + 1..]
                        .iter()
                        .position(|&x| x == b'/')
                        .ok_or(anyhow::anyhow!(
                            "Invalid packet: Data packet but fourth slash not found"
                        ))?;

                dbg!(fourth_slash, len);

                // There should be data between the fourth slash and the end of the message
                if fourth_slash == len - 1 {
                    return Err(anyhow::anyhow!("Invalid packet: Data packet but no data"));
                }

                let pos: i32 = str::parse(str::from_utf8(&bytes[third_slash + 1..fourth_slash])?)?;

                let data = Data::try_from(&bytes[fourth_slash + 1..len - 1])?;
                Some(Payload::Data { data, pos })
            }
            b"ack" => {
                let pos: i32 = str::parse(str::from_utf8(&bytes[third_slash + 1..len - 1])?)?;
                Some(Payload::Ack { pos })
            }
            b"close" => Some(Payload::Close),
            _ => None,
        };

        let payload = payload.ok_or(anyhow::anyhow!("Invalid packet: Unknown packet type"))?;

        Ok(Packet {
            session_id,
            payload,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parsing_packets_basic() {
        let bytes = "/data/1234567/0/hello/".as_bytes();

        let packet = Packet::try_from(bytes).unwrap();
        assert_eq!(
            packet,
            Packet {
                session_id: SessionId(1234567),
                payload: Payload::Data {
                    data: Data(b"hello".to_vec()),
                    pos: 0,
                },
            }
        );
    }

    #[test]
    fn parsing_packets_complex() {
        let raw_packets = vec![
            "/connect/1234567/",
            "/data/1234567/0/hello/",
            "/data/1234567/5/world/",
            "/ack/1234567/0/",
            "/ack/1234567/5/",
            "/close/1234567/",
        ];

        for raw_packet in raw_packets {
            let packet = Packet::try_from(raw_packet.as_bytes()).unwrap();
            assert_eq!(packet.session_id, SessionId(1234567));
        }
    }
}
