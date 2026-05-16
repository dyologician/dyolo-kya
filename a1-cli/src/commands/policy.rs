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

    // Enterprise Policy-as-Code requires native YAML parsing.
    // Fallback to JSON if parsing fails, ensuring complete CI/CD pipeline compatibility.
    let doc: serde_json::Value = if args.file.extension().and_then(|s| s.to_str()) == Some("yaml") || 
                                    args.file.extension().and_then(|s| s.to_str()) == Some("yml") {
        let yaml_val: serde_yaml::Value = serde_yaml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Invalid YAML policy format: {}", e))?;
        // Convert to JSON Value for universal gateway transmission
        serde_json::to_value(yaml_val).map_err(|e| anyhow::anyhow!("YAML to JSON conversion failed: {}", e))?
    } else {
        serde_json::from_str(&content)
            .or_else(|_| -> anyhow::Result<serde_json::Value> {
                // Attempt YAML parsing as a fallback if JSON fails, before completely bailing
                let yaml_fallback: serde_yaml::Value = serde_yaml::from_str(&content)
                    .map_err(|_| anyhow::anyhow!("Policy must be valid JSON or YAML"))?;
                serde_json::to_value(yaml_fallback).map_err(|e| anyhow::anyhow!("YAML fallback conversion failed: {}", e))
            })?
    };

    let certs = doc
        .get("certs")
        .and_then(|c| c.as_array())
        .ok_or_else(|| anyhow::anyhow!("Policy must contain a 'certs' array"))?;

    let body = serde_json::json!({ "requests": certs });

    let resp = client
        .post(format!("{gateway}/v1/cert/issue-batch"))
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
