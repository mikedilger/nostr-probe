use nostr_probe::{Command, Probe};
use nostr_types::{
    EventKind, Filter, KeySigner, PreEvent, PrivateKey, RelayMessage, Signer, SubscriptionId,
    Unixtime,
};
use std::env;

#[tokio::main]
async fn main() {
    if let Err(e) = inner().await {
        eprintln!("{}", e);
    }

    eprintln!("FAILED ON ERROR");
    std::process::exit(1);
}

async fn inner() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    let _ = args.next(); // program name
    let relay_url = match args.next() {
        Some(u) => u,
        None => panic!("Usage: test_relay <RelayURL>"),
    };

    // Create a new identity
    // eprintln!("Generating keypair...");
    let private_key = PrivateKey::generate();
    let public_key = private_key.public_key();
    let signer = KeySigner::from_private_key(private_key, "pass", 16).unwrap();

    // Create an event for testing the relay
    let pre_event = PreEvent {
        pubkey: public_key,
        created_at: Unixtime::now(),
        kind: EventKind::TextNote,
        content: "Hello. This is a test to see if this relay accepts notes from new people. \
                  This is from an ephemeral keypair, and this note can be ignored or deleted."
            .to_owned(),
        tags: vec![],
    };
    let event = signer.sign_event(pre_event).unwrap();
    event.verify(None).unwrap();

    // Connect to relay and handle commands
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
            RelayMessage::Ok(id, success, message) => {
                if id == event.id {
                    if !success {
                        eprintln!("FAILED: {}", message);
                        std::process::exit(1);
                    }
                    break;
                }
            }
            RelayMessage::Notice(notice) => {
                eprintln!("FAILED ON NOTICE: {}", notice);
                std::process::exit(1);
                //to_probe.send(Command::Exit).await?;
                //return Ok(join_handle.await?);
            }
            _ => {}
        }
    }

    let our_sub_id = SubscriptionId("fetch_by_id".to_string());
    let mut filter = Filter::new();
    filter.add_id(event.id);
    to_probe
        .send(Command::FetchEvents(our_sub_id.clone(), vec![filter]))
        .await?;

    loop {
        match from_probe.recv().await.unwrap() {
            RelayMessage::Eose(subid) => {
                if subid == our_sub_id {
                    to_probe.send(Command::Exit).await?;
                    break;
                }
            }
            RelayMessage::Closed(subid, _) => {
                if subid == our_sub_id {
                    to_probe.send(Command::Exit).await?;
                    break;
                }
            }
            RelayMessage::Event(subid, e) => {
                if subid == our_sub_id && e.id == event.id {
                    eprintln!("SUCCESS - THIS IS AN OPEN RELAY");
                    std::process::exit(0);
                    //to_probe.send(Command::Exit).await?;
                    //break;
                }
            }
            RelayMessage::Notice(_) => {
                to_probe.send(Command::Exit).await?;
                break;
            }
            _ => {}
        }
    }

    join_handle.await?;

    Ok(())
}
