use nostr_probe::{Command, ExitMessage, Probe};
use nostr_types::Event;
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

    let (tx, rx) = tokio::sync::mpsc::channel::<Command>(100);

    let join_handle = tokio::spawn(async move {
        let mut probe = Probe::new(
            rx,
            vec![
                ExitMessage::Ok(event.id, false),
                ExitMessage::Ok(event.id, true),
                ExitMessage::Notice,
            ],
        );

        if let Err(e) = probe.connect_and_listen(&relay_url).await {
            eprintln!("{}", e);
        }
    });

    tx.send(Command::PostEvent(event)).await?;

    Ok(join_handle.await?)
}
