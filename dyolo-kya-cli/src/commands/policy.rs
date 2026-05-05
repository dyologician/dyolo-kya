use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct PolicyArgs {
    /// Path to the YAML or JSON policy file
    #[arg(long, short)]
    pub file: PathBuf,
}

pub async fn run(gateway: &str, client: &reqwest::Client, args: PolicyArgs) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(&args.file)?;
    
    // Parse the policy document (assumes JSON format here to avoid adding a serde_yaml dep to the CLI)
    // In a fully featured CLI, you would map `serde_yaml::from_str` here.
    let doc: serde_json::Value = serde_json::from_str(&content)
        .or_else(|_| -> anyhow::Result<serde_json::Value> {
            anyhow::bail!("Policy must be valid JSON (YAML support requires serde_yaml dependency)")
        })?;
    
    let certs = doc.get("certs")
        .and_then(|c| c.as_array())
        .ok_or_else(|| anyhow::anyhow!("Policy must contain a 'certs' array"))?;
    
    let body = serde_json::json!({ "requests": certs });

    let resp = client.post(format!("{gateway}/v1/cert/issue-batch"))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;

    if status.is_success() {
        let parsed: serde_json::Value = serde_json::from_str(&text)?;
        println!("Successfully applied policy.");
        println!("Total certs issued: {}", parsed["total"]);
    } else {
        anyhow::bail!("gateway error {status}: {text}");
    }

    Ok(())
}