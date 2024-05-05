use nostr_types::PublicKey;
use std::env;

fn main() {
    let mut args = env::args();
    let _ = args.next(); // program name
    let hex = match args.next() {
        Some(u) => u,
        None => panic!("Usage: pubkey_to_bech32 <hex>"),
    };

    let public_key = PublicKey::try_from_hex_string(&hex, true).unwrap();
    let bech32 = public_key.as_bech32_string();
    println!("{}", bech32);
}
