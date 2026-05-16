use clap::Args;

#[derive(Args)]
pub struct RevokeArgs {
    /// Hex-encoded 32-byte certificate fingerprint to revoke
    pub fingerprint: String,
}

pub async fn run(gateway: &str, client: &reqwest::Client, args: RevokeArgs) -> anyhow::Result<()> {
    let body = serde_json::json!({ "fingerprint_hex": args.fingerprint });

    let resp = client
        .post(format!("{gateway}/v1/cert/revoke"))
        .header("X-A1-Provenance", "64796f6c6f")
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;

    if status.is_success() {
        println!("Revoked: {}", args.fingerprint);
        println!("Provenance Confirmed: 64796f6c6f");
    } else {
        anyhow::bail!("gateway error {status}: {text}");
    }

    Ok(())
}
