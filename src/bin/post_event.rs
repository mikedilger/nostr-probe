use nostr_probe::{Command, Probe};
use nostr_types::{Event, RelayMessage};
use std::env;
use std::io::Read;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    let _ = args.next(); // program name
    let relay_url = match args.next() {
        Some(u) => u,
        None => panic!("Usage: post_event <RelayURL> < EventJSON"),
    };

    let mut s: String = String::new();
    std::io::stdin().read_to_string(&mut s)?;
    let event: Event = serde_json::from_str(&s)?;
    event.verify(None)?;

    let (to_probe, from_main) = tokio::sync::mpsc::channel::<Command>(100);
    let (to_main, mut from_probe) = tokio::sync::mpsc::channel::<RelayMessage>(100);
    let join_handle = tokio::spawn(async move {
        let mut probe = Probe::new(from_main, to_main);
        if let Err(e) = probe.connect_and_listen(&relay_url).await {
            eprintln!("{}", e);
        }
    });

    to_probe.send(Command::PostEvent(event.clone())).await?;

    loop {
        match from_probe.recv().await.unwrap() {
            RelayMessage::Ok(id, _, _) => {
                if id == event.id {
                    to_probe.send(Command::Exit).await?;
                    break;
                }
            }
            RelayMessage::Notice(_) => {
                to_probe.send(Command::Exit).await?;
                break;
            }
            _ => {}
        }
    }

    Ok(join_handle.await?)
}
