use nostr_types::{NostrBech32, PrivateKey};
use std::env;

fn main() {
    let mut args = env::args();
    let _ = args.next(); // program name
    let bech32 = match args.next() {
        Some(s) => s,
        None => panic!("Usage: bech32_to_any <bech32_encoded_data>"),
    };
    let bech32 = bech32.trim();

    if let Some(nb32) = NostrBech32::try_from_string(bech32) {
        match nb32 {
            NostrBech32::EventAddr(ea) => {
                println!("Event Address:");
                println!("  d={}", ea.d);
                println!(
                    "  relays={}",
                    ea.relays
                        .iter()
                        .map(|r| r.as_str().to_owned())
                        .collect::<Vec<String>>()
                        .join(", ")
                );
                println!("  kind={}", Into::<u32>::into(ea.kind));
                println!("  author={}", ea.author.as_hex_string());
            }
            NostrBech32::EventPointer(ep) => {
                println!("Event Pointer:");
                println!("  id={}", ep.id.as_hex_string());
                println!(
                    "  relays={}",
                    ep.relays
                        .iter()
                        .map(|r| r.as_str().to_owned())
                        .collect::<Vec<String>>()
                        .join(", ")
                );
                if let Some(kind) = ep.kind {
                    println!("  kind={}", Into::<u32>::into(kind));
                }
                if let Some(author) = ep.author {
                    println!("  author={}", author.as_hex_string());
                }
            }
            NostrBech32::Id(id) => {
                println!("Id: {}", id.as_hex_string());
            }
            NostrBech32::Profile(profile) => {
                println!("Profile:");
                println!("  pubkey: {}", profile.pubkey.as_hex_string());
                println!(
                    "  relays={}",
                    profile
                        .relays
                        .iter()
                        .map(|r| r.as_str().to_owned())
                        .collect::<Vec<String>>()
                        .join(", ")
                );
            }
            NostrBech32::Pubkey(pubkey) => {
                println!("Pubkey: {}", pubkey.as_hex_string());
            }
            NostrBech32::Relay(url) => {
                println!("Relay URL: {}", url.0);
            }
        }
    } else if let Ok(mut key) = PrivateKey::try_from_bech32_string(bech32) {
        println!("Private Key: {}", key.as_hex_string());
    } else {
        let (hrp, data) = bech32::decode(bech32).unwrap();
        println!("DATA.0 = {}", hrp);
        println!("DATA.1 = {}", String::from_utf8_lossy(&data));
    }
}
