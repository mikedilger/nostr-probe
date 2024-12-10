use clap::Parser;
use nostr_types::{EventKind, Metadata, PreEvent, Signer, Tag, Unixtime};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Metadata
    #[arg(short, long)]
    metadata: Option<String>,

    // Kinds
    #[arg(short, long)]
    kind: Vec<u32>,

    // Identifier
    #[arg(short)]
    d: String,

    // Url
    #[arg(short, long)]
    url: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let signer = nostr_probe::load_signer()?;

    let mut tags: Vec<Tag> = Vec::new();

    // Identifier
    tags.push(Tag::new(&["d", &args.d]));
    if args.kind.is_empty() {
        return Err(Box::new(std::io::Error::other(
            "You must specify at least one kind",
        )));
    }

    // Kinds
    for kind in args.kind {
        tags.push(Tag::new(&["k", &format!("{kind}")]));
    }

    // Url
    if !args.url.contains("<bech32>") {
        return Err(Box::new(std::io::Error::other("URL missing <bech32> part")));
    }
    tags.push(Tag::new(&["web", &args.url]));

    let content = if let Some(metadata) = args.metadata {
        // Validate it parses
        let _ = serde_json::from_str::<Metadata>(&metadata)?;
        metadata // use the unparsed string
    } else {
        "".to_owned()
    };

    let pre_event: PreEvent = PreEvent {
        pubkey: signer.public_key(),
        created_at: Unixtime::now(),
        kind: EventKind::HandlerInformation,
        tags,
        content,
    };

    let event = signer.sign_event(pre_event)?;
    println!("{}", serde_json::to_string(&event)?);

    Ok(())
}
