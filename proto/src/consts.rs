//! Constants for the packet protocol for thingy
//! Version 1 ig?

pub const COAP_PORT: u16 = 5683;
pub const KEEPALIVE_S: u32 = 25;
pub const MAX_CHANNEL_NAME: usize = 24;
pub const MAX_NICK_NAME: usize = 24;
pub const MAX_MSG_SIZE: usize = 512;
pub const MAX_PACKET_SIZE: usize = 768; // lowk arbitrary but why not
