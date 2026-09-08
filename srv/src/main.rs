use std::net::SocketAddr;

use coap::Server;
use coap_lite::{CoapRequest, RequestType, ResponseType};
use orbit_proto::consts::COAP_PORT;
use orbit_proto::packet::{PING_PATH, channel_from_path, decode};
use orbit_proto::types::Body;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| format!("127.0.0.1:{COAP_PORT}"));
    println!("server running on {addr}");

    Server::new_udp(&addr)?
        .run(|mut req: Box<CoapRequest<SocketAddr>>| async move {
            let path = req.get_path();
            let peer = req.source.map(|a| a.to_string()).unwrap_or_default();
            let mut status = ResponseType::NotFound;

            match *req.get_method() {
                RequestType::Get => {
                    if path.trim_start_matches('/') == PING_PATH.trim_start_matches('/') {
                        status = ResponseType::Content;
                    }
                }
                RequestType::Put => {
                    match (channel_from_path(&path), decode(&req.message.payload)) {
                        (Ok(ch), Ok(env)) => {
                            match env.body {
                                Body::Plain(m) => {
                                    println!("{} <{}> \"{}\"", ch.as_str(), m.nick, m.text)
                                }
                                Body::Sealed(b) => {
                                    println!("{} got sealed packet, {} bytes", ch.as_str(), b.len())
                                }
                            }
                            status = ResponseType::Changed;
                        }
                        (c, e) => {
                            if c.err().or(e.err()).is_some() {
                                println!("rejected thing to {path} from {peer}");
                            }
                            status = ResponseType::BadRequest;
                        }
                    }
                }
                _ => {}
            }

            if let Some(r) = req.response.as_mut() {
                r.set_status(status);
                r.message.payload = Vec::new();
            }
            req
        })
        .await?;
    Ok(())
}
