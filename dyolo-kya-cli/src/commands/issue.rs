use clap::Args;

#[derive(Args)]
pub struct IssueArgs {
    /// Hex-encoded Ed25519 public key of the delegate
    #[arg(long)]
    pub delegate_pk: String,

    /// Intent names to authorize (repeat for multiple: --intent trade.equity --intent query.portfolio)
    #[arg(long = "intent", required = true)]
    pub intents: Vec<String>,

    /// Certificate lifetime in seconds
    #[arg(long, default_value_t = 3600)]
    pub ttl: u64,

    /// Maximum further delegation hops
    #[arg(long, default_value_t = 16)]
    pub max_depth: u8,

    /// Extension fields as key=value pairs (repeat for multiple: --ext cost_center=ai-ops)
    #[arg(long = "ext")]
    pub extensions: Vec<String>,

    /// AWS KMS Key ID for remote signing (bypasses gateway signing)
    #[arg(long)]
    pub kms_key_id: Option<String>,
}

pub async fn run(gateway: &str, client: &reqwest::Client, args: IssueArgs) -> anyhow::Result<()> {
    let intents: Vec<serde_json::Value> = args.intents.iter()
        .map(|name| serde_json::json!({ "name": name }))
        .collect();

    let mut extensions = serde_json::Map::new();
    for ext in &args.extensions {
        let (k, v) = ext.split_once('=')
            .ok_or_else(|| anyhow::anyhow!("--ext must be key=value, got: {ext}"))?;
        extensions.insert(k.to_string(), serde_json::Value::String(v.to_string()));
    }

    let body = serde_json::json!({
        "delegate_pk_hex": args.delegate_pk,
        "intents": intents,
        "ttl_seconds": args.ttl,
        "max_depth": args.max_depth,
        "extensions": extensions,
        "kms_key_id": args.kms_key_id,
    });

    let resp = client.post(format!("{gateway}/v1/cert/issue"))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;

    if status.is_success() {
        let parsed: serde_json::Value = serde_json::from_str(&text)?;
        println!("fingerprint: {}", parsed["fingerprint_hex"].as_str().unwrap_or("?"));
        println!("scope_root:  {}", parsed["scope_root_hex"].as_str().unwrap_or("?"));
        println!();
        println!("{}", serde_json::to_string_pretty(&parsed["cert"])?);
    } else {
        anyhow::bail!("gateway error {status}: {text}");
    }

    Ok(())
}
