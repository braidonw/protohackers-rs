use nom::branch::alt;
use nom::bytes::complete::{escaped_transform, is_not, tag};
use nom::character::complete::char;
use nom::combinator::value;
use nom::sequence::delimited;
use nom::{character::complete::digit1, IResult};
use nom::{error, AsBytes};

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct SessionId(u32);

#[derive(Debug, PartialEq)]
pub enum Payload {
    Connect,
    Close,
    Ack { position: u32 },
    Data { data: Vec<u8>, position: u32 },
}

#[derive(Debug)]
pub struct Message {
    pub session: SessionId,
    payload: Payload,
}

impl Message {
    pub fn parse(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        let (_input, message) = parse_message(bytes).map_err(|_| {
            anyhow::anyhow!("Failed to parse message: {:?}", std::str::from_utf8(bytes))
        })?;

        Ok(message)
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session
    }

    pub fn to_packet(&self) -> Vec<u8> {
        match &self.payload {
            Payload::Connect => format!("/connect/{}/", self.session.0).as_bytes().to_vec(),
            Payload::Close => format!("/close/{}/", self.session.0).as_bytes().to_vec(),
            Payload::Ack { position } => format!("/ack/{}/{}/", self.session.0, position)
                .as_bytes()
                .to_vec(),
            Payload::Data { data, position } => {
                let data_bytes = data.iter().flat_map(|&x| {
                    if x == b'\\' {
                        vec![b'\\', b'\\']
                    } else if x == b'/' {
                        vec![b'\\', b'/']
                    } else {
                        vec![x]
                    }
                });

                format!("/data/{}/{position}/", self.session.0)
                    .into_bytes()
                    .into_iter()
                    .chain(data_bytes)
                    .chain(std::iter::once(b'/'))
                    .collect()
            }
        }
    }
}

fn parse_message(input: &[u8]) -> IResult<&[u8], Message> {
    // /data/123/1/Hello, World!/
    let (input, message_kind) =
        delimited(char::<&[u8], error::Error<_>>('/'), is_not("/"), char('/'))(input)
            .expect("Failed to parse message kind");

    // 123/1/Hello, World!/
    let (input, session_id) = parse_u32_from_digits(input)?;

    // 1/Hello, World!/
    let payload = match message_kind.as_bytes() {
        b"connect" => Payload::Connect,
        b"close" => Payload::Close,

        b"ack" => {
            let (_input, position) = delimited(char('/'), parse_u32_from_digits, char('/'))(input)?;
            Payload::Ack { position }
        }

        b"data" => {
            let (input, position) = delimited(char('/'), parse_u32_from_digits, char('/'))(input)?;

            let (_input, data) = escaped_transform(
                is_not::<&str, &[u8], error::Error<&[u8]>>(r#"\/"#),
                '\\',
                alt((
                    value(b"\\".as_slice(), tag(b"\\")),
                    value(b"/".as_slice(), tag(b"/")),
                )),
            )(input)?;

            Payload::Data {
                data: data.to_vec(),
                position,
            }
        }

        _ => {
            return Err(nom::Err::Error(error::Error::new(
                input,
                error::ErrorKind::Tag,
            )))
        }
    };

    let message = Message {
        session: SessionId(session_id),
        payload,
    };

    Ok((input, message))
}

fn parse_u32_from_digits(input: &[u8]) -> IResult<&[u8], u32> {
    let (input, digits) = digit1(input)?;
    let number_str = std::str::from_utf8(digits).expect("Failed to parse session id");
    let number = number_str
        .parse::<u32>()
        .map_err(|_| nom::Err::Error(error::Error::new(input, error::ErrorKind::Digit)))?;
    Ok((input, number))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_connect() {
        let bytes = b"/connect/123/";
        let (_input, message) = parse_message(bytes).unwrap();
        assert_eq!(message.session.0, 123);
        assert_eq!(message.payload, Payload::Connect);
        assert_eq!(message.to_packet(), bytes);
    }

    #[test]
    fn parse_close() {
        let bytes = b"/close/123/";
        let (_input, message) = parse_message(bytes).unwrap();
        assert_eq!(message.session.0, 123);
        assert_eq!(message.payload, Payload::Close);
        assert_eq!(message.to_packet(), bytes);
    }

    #[test]
    fn parse_ack() {
        let bytes = b"/ack/123/456/";
        let (_input, message) = parse_message(bytes).unwrap();
        assert_eq!(message.session.0, 123);
        assert_eq!(message.payload, Payload::Ack { position: 456 });
        assert_eq!(message.to_packet(), bytes);
    }

    #[test]
    fn parse_data() {
        let bytes = b"/data/123/456/Hello, World!/";
        let (_input, message) = parse_message(bytes).unwrap();
        assert_eq!(message.session.0, 123);
        assert_eq!(
            message.payload,
            Payload::Data {
                position: 456,
                data: b"Hello, World!".to_vec()
            }
        );

        assert_eq!(message.to_packet(), bytes);
    }
}
