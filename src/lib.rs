use base64::Engine;
use colorful::{Color, Colorful};
use futures_util::stream::FusedStream;
use futures_util::{SinkExt, StreamExt};
use http::Uri;
use lazy_static::lazy_static;
use nostr_types::{
    ClientMessage, EncryptedPrivateKey, Event, EventKind, Filter, Id, KeySigner, PreEvent,
    RelayMessage, Signer, SubscriptionId, Tag, Unixtime, Why,
};
use tokio::sync::mpsc::{Receiver, Sender};
use tungstenite::Message;
use zeroize::Zeroize;

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
    Auth(Event),
    FetchEvents(SubscriptionId, Vec<Filter>),
    Exit,
}

pub struct Probe {
    pub from_main: tokio::sync::mpsc::Receiver<Command>,
    pub to_main: tokio::sync::mpsc::Sender<RelayMessage>,
}

impl Probe {
    pub fn new(
        from_main: tokio::sync::mpsc::Receiver<Command>,
        to_main: tokio::sync::mpsc::Sender<RelayMessage>,
    ) -> Probe {
        Probe { from_main, to_main }
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
                    let msg = Message::Ping(vec![0x1]);
                    self.send(&mut websocket, msg).await?;
                },
                local_message = self.from_main.recv() => {
                    match local_message {
                        Some(Command::PostEvent(event)) => {
                            let client_message = ClientMessage::Event(Box::new(event));
                            let wire = serde_json::to_string(&client_message)?;
                            let msg = Message::Text(wire);
                            self.send(&mut websocket, msg).await?;
                        },
                        Some(Command::Auth(event)) => {
                            let client_message = ClientMessage::Auth(Box::new(event));
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
                        Some(Command::Exit) => {
                            break;
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

                    // Display it
                    Self::display(message.clone())?;

                    // Take action
                    match message {
                        Message::Text(s) => {
                            // Send back to main
                            let relay_message: RelayMessage = serde_json::from_str(&s)?;
                            self.to_main.send(relay_message).await?;
                        },
                        Message::Binary(_) => { },
                        Message::Ping(_) => { },
                        Message::Pong(_) => { },
                        Message::Close(_) => break,
                        Message::Frame(_) => unreachable!(),
                    }
                },
            }
        }

        // Send close message before disconnecting
        let msg = Message::Close(None);
        self.send(&mut websocket, msg).await?;

        Ok(())
    }

    fn display(message: Message) -> Result<(), Box<dyn std::error::Error>> {
        match message {
            Message::Text(s) => {
                let relay_message: RelayMessage = serde_json::from_str(&s)?;
                match relay_message {
                    RelayMessage::Auth(challenge) => {
                        eprintln!("{}: AUTH({})", PREFIXES.from_relay, challenge);
                    }
                    RelayMessage::Event(sub, e) => {
                        let event_json = serde_json::to_string(&e)?;
                        eprintln!(
                            "{}: EVENT({}, {})",
                            PREFIXES.from_relay,
                            sub.as_str(),
                            event_json
                        );
                    }
                    RelayMessage::Closed(sub, msg) => {
                        eprintln!("{}: CLOSED({}, {})", PREFIXES.from_relay, sub.as_str(), msg);
                    }
                    RelayMessage::Notice(s) => {
                        eprintln!("{}: NOTICE({})", PREFIXES.from_relay, s);
                    }
                    RelayMessage::Eose(sub) => {
                        eprintln!("{}: EOSE({})", PREFIXES.from_relay, sub.as_str());
                    }
                    RelayMessage::Ok(id, ok, reason) => {
                        eprintln!(
                            "{}: OK({}, {}, {})",
                            PREFIXES.from_relay,
                            id.as_hex_string(),
                            ok,
                            reason
                        );
                    }
                }
            }
            Message::Binary(_) => {
                eprintln!("{}: Binary message received!!!", PREFIXES.from_relay);
            }
            Message::Ping(_) => {
                eprintln!("{}: Ping", PREFIXES.from_relay);
            }
            Message::Pong(_) => {
                eprintln!("{}: Pong", PREFIXES.from_relay);
            }
            Message::Close(_) => {
                eprintln!("{}", "Remote closed nicely.".color(Color::Green));
            }
            Message::Frame(_) => {
                unreachable!()
            }
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

pub fn load_signer() -> Result<KeySigner, Box<dyn std::error::Error>> {
    let mut config_dir = match dirs::config_dir() {
        Some(cd) => cd,
        None => panic!("No config_dir defined for your operating system"),
    };
    config_dir.push("nostr-probe");
    config_dir.push("epk");

    let epk_bytes = match std::fs::read(&config_dir) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("{}", e);
            panic!(
                "Could not find your encrypted private key in {}",
                config_dir.display()
            );
        }
    };
    let epk_string = String::from_utf8(epk_bytes)?;
    let epk = EncryptedPrivateKey(epk_string);

    let mut password = rpassword::prompt_password("Password: ").unwrap();
    let mut signer = KeySigner::from_encrypted_private_key(epk, &password)?;
    signer.unlock(&password)?;
    password.zeroize();
    Ok(signer)
}

pub async fn req(
    relay_url: &str,
    signer: KeySigner,
    filter: Filter,
    to_probe: Sender<Command>,
    mut from_probe: Receiver<RelayMessage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let pubkey = signer.public_key();
    let mut authenticated: Option<Id> = None;

    let our_sub_id = SubscriptionId("subscription-id".to_string());
    to_probe
        .send(Command::FetchEvents(
            our_sub_id.clone(),
            vec![filter.clone()],
        ))
        .await?;

    loop {
        let relay_message = from_probe.recv().await.unwrap();
        let why = relay_message.why();
        match relay_message {
            RelayMessage::Auth(challenge) => {
                let pre_event = PreEvent {
                    pubkey,
                    created_at: Unixtime::now(),
                    kind: EventKind::Auth,
                    tags: vec![
                        Tag::new(&["relay", relay_url]),
                        Tag::new(&["challenge", &challenge]),
                    ],
                    content: "".to_string(),
                };
                let event = signer.sign_event(pre_event)?;
                authenticated = Some(event.id);
                to_probe.send(Command::Auth(event)).await?;
            }
            RelayMessage::Eose(sub) => {
                if sub == our_sub_id {
                    to_probe.send(Command::Exit).await?;
                    break;
                }
            }
            RelayMessage::Event(sub, e) => {
                if sub == our_sub_id {
                    println!("{}", serde_json::to_string(&e)?);
                }
            }
            RelayMessage::Closed(sub, _) => {
                if sub == our_sub_id {
                    if why == Some(Why::AuthRequired) {
                        if authenticated.is_none() {
                            eprintln!("Relay CLOSED our sub due to auth-required, but it has not AUTHed us! (Relay is buggy)");
                            to_probe.send(Command::Exit).await?;
                            break;
                        }

                        // We have already authenticated. We will resubmit once we get the
                        // OK message.
                    } else {
                        to_probe.send(Command::Exit).await?;
                        break;
                    }
                }
            }
            RelayMessage::Notice(_) => {
                to_probe.send(Command::Exit).await?;
                break;
            }
            RelayMessage::Ok(id, is_ok, reason) => {
                if let Some(authid) = authenticated {
                    if authid == id {
                        if is_ok {
                            // Resubmit our request
                            to_probe
                                .send(Command::FetchEvents(
                                    our_sub_id.clone(),
                                    vec![filter.clone()],
                                ))
                                .await?;
                        } else {
                            eprintln!("AUTH failed: {}", reason);
                            to_probe.send(Command::Exit).await?;
                            break;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
