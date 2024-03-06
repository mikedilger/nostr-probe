use base64::Engine;
use colorful::{Color, Colorful};
use futures_util::stream::FusedStream;
use futures_util::{SinkExt, StreamExt};
use http::Uri;
use lazy_static::lazy_static;
use nostr_types::{ClientMessage, Event, Id, RelayMessage, SubscriptionId};
use tungstenite::Message;

pub struct Prefixes {
    from_relay: String,
}

lazy_static! {
    pub static ref PREFIXES: Prefixes = Prefixes {
        from_relay: "Relay".color(Color::Blue).to_string()
    };
}

pub enum Command {
    PostEvent(Event),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitMessage {
    Auth,
    Closed(SubscriptionId),
    Eose(SubscriptionId),
    Event(SubscriptionId),
    Notice,
    Ok(Id, bool),
}

pub struct Probe {
    pub rx: tokio::sync::mpsc::Receiver<Command>,
    pub exit_on: Vec<ExitMessage>,
}

impl Probe {
    pub fn new(rx: tokio::sync::mpsc::Receiver<Command>, exit_on: Vec<ExitMessage>) -> Probe {
        Probe { rx, exit_on }
    }

    pub async fn connect_and_listen(
        &mut self,
        relay_url: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (host, uri) = url_to_host_and_uri(relay_url);

        let key: [u8; 16] = rand::random();
        let request = http::request::Request::builder()
            .method("GET")
            .header("Host", host)
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header(
                "Sec-WebSocket-Key",
                base64::engine::general_purpose::STANDARD.encode(key),
            )
            .uri(uri)
            .body(())?;

        let (mut websocket, _response) = tokio::time::timeout(
            std::time::Duration::new(5, 0),
            tokio_tungstenite::connect_async(request),
        )
        .await??;

        let mut ping_timer = tokio::time::interval(std::time::Duration::new(15, 0));
        ping_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        ping_timer.tick().await; // use up the first immediate tick.

        loop {
            tokio::select! {
                _ = ping_timer.tick() => {
                    websocket.send(Message::Ping(vec![0x1])).await?;
                },
                local_message = self.rx.recv() => {
                    match local_message {
                        Some(Command::PostEvent(event)) => {
                            let client_message = ClientMessage::Event(Box::new(event));
                            let wire = serde_json::to_string(&client_message)?;
                            websocket.send(Message::Text(wire)).await?;
                        },
                        None => { }
                    }
                },
                message = websocket.next() => {
                    let message = match message {
                        Some(m) => m,
                        None => {
                            if websocket.is_terminated() {
                                eprintln!("{}", "Connection terminated".color(Color::Orange1));
                            }
                            break;
                        }
                    }?;

                    match message {
                        Message::Text(s) => {
                            let relay_message: RelayMessage = serde_json::from_str(&s)?;
                            match relay_message {
                                RelayMessage::Auth(challenge) => {
                                    println!("{}: AUTH({})", PREFIXES.from_relay, challenge);
                                    if self.exit_on.contains(&ExitMessage::Auth) {
                                        println!("Exiting on Auth");
                                        break;
                                    }
                                }
                                RelayMessage::Event(sub, e) => {
                                    let event_json = serde_json::to_string(&e)?;
                                    println!("{}: EVENT({}, {})", PREFIXES.from_relay, sub.as_str(), event_json);
                                    if self.exit_on.contains(&ExitMessage::Event(sub)) {
                                        println!("Exiting on matching Event(sub)");
                                        break;
                                    }
                                }
                                RelayMessage::Closed(sub, msg) => {
                                    println!("{}: CLOSED({}, {})", PREFIXES.from_relay, sub.as_str(), msg);
                                    if self.exit_on.contains(&ExitMessage::Closed(sub)) {
                                        println!("Exiting on matching Closed(sub)");
                                        break;
                                    }
                                }
                                RelayMessage::Notice(s) => {
                                    println!("{}: NOTICE({})", PREFIXES.from_relay, s);
                                    if self.exit_on.contains(&ExitMessage::Notice) {
                                        println!("Exiting on Notice");
                                        break;
                                    }
                                }
                                RelayMessage::Eose(sub) => {
                                    println!("{}: EOSE({})", PREFIXES.from_relay, sub.as_str());
                                    if self.exit_on.contains(&ExitMessage::Eose(sub)) {
                                        println!("Exiting on matching Eose(sub)");
                                        break;
                                    }
                                }
                                RelayMessage::Ok(id, ok, reason) => {
                                    println!("{}: OK({}, {}, {})", PREFIXES.from_relay, id.as_hex_string(), ok, reason);
                                    if self.exit_on.contains(&ExitMessage::Ok(id, ok)) {
                                        println!("Exiting on matching Ok(id, ok)");
                                        break;
                                    }
                                }
                            }
                        },
                        Message::Binary(_) => {
                            eprintln!("{}: Binary message received!!!", PREFIXES.from_relay);
                        },
                        Message::Ping(_) => {
                            eprintln!("{}: Ping", PREFIXES.from_relay);
                        },
                        Message::Pong(_) => {
                            eprintln!("{}: Pong", PREFIXES.from_relay);
                        },
                        Message::Close(_) => {
                            eprintln!("{}", "Remote closed nicely.".color(Color::Green));
                            break;
                        }
                        Message::Frame(_) => {
                            unreachable!()
                        }
                    }
                },
            }
        }

        Ok(())
    }
}

pub fn url_to_host_and_uri(url: &str) -> (String, Uri) {
    let uri: http::Uri = url.parse::<http::Uri>().expect("Could not parse url");
    let authority = uri.authority().expect("Has no hostname").as_str();
    let host = authority
        .find('@')
        .map(|idx| authority.split_at(idx + 1).1)
        .unwrap_or_else(|| authority);
    if host.is_empty() {
        panic!("URL has empty hostname");
    }
    (host.to_owned(), uri)
}
