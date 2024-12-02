use base64::Engine;
use nostr_probe::{Command, Probe};
use nostr_types::{
    ContentEncryptionAlgorithm, Event, EventKind, Filter, KeySigner, PreEvent, PrivateKey,
    PublicKey, RelayMessage, RelayUrl, Signer, SubscriptionId, Tag, Unixtime,
};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize, Deserialize)]
struct NostrConnectRequest {
    pub id: String,
    pub method: String,
    pub params: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NostrConnectResult {
    pub id: String,
    pub result: String,
    pub error: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    let _ = args.next(); // program name
    let bunker_url = match args.next() {
        Some(u) => u,
        None => panic!("Usage: test_nip46 <BunkerURL>"),
    };

    if !bunker_url.starts_with("bunker://") {
        panic!("BunkerURL does not start with bunker://");
    }

    let parts: Vec<&str> = bunker_url[9..].split('?').collect();
    if parts.len() != 2 {
        panic!("BunkerURL does not have two parts separated by a '?'");
    }

    let remote_pubkey = {
        let remote_pkhex = parts[0].to_owned();
        PublicKey::try_from_hex_string(&remote_pkhex, true)?
    };

    let mut relays: Vec<RelayUrl> = Vec::new();
    let mut secret: Option<String> = None;
    {
        let params: Vec<&str> = parts[1].split('&').collect();
        for param in &params {
            let bits: Vec<&str> = param.split('=').collect();
            if bits.len() != 2 {
                panic!("Malformed BunkerURL");
            }
            match bits[0] {
                "relay" => {
                    let url = RelayUrl::try_from_str(bits[1])?;
                    relays.push(url);
                }
                "secret" => {
                    secret = Some(bits[1].to_owned());
                }
                _ => {}
            }
        }
    }

    if relays.is_empty() {
        panic!("No relays specified!");
    }

    let relay_url = relays[0].clone();

    // Create a local identity
    let ephemeral_private_key = PrivateKey::generate();
    let ephemeral_public_key = ephemeral_private_key.public_key();
    let ephemeral_signer = KeySigner::from_private_key(ephemeral_private_key, "pass", 16).unwrap();

    // Connect to relay and handle commands
    let (to_probe, from_main) = tokio::sync::mpsc::channel::<Command>(100);
    let (to_main, mut from_probe) = tokio::sync::mpsc::channel::<RelayMessage>(100);
    let join_handle = tokio::spawn(async move {
        let mut probe = Probe::new(from_main, to_main);
        if let Err(e) = probe.connect_and_listen(relay_url.as_str()).await {
            eprintln!("{}", e);
        }
    });

    let connect_request_id = {
        let data: [u8; 16] = rand::random();
        base64::engine::general_purpose::STANDARD.encode(data)
    };

    // Subscribe to nostr-connect events from our peer on this relay
    let our_sub_id = SubscriptionId("test_nip46".to_string());
    let mut filter = Filter::new();
    filter.add_author(remote_pubkey);
    filter.add_event_kind(EventKind::NostrConnect);
    filter.add_tag_value('p', ephemeral_public_key.as_hex_string());
    to_probe
        .send(Command::FetchEvents(our_sub_id.clone(), vec![filter]))
        .await?;

    // Generate connect event
    let connect_event = {
        let plaintext_content = match secret {
            Some(s) => format!(
                r#"{{"id":"{}","method":"connect","params":["{}","{}"]}}"#,
                connect_request_id,
                remote_pubkey.as_hex_string(),
                s
            ),
            None => format!(
                r#"{{"id":"{}","method":"connect","params":["{}"]}}"#,
                connect_request_id,
                remote_pubkey.as_hex_string()
            ),
        };

        let encrypted_content = ephemeral_signer.encrypt(
            &remote_pubkey,
            &plaintext_content,
            ContentEncryptionAlgorithm::Nip04,
        )?;

        let pre_event = PreEvent {
            pubkey: ephemeral_public_key,
            created_at: Unixtime::now(),
            kind: EventKind::NostrConnect,
            content: encrypted_content,
            tags: vec![Tag::new(&["p", &remote_pubkey.as_hex_string()])],
        };

        let connect_event = ephemeral_signer.sign_event(pre_event).unwrap();
        connect_event.verify(None).unwrap();
        connect_event
    };

    to_probe.send(Command::PostEvent(connect_event)).await?;

    // Wait for an event from the remote
    let reply_event: Event;
    loop {
        match from_probe.recv().await.unwrap() {
            RelayMessage::Event(sub, e) => {
                if sub == our_sub_id {
                    reply_event = *e;
                    break;
                }
            }
            RelayMessage::Notice(_) => {
                to_probe.send(Command::Exit).await?;
                return Ok(join_handle.await?);
            }
            _ => {}
        }
    }

    // Decrypt this event
    let contents = ephemeral_signer.decrypt_event_contents(&reply_event)?;
    let ncresult: NostrConnectResult = serde_json::from_str(&contents)?;
    if ncresult.id != connect_request_id {
        panic!(
            "Response doesn't match our connect_request_id: {} != {}",
            ncresult.id, connect_request_id
        );
    }
    if ncresult.result != "ack" {
        panic!("Response result is not 'ack'");
    }

    // Connection complete.
    println!("nostr-connect is connected.");

    // Now let us test signing an event
    // Create a pre-event for the nip46 server to sign
    let pre_event = PreEvent {
        pubkey: remote_pubkey,
        created_at: Unixtime::now(),
        kind: EventKind::TextNote,
        content: "This is a test".to_owned(),
        tags: vec![],
    };

    let sign_request_id = {
        let data: [u8; 16] = rand::random();
        base64::engine::general_purpose::STANDARD.encode(data)
    };

    let sign_request_event = {
        let stringified_pre_event = serde_json::to_string(&pre_event)?;
        let request = NostrConnectRequest {
            id: sign_request_id.clone(),
            method: "sign_event".to_string(),
            params: vec![stringified_pre_event],
        };
        let stringified_request = serde_json::to_string(&request)?;
        let encrypted_content = ephemeral_signer.encrypt(
            &remote_pubkey,
            &stringified_request,
            ContentEncryptionAlgorithm::Nip04,
        )?;

        let pre_event = PreEvent {
            pubkey: ephemeral_public_key,
            created_at: Unixtime::now(),
            kind: EventKind::NostrConnect,
            content: encrypted_content,
            tags: vec![Tag::new(&["p", &remote_pubkey.as_hex_string()])],
        };

        let sign_request_event = ephemeral_signer.sign_event(pre_event).unwrap();
        sign_request_event.verify(None).unwrap();
        sign_request_event
    };

    to_probe
        .send(Command::PostEvent(sign_request_event))
        .await?;

    // Wait for an event from the remote
    let reply_event: Event;
    loop {
        match from_probe.recv().await.unwrap() {
            RelayMessage::Event(sub, e) => {
                if sub == our_sub_id {
                    reply_event = *e;
                    break;
                }
            }
            RelayMessage::Notice(_) => {
                to_probe.send(Command::Exit).await?;
                return Ok(join_handle.await?);
            }
            _ => {}
        }
    }

    // Decrypt this event
    let contents = ephemeral_signer.decrypt_event_contents(&reply_event)?;
    let ncresult: NostrConnectResult = serde_json::from_str(&contents)?;
    if ncresult.id != sign_request_id {
        panic!(
            "Response doesn't match our sign_request_id: {} != {}",
            ncresult.id, sign_request_id
        );
    }

    if !ncresult.error.is_empty() {
        println!("{:?}", ncresult);
        to_probe.send(Command::Exit).await?;
        return Ok(join_handle.await?);
    }

    let signed_event: Event = serde_json::from_str(&ncresult.result)?;
    signed_event.verify(None)?;
    println!("{}", ncresult.result);

    println!("Success.");

    to_probe.send(Command::Exit).await?;
    Ok(join_handle.await?)
}
