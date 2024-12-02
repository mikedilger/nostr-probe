use nostr_probe::{Command, Probe};
use nostr_types::{EventKind, Filter, PublicKey, RelayMessage};
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
            },
        },
        None => panic!("Usage: fetch_by_kind_and_author <RelayURL> <KindNumber> <PubKey>"),
    };

    let signer = nostr_probe::load_signer()?;

    let (to_probe, from_main) = tokio::sync::mpsc::channel::<Command>(100);
    let (to_main, from_probe) = tokio::sync::mpsc::channel::<RelayMessage>(100);
    let relay_url2 = relay_url.clone();
    let join_handle = tokio::spawn(async move {
        let mut probe = Probe::new(from_main, to_main);
        if let Err(e) = probe.connect_and_listen(&relay_url2).await {
            eprintln!("{}", e);
        }
    });

    let filter = Filter {
        kinds: vec![kind],
        authors: vec![author_key],
        ..Default::default()
    };

    nostr_probe::req(&relay_url, signer, filter, to_probe, from_probe).await?;

    Ok(join_handle.await?)
}
