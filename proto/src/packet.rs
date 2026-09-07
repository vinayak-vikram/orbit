use heapless::String;
use minicbor::encode::write::Cursor;

use crate::consts::{MAX_CHANNEL_NAME, MAX_PACKET_SIZE};
use crate::types::{Body, Channel, Envelope, ValidationError};

pub const PING_PATH: &str = "/ping";

const CHANNEL_PREFIX: &str = "channels/";
const CHANNEL_PATH_LEN: usize = CHANNEL_PREFIX.len() + MAX_CHANNEL_NAME + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtoError {
    Malformed,
    TooLarge,
    BufferTooSmall,
    BadPath,
    Validation(ValidationError),
}

impl From<ValidationError> for ProtoError {
    fn from(e: ValidationError) -> Self {
        Self::Validation(e)
    }
}

pub fn encode_into(env: &Envelope, buf: &mut [u8]) -> Result<usize, ProtoError> {
    if let Body::Plain(msg) = &env.body {
        msg.validate()?;
    }
    // clamp
    let n = buf.len().min(MAX_PACKET_SIZE);
    let mut cur = Cursor::new(&mut buf[..n]);
    minicbor::encode(env, &mut cur).map_err(|_| ProtoError::BufferTooSmall)?;
    Ok(cur.position())
}

pub fn decode(bytes: &[u8]) -> Result<Envelope<'_>, ProtoError> {
    if bytes.len() > MAX_PACKET_SIZE {
        return Err(ProtoError::TooLarge);
    }
    let env: Envelope = minicbor::decode(bytes).map_err(|_| ProtoError::Malformed)?;
    if let Body::Plain(msg) = &env.body {
        msg.validate()?;
    }
    Ok(env)
}

pub fn channel_path(ch: &Channel) -> String<CHANNEL_PATH_LEN> {
    let mut p = String::new();
    p.push('/');
    p.push_str(CHANNEL_PREFIX);
    p.push_str(ch.as_str());
    p
}

// coap-rs gives us ledaing slash on client
// but none on server
// so uh
pub fn channel_from_path(path: &str) -> Result<Channel, ProtoError> {
    let name = path
        .trim_start_matches('/')
        .strip_prefix(CHANNEL_PREFIX)
        .ok_or(ProtoError::BadPath)?;
    Ok(Channel::new(name)?)
}
