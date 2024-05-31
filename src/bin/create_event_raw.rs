use nostr_types::{Event, Id, Signer, Tag};
use secp256k1::hashes::Hash;
use serde_json::Value;
use std::env;
use std::io::Read;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    let _ = args.next(); // program name

    let mut s: String = String::new();
    std::io::stdin().read_to_string(&mut s)?;
    println!("INPUT: {}", s);

    let value: Value = serde_json::from_str(&s)?;
    let obj = value.as_object().unwrap();
    let created_at = format!("{}", obj.get("created_at").unwrap());
    let kind = format!("{}", obj.get("kind").unwrap());
    let tags: Vec<Tag> = serde_json::from_value(obj.get("tags").unwrap().clone())?;
    let content = obj.get("content").unwrap().as_str().unwrap().to_owned();
    let signer = nostr_probe::load_signer()?;

    // Event pubkey must match our signer

    let serial_for_sig = format!(
        "[0,\"{}\",{},{},{},\"{}\"]",
        signer.public_key().as_hex_string(),
        created_at,
        kind,
        serde_json::to_string(&tags)?,
        &content,
    );
    println!("SIGN: {}", serial_for_sig);
    let hash = secp256k1::hashes::sha256::Hash::hash(serial_for_sig.as_bytes());
    let id: [u8; 32] = hash.to_byte_array();
    let id = Id(id);
    let signature = signer.sign_id(id)?;

    let output = format!(
        r##"{{"id":"{}","pubkey":"{}","created_at":{},"kind":{},"tags":{},"content":"{}","sig":"{}"}}"##,
        id.as_hex_string(),
        signer.public_key().as_hex_string(),
        created_at,
        kind,
        serde_json::to_string(&tags)?,
        content,
        signature.as_hex_string(),
    );
    println!("EVENT: {}", output);

    let event: Event = serde_json::from_str(&output)?;
    event.verify(None)?;

    println!("Event verified.");
    Ok(())
}
