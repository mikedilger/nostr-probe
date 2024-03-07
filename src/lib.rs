use base64::Engine;
use colorful::{Color, Colorful};
use futures_util::stream::FusedStream;
use futures_util::{SinkExt, StreamExt};
use http::Uri;
use lazy_static::lazy_static;
use nostr_types::{ClientMessage, Event, Filter, Id, RelayMessage, SubscriptionId};
use tungstenite::Message;

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub struct Prefixes {
    from_relay: String,
    sending: String,
}

lazy_static! {
    pub static ref PREFIXES: Prefixes = Prefixes {
        from_relay: "Relay".color(Color::Blue).to_string(),
        sending: "Sending".color(Color::MediumPurple).to_string(),
    };
}

pub enum Command {
    PostEvent(Event),
    FetchEvents(SubscriptionId, Vec<Filter>),
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

        let mut events: Vec<Event> = vec![];

        let mut ping_timer = tokio::time::interval(std::time::Duration::new(15, 0));
        ping_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        ping_timer.tick().await; // use up the first immediate tick.

        loop {
            tokio::select! {
                _ = ping_timer.tick() => {
                    let msg = Message::Ping(vec![0x1]);
                    self.send(&mut websocket, msg).await?;
                },
                local_message = self.rx.recv() => {
                    match local_message {
                        Some(Command::PostEvent(event)) => {
                            let client_message = ClientMessage::Event(Box::new(event));
                            let wire = serde_json::to_string(&client_message)?;
                            let msg = Message::Text(wire);
                            self.send(&mut websocket, msg).await?;
                        },
                        Some(Command::FetchEvents(subid, filters)) => {
                            let client_message = ClientMessage::Req(subid, filters);
                            let wire = serde_json::to_string(&client_message)?;
                            let msg = Message::Text(wire);
                            self.send(&mut websocket, msg).await?;
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
                                    eprintln!("{}: AUTH({})", PREFIXES.from_relay, challenge);
                                    if self.exit_on.contains(&ExitMessage::Auth) {
                                        eprintln!("Exiting on Auth");
                                        break;
                                    }
                                }
                                RelayMessage::Event(sub, e) => {
                                    let event_json = serde_json::to_string(&e)?;
                                    eprintln!("{}: EVENT({}, {})", PREFIXES.from_relay, sub.as_str(), event_json);
                                    events.push(*e);
                                    if self.exit_on.contains(&ExitMessage::Event(sub)) {
                                        eprintln!("Exiting on matching Event(sub)");
                                        break;
                                    }
                                }
                                RelayMessage::Closed(sub, msg) => {
                                    eprintln!("{}: CLOSED({}, {})", PREFIXES.from_relay, sub.as_str(), msg);
                                    if self.exit_on.contains(&ExitMessage::Closed(sub)) {
                                        eprintln!("Exiting on matching Closed(sub)");
                                        break;
                                    }
                                }
                                RelayMessage::Notice(s) => {
                                    eprintln!("{}: NOTICE({})", PREFIXES.from_relay, s);
                                    if self.exit_on.contains(&ExitMessage::Notice) {
                                        eprintln!("Exiting on Notice");
                                        break;
                                    }
                                }
                                RelayMessage::Eose(sub) => {
                                    eprintln!("{}: EOSE({})", PREFIXES.from_relay, sub.as_str());
                                    if self.exit_on.contains(&ExitMessage::Eose(sub)) {
                                        eprintln!("Exiting on matching Eose(sub)");
                                        break;
                                    }
                                }
                                RelayMessage::Ok(id, ok, reason) => {
                                    eprintln!("{}: OK({}, {}, {})", PREFIXES.from_relay, id.as_hex_string(), ok, reason);
                                    if self.exit_on.contains(&ExitMessage::Ok(id, ok)) {
                                        eprintln!("Exiting on matching Ok(id, ok)");
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

        // Send close message before disconnecting
        let msg = Message::Close(None);
        self.send(&mut websocket, msg).await?;

        eprintln!();

        // Print all events on stdout
        for event in &events {
            println!("{}", serde_json::to_string(&event)?);
        }

        Ok(())
    }

    async fn send(
        &mut self,
        websocket: &mut Ws,
        message: Message,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match message {
            Message::Text(ref s) => eprintln!("{}: Text({})", PREFIXES.sending, s),
            Message::Binary(_) => eprintln!("{}: Binary(_)", PREFIXES.sending),
            Message::Ping(_) => eprintln!("{}: Ping(_)", PREFIXES.sending),
            Message::Pong(_) => eprintln!("{}: Pong(_)", PREFIXES.sending),
            Message::Close(_) => eprintln!("{}: Close(_)", PREFIXES.sending),
            Message::Frame(_) => eprintln!("{}: Frame(_)", PREFIXES.sending),
        }
        Ok(websocket.send(message).await?)
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
