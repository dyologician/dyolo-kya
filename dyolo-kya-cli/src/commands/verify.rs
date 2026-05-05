use clap::Args;

#[derive(Args)]
pub struct VerifyArgs {
    /// Path to a JSON file containing a VerifiedToken
    pub token_file: std::path::PathBuf,
}

pub async fn run(gateway: &str, client: &reqwest::Client, args: VerifyArgs) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(&args.token_file)?;
    let token: serde_json::Value = serde_json::from_str(&content)?;
    let body = serde_json::json!({ "token": token });

    let resp = client.post(format!("{gateway}/v1/token/verify"))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;
    let parsed: serde_json::Value = serde_json::from_str(&text)?;

    if status.is_success() {
        println!("valid              : {}", parsed["valid"]);
        println!("chain_depth        : {}", parsed["chain_depth"]);
        println!("chain_fingerprint  : {}", parsed["chain_fingerprint"].as_str().unwrap_or("?"));
        println!("verified_at_unix   : {}", parsed["verified_at_unix"]);
    } else {
        println!("INVALID — {}", parsed["error"].as_str().unwrap_or("unknown error"));
        std::process::exit(1);
    }

    Ok(())
}
