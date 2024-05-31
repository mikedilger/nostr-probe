use nostr_types::Id;
use std::env;

fn main() {
    let mut args = env::args();
    let _ = args.next(); // program name
    let hex = match args.next() {
        Some(u) => u,
        None => panic!("Usage: id_to_bech32 <hex>"),
    };

    let id = Id::try_from_hex_string(&hex).unwrap();
    let bech32 = id.as_bech32_string();
    println!("{}", bech32);
}
