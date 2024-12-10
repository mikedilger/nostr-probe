use nostr_probe::{Command, Probe};
use nostr_types::{Filter, RelayMessage, SubscriptionId};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    let _ = args.next(); // program name
    let relay_url = match args.next() {
        Some(u) => u,
        None => panic!("Usage: fetch_by_filter <RelayURL> <FilterJSON>"),
    };
    let filter: Filter = match args.next() {
        Some(filter) => match serde_json::from_str(&filter) {
            Ok(f) => f,
            Err(e) => panic!("{}", e),
        },
        None => panic!("Usage: fetch_by_filter <RelayURL> <FilterJSON>"),
    };

    let (to_probe, from_main) = tokio::sync::mpsc::channel::<Command>(100);
    let (to_main, mut from_probe) = tokio::sync::mpsc::channel::<RelayMessage>(100);
    let join_handle = tokio::spawn(async move {
        let mut probe = Probe::new(from_main, to_main);
        if let Err(e) = probe.connect_and_listen(&relay_url).await {
            eprintln!("{}", e);
        }
    });

    let our_sub_id = SubscriptionId("count_by_filter".to_string());
    to_probe
        .send(Command::CountEvents(our_sub_id.clone(), vec![filter]))
        .await?;

    loop {
        match from_probe.recv().await.unwrap() {
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
            RelayMessage::Count(sub, result) => {
                if sub == our_sub_id {
                    println!("{}", serde_json::to_string(&result)?);
                }
            }
            RelayMessage::Closed(sub, _) => {
                if sub == our_sub_id {
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
