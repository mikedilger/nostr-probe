use nostr_probe::{Command, ExitMessage, Probe};
use nostr_types::Event;
use std::env;
use std::fs;
use std::io::Read;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    let _ = args.next(); // program name
    let relay_url = match args.next() {
        Some(u) => u,
        None => panic!("Usage: post_from_files <RelayURL> <Directory>"),
    };
    let directory = match args.next() {
        Some(d) => d,
        None => panic!("Usage: post_from_files <RelayURL> <Directory>"),
    };

    let mut events: Vec<Event> = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let mut file = fs::OpenOptions::new()
            .read(true)
            .open(entry.path())
            .unwrap();
        let mut contents: String = String::new();
        file.read_to_string(&mut contents).unwrap();
        let event: Event = serde_json::from_str(&contents).unwrap();
        event.verify(None).unwrap();
        events.push(event);
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<Command>(100);

    let join_handle = tokio::spawn(async move {
        let mut probe = Probe::new(rx, vec![ExitMessage::Notice]);

        if let Err(e) = probe.connect_and_listen(&relay_url).await {
            eprintln!("{}", e);
        }
    });

    for event in &events {
        tx.send(Command::PostEvent(event.clone())).await?;

        // Ideally wait for the response, but our Probe doesn't talk to us.
        tokio::time::sleep(std::time::Duration::new(0, 250)).await;
    }

    tx.send(Command::Exit).await?;

    Ok(join_handle.await?)
}
