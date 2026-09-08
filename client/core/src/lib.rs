use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use coap::UdpCoAPClient;
use coap::client::ObserveMessage;
use coap::request::RequestBuilder;
use coap_lite::RequestType;
use orbit_proto::consts::{KEEPALIVE_S, MAX_PACKET_SIZE};
use orbit_proto::packet::{PING_PATH, ProtoError, channel_path, decode, encode_into};
use orbit_proto::types::{Body, Channel, Envelope, Msg, Nick, ValidationError};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum ClientError {
    Bad(ValidationError),
    Proto(ProtoError),
    Io(std::io::Error),
}

impl From<ValidationError> for ClientError {
    fn from(e: ValidationError) -> Self {
        Self::Bad(e)
    }
}

impl From<ProtoError> for ClientError {
    fn from(e: ProtoError) -> Self {
        Self::Proto(e)
    }
}

impl From<std::io::Error> for ClientError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// what the ui actually holds onto. proto::Msg borrows the socket buffer so it cant outlive it
#[derive(Debug, Clone)]
pub struct Line {
    pub nick: String,
    pub text: String,
    pub ts: Option<u64>,
}

// need to differentiate betweenn dead srv and just quiet thign
#[derive(Debug, Clone)]
pub enum Event {
    Line(Line),
    Lost(String),
}

pub struct Client {
    url: String,
    nick: Nick,
    id: [u8; 4],
    seq: AtomicU64,
    sub: oneshot::Sender<ObserveMessage>,
    ka: tokio::task::JoinHandle<()>,
}

impl Client {
    pub async fn join(
        relay: &str,
        channel: &str,
        nick: &str,
    ) -> Result<(Self, UnboundedReceiver<Event>), ClientError> {
        let ch = Channel::new(channel)?;
        let nick = Nick::new(nick)?;
        let path = channel_path(&ch);
        let url = format!("coap://{relay}{}", path.as_str());
        let id = fresh_id();
        let seq = AtomicU64::new(0);

        // register ourselves...
        put(
            &url,
            id,
            seq.fetch_add(1, Ordering::Relaxed),
            &nick,
            "joined",
        )
        .await?;

        let sock = UdpCoAPClient::new(relay).await?;
        let (tx, rx) = mpsc::unbounded_channel();
        let sub = sock
            .observe(path.as_str(), move |m| {
                let m = match m {
                    Ok(m) => m,
                    Err(e) => {
                        let _ = tx.send(Event::Lost(e.to_string()));
                        return;
                    }
                };
                let Ok(env) = decode(&m.payload) else { return };
                if let Body::Plain(msg) = env.body {
                    let _ = tx.send(Event::Line(Line {
                        nick: msg.nick.into(),
                        text: msg.text.into(),
                        ts: msg.ts,
                    }));
                }
            })
            .await?;
        let ka = tokio::spawn(keepalive(sock));

        Ok((
            Self {
                url,
                nick,
                id,
                seq,
                sub,
                ka,
            },
            rx,
        ))
    }

    pub async fn send(&self, text: &str) -> Result<(), ClientError> {
        let n = self.seq.fetch_add(1, Ordering::Relaxed);
        put(&self.url, self.id, n, &self.nick, text).await
    }

    pub fn nick(&self) -> &str {
        self.nick.as_str()
    }

    pub fn leave(self) {
        let _ = self.sub.send(ObserveMessage::Terminate);
        self.ka.abort();
    }
}

async fn put(url: &str, id: [u8; 4], seq: u64, nick: &Nick, text: &str) -> Result<(), ClientError> {
    let env = Envelope {
        sender: id,
        seq,
        body: Body::Plain(Msg {
            nick: nick.as_str(),
            text,
            ts: {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_millis() as u64)
            },
        }),
    };
    let mut buf = [0u8; MAX_PACKET_SIZE];
    let n = encode_into(&env, &mut buf)?;
    UdpCoAPClient::put(url, buf[..n].to_vec()).await?;
    Ok(())
}

async fn keepalive(sock: UdpCoAPClient) {
    loop {
        tokio::time::sleep(Duration::from_secs(KEEPALIVE_S as u64)).await;
        let ping = RequestBuilder::new(PING_PATH, RequestType::Get).build();
        let _ = sock.send(ping).await;
    }
}

// TODO: probably make more random for crypto stuff later?
fn fresh_id() -> [u8; 4] {
    let jitter = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    (jitter ^ std::process::id()).to_le_bytes()
}
