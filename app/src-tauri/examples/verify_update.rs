//! Offline release check: verifies the real archive and rejects an in-memory tampered copy.
//! Usage: cargo run --example verify_update -- path/to/archive
use base64::{engine::general_purpose::STANDARD, Engine};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Expected updater archive path")?;
    let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))?;
    let public = String::from_utf8(
        STANDARD.decode(
            config["plugins"]["updater"]["pubkey"]
                .as_str()
                .ok_or("Missing public key")?,
        )?,
    )?;
    let signature = String::from_utf8(
        STANDARD.decode(std::fs::read_to_string(format!("{path}.sig"))?.trim())?,
    )?;
    let public = minisign_verify::PublicKey::decode(&public)?;
    let signature = minisign_verify::Signature::decode(&signature)?;
    let mut data = std::fs::read(path)?;
    public.verify(&data, &signature, true)?;
    if data.is_empty() {
        return Err("Empty archive".into());
    }
    data[0] ^= 1;
    if public.verify(&data, &signature, true).is_ok() {
        return Err("Tampered data passed verification".into());
    }
    println!("Signature matches embedded public key; tampered archive rejected. No installation performed.");
    Ok(())
}
