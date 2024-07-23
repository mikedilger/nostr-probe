use nostr_types::{EventKind, NAddr, NostrUrl, PublicKey, UncheckedUrl};
use std::env;

fn main() {
    let mut args = env::args();
    let _ = args.next(); // program name

    let usage = |msg| -> ! {
        panic!("{}\nUsage: form_event_addr <kind_number> <author_pubkeyhex> <d-identifier> [<relay_url> ...]", msg);
    };

    let kind: EventKind = match args.next() {
        Some(k) => {
            let u = k
                .parse::<u32>()
                .unwrap_or_else(|_| usage("Kind not parsed"));
            u.into()
        }
        None => usage("Kind missing"),
    };

    let author = match args.next() {
        Some(hex) => PublicKey::try_from_hex_string(&hex, true)
            .unwrap_or_else(|_| usage("Public key not parsed")),
        None => usage("Public key missing"),
    };

    let d = match args.next() {
        Some(d) => d,
        None => usage("d-identifier missing"),
    };

    let mut relays: Vec<UncheckedUrl> = Vec::new();
    for r in args {
        relays.push(UncheckedUrl(r));
    }

    let na = NAddr {
        d,
        author,
        kind,
        relays,
    };

    let nurl: NostrUrl = na.into();

    println!("{}", nurl);
}
