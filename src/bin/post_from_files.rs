use nostr_probe::{Command, Probe};
use nostr_types::{Event, RelayMessage};
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

    let (to_probe, from_main) = tokio::sync::mpsc::channel::<Command>(100);
    let (to_main, mut from_probe) = tokio::sync::mpsc::channel::<RelayMessage>(100);
    let join_handle = tokio::spawn(async move {
        let mut probe = Probe::new(from_main, to_main);
        if let Err(e) = probe.connect_and_listen(&relay_url).await {
            eprintln!("{}", e);
        }
    });

    'events: for event in &events {
        to_probe.send(Command::PostEvent(event.clone())).await?;

        // Wait for OK
        loop {
            match from_probe.recv().await.unwrap() {
                RelayMessage::Ok(id, _, _) => {
                    if id == event.id {
                        break;
                    }
                }
                RelayMessage::Notice(_) => {
                    to_probe.send(Command::Exit).await?;
                    break 'events;
                }
                _ => {}
            }
        }
    }

    to_probe.send(Command::Exit).await?;

    Ok(join_handle.await?)
}
