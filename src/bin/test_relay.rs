use nostr_probe::{Command, ExitMessage, Probe};
use nostr_types::{EventKind, Filter, IdHex, KeySigner, PreEvent, PrivateKey, Signer, SubscriptionId, Unixtime};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    let _ = args.next(); // program name
    let relay_url = match args.next() {
        Some(u) => u,
        None => panic!("Usage: test_relay <RelayURL>"),
    };

    // Create a new identity
    eprintln!("Generating keypair...");
    let private_key = PrivateKey::generate();
    let public_key = private_key.public_key();
    let signer = KeySigner::from_private_key(private_key, "pass", 16).unwrap();

    // Create an event for testing the relay
    let pre_event = PreEvent {
        pubkey: public_key,
        created_at: Unixtime::now().unwrap(),
        kind: EventKind::TextNote,
        content: "Hello. This is a test to see if this relay accepts notes from new people. \
                  This is from an ephemeral keypair, and this note can be ignored or deleted."
            .to_owned(),
        tags: vec![],
    };
    let event = signer.sign_event(pre_event).unwrap();
    event.verify(None).unwrap();

    let our_sub_id = SubscriptionId("fetch_by_id".to_string());
    let cloned_sub_id = our_sub_id.clone();

    // Connect to relay and handle commands
    let (tx, rx) = tokio::sync::mpsc::channel::<Command>(100);
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

    let id: IdHex = event.id.into();

    tx.send(Command::PostEvent(event)).await?;

    // Ideally we would be triggered by a relay message, but Probe doesn't talk to us.
    tokio::time::sleep(std::time::Duration::new(1, 0)).await;

    let mut filter = Filter::new();
    filter.add_id(&id);
    tx.send(Command::FetchEvents(our_sub_id, vec![filter])).await?;

    Ok(join_handle.await?)
}
