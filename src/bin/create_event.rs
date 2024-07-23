use nostr_types::{PreEvent, Signer, Unixtime};
use std::env;
use std::io::Read;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    let _ = args.next(); // program name

    let mut s: String = String::new();
    std::io::stdin().read_to_string(&mut s)?;
    let mut pre_event: PreEvent = serde_json::from_str(&s)?;

    // Update creation stamp
    pre_event.created_at = Unixtime::now();

    let signer = nostr_probe::load_signer()?;

    let event = signer.sign_event(pre_event)?;

    println!("{}", serde_json::to_string(&event)?);

    Ok(())
}
