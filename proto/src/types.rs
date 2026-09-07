use heapless::String;
use minicbor::{Decode, Encode};

use crate::consts::{MAX_CHANNEL_NAME, MAX_MSG_SIZE, MAX_NICK_NAME};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationError {
    Empty,
    TooLong,
    BadChar,
}

fn check_channel(s: &str) -> Result<(), ValidationError> {
    if s.is_empty() {
        return Err(ValidationError::Empty);
    }
    if s.len() > MAX_CHANNEL_NAME {
        return Err(ValidationError::TooLong);
    }
    if !s
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(ValidationError::BadChar);
    }
    Ok(())
}

fn check_nick(s: &str) -> Result<(), ValidationError> {
    if s.trim().is_empty() {
        return Err(ValidationError::Empty);
    }
    if s.len() > MAX_NICK_NAME {
        return Err(ValidationError::TooLong);
    }
    if s.chars().any(char::is_control) {
        return Err(ValidationError::BadChar);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel(String<MAX_CHANNEL_NAME>);

impl Channel {
    pub fn new(s: &str) -> Result<Self, ValidationError> {
        check_channel(s)?;
        let mut out = String::new();
        out.push_str(s).map_err(|_| ValidationError::TooLong)?;
        Ok(Self(out))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nick(String<MAX_NICK_NAME>);

impl Nick {
    pub fn new(s: &str) -> Result<Self, ValidationError> {
        check_nick(s)?;
        let mut out = String::new();
        out.push_str(s).map_err(|_| ValidationError::TooLong)?;
        Ok(Self(out))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Encode, Decode)]
pub struct Msg<'a> {
    #[b(0)]
    pub nick: &'a str,
    #[b(1)]
    pub text: &'a str,
    #[n(2)]
    pub ts: Option<u64>,
}

impl Msg<'_> {
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_nick(self.nick)?;
        if self.text.is_empty() {
            return Err(ValidationError::Empty);
        }
        if self.text.len() > MAX_MSG_SIZE {
            return Err(ValidationError::TooLong);
        }
        if self.text.chars().any(char::is_control) {
            return Err(ValidationError::BadChar);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Encode, Decode)]
pub enum Body<'a> {
    #[n(0)]
    Plain(#[b(0)] Msg<'a>),
    #[n(1)]
    Sealed(#[cbor(b(0), with = "minicbor::bytes")] &'a [u8]),
}

#[derive(Debug, Clone, Encode, Decode)]
pub struct Envelope<'a> {
    #[cbor(n(0), with = "minicbor::bytes")]
    pub sender: [u8; 4], // id is thing
    #[n(1)]
    pub seq: u64,
    #[b(2)]
    pub body: Body<'a>,
}
