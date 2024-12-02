use nostr_probe::{Command, Probe};
use nostr_types::{Filter, Id, RelayMessage, SubscriptionId};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    let _ = args.next(); // program name
    let relay_url = match args.next() {
        Some(u) => u,
        None => panic!("Usage: fetch_by_id <RelayURL> <IdHex>"),
    };
    let id: Id = match args.next() {
        Some(id) => Id::try_from_hex_string(&id)?,
        None => panic!("Usage: fetch_by_id <RelayURL> <IdHex>"),
    };

    let (to_probe, from_main) = tokio::sync::mpsc::channel::<Command>(100);
    let (to_main, mut from_probe) = tokio::sync::mpsc::channel::<RelayMessage>(100);
    let join_handle = tokio::spawn(async move {
        let mut probe = Probe::new(from_main, to_main);
        if let Err(e) = probe.connect_and_listen(&relay_url).await {
            eprintln!("{}", e);
        }
    });

    let mut filter = Filter::new();
    filter.add_id(id);

    let our_sub_id = SubscriptionId("fetch_by_id".to_string());
    to_probe
        .send(Command::FetchEvents(our_sub_id.clone(), vec![filter]))
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
