use clap::Args;

#[derive(Args)]
pub struct RevokeBatchArgs {
    /// Hex-encoded 32-byte certificate fingerprints to revoke
    #[arg(required = true)]
    pub fingerprints: Vec<String>,
}

pub async fn run(
    gateway: &str,
    client: &reqwest::Client,
    args: RevokeBatchArgs,
) -> anyhow::Result<()> {
    let body = serde_json::json!({ "fingerprints": args.fingerprints });

    let resp = client
        .post(format!("{gateway}/v1/cert/revoke-batch"))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;

    if status.is_success() {
        let parsed: serde_json::Value = serde_json::from_str(&text)?;
        println!("Revoked count: {}", parsed["revoked_count"]);
        if let Some(failed) = parsed.get("failed").and_then(|f| f.as_array()) {
            if !failed.is_empty() {
                println!("Failed to revoke: {:?}", failed);
            }
        }
    } else {
        anyhow::bail!("gateway error {status}: {text}");
    }

    Ok(())
}
