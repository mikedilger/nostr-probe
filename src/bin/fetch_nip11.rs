use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use std::env;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    let _ = args.next(); // program name
    let relay_url = match args.next() {
        Some(u) => u,
        None => panic!("Usage: fetch_nip11 <RelayURL>"),
    };

    let (host, uri) = nostr_probe::url_to_host_and_uri(&relay_url);

    let scheme = match uri.scheme() {
        Some(refscheme) => match refscheme.as_str() {
            "wss" => "https",
            "ws" => "http",
            u => panic!("Unknown scheme {}", u),
        },
        None => panic!("Relay URL has no scheme."),
    };

    let client = Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Some(Duration::from_secs(60)))
        .timeout(Some(Duration::from_secs(60)))
        .connection_verbose(true)
        .build()?;
    let response = client
        .get(format!("{}://{}", scheme, host))
        .header("Host", host)
        .header("Accept", "application/nostr+json")
        .send()?;
    let json = response.text()?;
    let value: serde_json::Value = serde_json::from_str(&json)?;
    println!("{}", serde_json::to_string_pretty(&value)?);

    Ok(())
}
