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
            NostrBech32::NAddr(na) => {
                println!("Event Address:");
                println!("  d={}", na.d);
                println!(
                    "  relays={}",
                    na.relays
                        .iter()
                        .map(|r| r.as_str().to_owned())
                        .collect::<Vec<String>>()
                        .join(", ")
                );
                println!("  kind={}", Into::<u32>::into(na.kind));
                println!("  author={}", na.author.as_hex_string());
            }
            NostrBech32::NEvent(ne) => {
                println!("Event Pointer:");
                println!("  id={}", ne.id.as_hex_string());
                println!(
                    "  relays={}",
                    ne.relays
                        .iter()
                        .map(|r| r.as_str().to_owned())
                        .collect::<Vec<String>>()
                        .join(", ")
                );
                if let Some(kind) = ne.kind {
                    println!("  kind={}", Into::<u32>::into(kind));
                }
                if let Some(author) = ne.author {
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
            NostrBech32::CryptSec(cs) => {
                println!("Encrypted secret key: {}", cs);
            }
        }
    } else if let Ok(mut key) = PrivateKey::try_from_bech32_string(bech32) {
        println!("Private Key: {}", key.as_hex_string());
    } else {
        let (hrp, data) = bech32::decode(bech32).unwrap();
        println!("HRP = {}", hrp);
        println!("DATA = \"{:?}\"", String::from_utf8_lossy(&data));
    }
}
