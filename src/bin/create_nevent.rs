use nostr_types::{Id, NEvent, NostrBech32, UncheckedUrl};
use std::env;

fn main() {
    let mut args = env::args();
    let _ = args.next(); // program name

    let hex = match args.next() {
        Some(u) => u,
        None => panic!("Usage: create_nevent <idhex> <relay_url> [<relay_url>]"),
    };
    let id = Id::try_from_hex_string(&hex).unwrap();

    let mut relays: Vec<UncheckedUrl> = Vec::new();
    while let Some(urlstr) = args.next() {
        let url = UncheckedUrl::from_str(&urlstr);
        relays.push(url);
    }

    let ep = NEvent {
        id,
        relays,
        kind: None,
        author: None,
    };

    let nurl = NostrBech32::NEvent(ep);

    println!("{}", nurl);
}
