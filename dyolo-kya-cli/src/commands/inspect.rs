use clap::Args;

#[derive(Args)]
pub struct InspectArgs {
    /// Hex-encoded 32-byte certificate fingerprint to inspect
    pub fingerprint: String,
}

pub async fn run(gateway: &str, client: &reqwest::Client, args: InspectArgs) -> anyhow::Result<()> {
    let resp = client.get(format!("{gateway}/v1/cert/{}", args.fingerprint))
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;

    if status.is_success() {
        let parsed: serde_json::Value = serde_json::from_str(&text)?;
        let revoked = parsed["revoked"].as_bool().unwrap_or(false);
        println!("fingerprint: {}", parsed["fingerprint"].as_str().unwrap_or("?"));
        println!("revoked    : {revoked}");
    } else {
        anyhow::bail!("gateway error {status}: {text}");
    }

    Ok(())
}
