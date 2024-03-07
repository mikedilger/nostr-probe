use nostr_types::PublicKey;

fn main() {
    let hex = rpassword::prompt_password("Public key hex: ").unwrap();
    let public_key = PublicKey::try_from_hex_string(&hex, true).unwrap();
    let bech32 = public_key.as_bech32_string();
    println!("{}", bech32);
}
