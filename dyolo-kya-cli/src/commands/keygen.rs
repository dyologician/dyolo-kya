use dyolo_kya::DyoloIdentity;

pub fn run() -> anyhow::Result<()> {
    let identity = DyoloIdentity::generate();
    let sk_hex   = hex::encode(identity.to_signing_bytes());
    let vk_hex   = hex::encode(identity.verifying_key().as_bytes());

    println!("signing_key_hex  : {sk_hex}");
    println!("verifying_key_hex: {vk_hex}");
    println!();
    println!("Set DYOLO_SIGNING_KEY_HEX={sk_hex} on the gateway.");
    println!("Use verifying_key_hex as the principal_pk when building chains.");

    Ok(())
}
