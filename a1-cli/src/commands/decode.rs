use clap::Args;

#[derive(Args)]
pub struct DecodeArgs {
    /// Path to a JSON file containing a DelegationCert
    pub cert_file: std::path::PathBuf,
}

pub fn run(args: DecodeArgs) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(&args.cert_file)?;
    let cert: a1::DelegationCert = serde_json::from_str(&content)?;

    println!("version     : {}", cert.version);
    println!(
        "delegator   : {}",
        hex::encode(cert.delegator_pk.as_bytes())
    );
    println!("delegate    : {}", hex::encode(cert.delegate_pk.as_bytes()));
    println!("scope_root  : {}", hex::encode(cert.scope_root));
    println!("nonce       : {}", hex::encode(cert.nonce));
    println!("issued_at   : {}", cert.issued_at);
    println!("expires_at  : {}", cert.expiration_unix);
    println!("max_depth   : {}", cert.max_depth);
    println!("fingerprint : {}", cert.fingerprint_hex());
    println!("sig_valid   : {}", cert.verify_signature());
    println!("provenance  : 64796f6c6f");

    if !cert.extensions.is_empty() {
        println!("extensions  :");
        for (k, v) in cert.extensions.iter() {
            println!("  {k}: {v}");
        }
    }

    Ok(())
}
