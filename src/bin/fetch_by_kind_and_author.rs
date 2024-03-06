use nostr_probe::{Command, ExitMessage, Probe};
use nostr_types::{EventKind, Filter, PublicKey, PublicKeyHex, SubscriptionId};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    let _ = args.next(); // program name
    let relay_url = match args.next() {
        Some(u) => u,
        None => panic!("Usage: fetch_by_kind_and_author <RelayURL> <KindNumber> <PubKey>"),
    };
    let kind_number = match args.next() {
        Some(num) => num.parse::<u32>()?,
        None => panic!("Usage: fetch_by_kind_and_author <RelayURL> <KindNumber> <PubKey>"),
    };
    let kind: EventKind = kind_number.into();
    let author_key = match args.next() {
        Some(key) => match PublicKey::try_from_bech32_string(&key, true) {
            Ok(key) => key,
            Err(_) => match PublicKey::try_from_hex_string(&key, true) {
                Ok(key) => key,
                Err(_) => panic!("Could not parse public key"),
            }
        },
        None => panic!("Usage: fetch_by_kind_and_author <RelayURL> <KindNumber> <PubKey>"),
    };
    let key: PublicKeyHex = author_key.into();

    let (tx, rx) = tokio::sync::mpsc::channel::<Command>(100);

    let our_sub_id = SubscriptionId("fetch_by_kind_and_author".to_string());
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

    let filter = Filter {
        kinds: vec![kind],
        authors: vec![key],
        .. Default::default()
    };

    tx.send(Command::FetchEvents(our_sub_id, vec![filter])).await?;

    Ok(join_handle.await?)
}
