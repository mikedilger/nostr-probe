use nostr_types::{PreEvent, PublicKey, Signer, Unixtime};
use std::env;
use std::io::Read;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    let _ = args.next(); // program name

    let pubkey = match args.next() {
        Some(key) => match PublicKey::try_from_bech32_string(&key, true) {
            Ok(key) => key,
            Err(_) => match PublicKey::try_from_hex_string(&key, true) {
                Ok(key) => key,
                Err(_) => panic!("Could not parse public key"),
            },
        },
        None => panic!("Usage: create_giftwrap <RecipientPubkey> < JSON_PRE_EVENT"),
    };

    let mut s: String = String::new();
    std::io::stdin().read_to_string(&mut s)?;
    let mut pre_event: PreEvent = serde_json::from_str(&s)?;

    // Update creation stamp
    pre_event.created_at = Unixtime::now();

    let signer = nostr_probe::load_signer()?;

    let event = signer.giftwrap(pre_event, pubkey)?;

    println!("{}", serde_json::to_string(&event)?);

    Ok(())
}
