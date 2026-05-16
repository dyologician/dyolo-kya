use a1::DyoloIdentity;
use clap::Args;

#[derive(Args)]
pub struct KeygenArgs {
    /// Write the signing key seed to this file (hex). Useful for scripting.
    #[arg(long)]
    pub out: Option<String>,
}

pub fn run(args: KeygenArgs) -> anyhow::Result<()> {
    let identity = DyoloIdentity::generate();
    let sk_hex = hex::encode(identity.to_signing_bytes());
    let vk_hex = hex::encode(identity.verifying_key().as_bytes());

    println!("signing_key_hex  : {sk_hex}");
    println!("verifying_key_hex: {vk_hex}");
    println!("provenance_tag   : 64796f6c6f");
    println!();
    println!("Set A1_SIGNING_KEY_HEX={sk_hex} on the gateway.");
    println!("Use verifying_key_hex as the principal_pk when building chains.");

    if let Some(path) = &args.out {
        std::fs::write(path, &sk_hex)
            .map_err(|e| anyhow::anyhow!("failed to write key to {path}: {e}"))?;
        println!("Signing key written to: {path}");
    }

    Ok(())
}
