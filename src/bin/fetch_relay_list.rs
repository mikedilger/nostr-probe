use nostr_probe::{Command, ExitMessage, Probe};
use nostr_types::{EventKind, Filter, PublicKeyHex, SubscriptionId};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    let _ = args.next(); // program name
    let relay_url = match args.next() {
        Some(u) => u,
        None => panic!("Usage: fetch_relay_list <RelayURL> <PubKeyHex>"),
    };
    let pubkeyhex: PublicKeyHex = match args.next() {
        Some(id) => PublicKeyHex::try_from_str(&id)?,
        None => panic!("Usage: fetch_relay_list <RelayURL> <PubKeyHex>"),
    };

    let (tx, rx) = tokio::sync::mpsc::channel::<Command>(100);

    let our_sub_id = SubscriptionId("fetch_relay_list".to_string());
    let cloned_sub_id = our_sub_id.clone();

    let join_handle = tokio::spawn(async move {
        let mut probe = Probe::new(
            rx,
            vec![
                ExitMessage::Eose(cloned_sub_id.clone()),
                ExitMessage::Closed(cloned_sub_id),
                ExitMessage::Notice,
            ],
        );

        if let Err(e) = probe.connect_and_listen(&relay_url).await {
            eprintln!("{}", e);
        }
    });

    let mut filter = Filter::new();
    filter.add_author(&pubkeyhex);
    filter.add_event_kind(EventKind::RelayList);
    filter.limit = Some(1);

    tx.send(Command::FetchEvents(our_sub_id, vec![filter]))
        .await?;

    Ok(join_handle.await?)
}
