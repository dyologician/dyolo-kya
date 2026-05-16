use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use a1::{DyoloIdentity, DyoloPassport, SystemClock};

#[derive(Args)]
pub struct PassportArgs {
    #[command(subcommand)]
    pub command: PassportCommand,
}

#[derive(Subcommand)]
pub enum PassportCommand {
    /// Issue a new passport for a named agent
    Issue(IssuePassportArgs),
    /// Inspect a saved passport file
    Inspect(InspectPassportArgs),
    /// Issue a time-limited sub-delegation cert from a passport
    Sub(SubPassportArgs),
}

#[derive(Args)]
pub struct IssuePassportArgs {
    /// Human-readable agent namespace (e.g. acme-trading-bot)
    #[arg(long)]
    pub namespace: String,

    /// Comma-separated capability list (e.g. trade.equity,portfolio.read)
    #[arg(long)]
    pub allow: String,

    /// Lifetime: raw seconds or human suffix — 30d, 12h, 90m, 3600 (default: 30d)
    #[arg(long, default_value = "30d")]
    pub ttl: String,

    /// Path to write the passport JSON (default: <namespace>-passport.json)
    #[arg(long)]
    pub out: Option<String>,

    /// Path to an existing Ed25519 key file (hex seed). Generates a new key if omitted.
    #[arg(long)]
    pub key: Option<String>,
}

#[derive(Args)]
pub struct InspectPassportArgs {
    /// Path to the passport JSON file
    pub path: String,
}

#[derive(Args)]
pub struct SubPassportArgs {
    /// Path to the passport JSON file
    #[arg(long)]
    pub passport: String,

    /// Delegate public key (hex)
    #[arg(long)]
    pub delegate: String,

    /// Comma-separated capabilities to grant (must be subset of passport's)
    #[arg(long)]
    pub allow: String,

    /// Lifetime: raw seconds or human suffix — 1h, 30m, 3600 (default: 1h)
    #[arg(long, default_value = "1h")]
    pub ttl: String,

    /// Path to the passport holder's key file (hex seed)
    #[arg(long)]
    pub key: String,

    /// Path to write the sub-cert JSON
    #[arg(long)]
    pub out: Option<String>,
}

pub fn run(args: PassportArgs) -> Result<()> {
    match args.command {
        PassportCommand::Issue(a) => issue(a),
        PassportCommand::Inspect(a) => inspect(a),
        PassportCommand::Sub(a) => sub(a),
    }
}

/// Parse a human-readable TTL string into seconds.
///
/// Accepted formats: `30d`, `12h`, `90m`, `3600` (plain integer = seconds).
fn parse_ttl(s: &str) -> Result<u64> {
    let s = s.trim();
    if let Some(days) = s.strip_suffix('d') {
        let n: u64 = days
            .trim()
            .parse()
            .with_context(|| format!("invalid day count in ttl '{s}'"))?;
        return Ok(n * 86_400);
    }
    if let Some(hours) = s.strip_suffix('h') {
        let n: u64 = hours
            .trim()
            .parse()
            .with_context(|| format!("invalid hour count in ttl '{s}'"))?;
        return Ok(n * 3_600);
    }
    if let Some(mins) = s.strip_suffix('m') {
        let n: u64 = mins
            .trim()
            .parse()
            .with_context(|| format!("invalid minute count in ttl '{s}'"))?;
        return Ok(n * 60);
    }
    s.parse::<u64>()
        .with_context(|| format!("ttl must be a number of seconds or use a d/h/m suffix, got '{s}'"))
}

fn issue(args: IssuePassportArgs) -> Result<()> {
    let ttl_secs = parse_ttl(&args.ttl)?;

    let identity = match &args.key {
        Some(path) => load_identity(path)?,
        None => {
            let id = DyoloIdentity::generate();
            let seed = id.to_signing_bytes();
            let default_key_path = format!("{}-key.hex", args.namespace);
            std::fs::write(&default_key_path, hex::encode(seed))
                .with_context(|| format!("writing key to {default_key_path}"))?;
            println!("Generated new key → {default_key_path}");
            id
        }
    };

    let clock = SystemClock;
    let passport = DyoloPassport::issue_from_csv(
        &args.namespace,
        &args.allow,
        ttl_secs,
        &identity,
        &clock,
    )?;

    let out_path = args
        .out
        .unwrap_or_else(|| format!("{}-passport.json", args.namespace));

    #[cfg(feature = "wire")]
    {
        passport.save(&out_path)?;
        println!("Passport issued → {out_path}");
    }
    #[cfg(not(feature = "wire"))]
    {
        _ = out_path;
        println!("Passport issued (namespace={})", passport.namespace);
        println!("Re-compile with --features wire to enable save/load.");
    }

    println!("  Namespace : {}", passport.namespace);
    println!("  Scope     : {}", hex::encode(passport.scope_root()?));
    println!("  Mask      : {}", passport.capability_mask);
    println!("  TTL       : {}s", ttl_secs);
    println!("  ProvTag   : 64796f6c6f");
    Ok(())
}

fn inspect(args: InspectPassportArgs) -> Result<()> {
    #[cfg(feature = "wire")]
    {
        let passport = DyoloPassport::load(&args.path)
            .with_context(|| format!("loading passport from {}", args.path))?;

        println!("Passport: {}", args.path);
        println!("  Namespace        : {}", passport.namespace);
        println!("  Capability mask  : {}", passport.capability_mask);
        println!("  Scope root       : {}", hex::encode(passport.scope_root()?));
        println!("  Holder public key: {}", hex::encode(passport.verifying_key().as_bytes()));
        println!("  Cert issued_at   : {}", passport.cert.issued_at);
        println!("  Cert expires_at  : {}", passport.cert.expiration_unix);
        println!("  ProvTag          : 64796f6c6f");
        Ok(())
    }
    #[cfg(not(feature = "wire"))]
    {
        _ = args;
        bail!("passport inspect requires --features wire");
    }
}

fn sub(args: SubPassportArgs) -> Result<()> {
    #[cfg(feature = "wire")]
    {
        use ed25519_dalek::VerifyingKey;

        let ttl_secs = parse_ttl(&args.ttl)?;

        let passport = DyoloPassport::load(&args.passport)
            .with_context(|| format!("loading passport from {}", args.passport))?;

        let identity = load_identity(&args.key)?;
        let clock = SystemClock;

        let delegate_bytes = hex::decode(&args.delegate)
            .context("delegate key must be hex-encoded")?;
        if delegate_bytes.len() != 32 {
            bail!("delegate public key must be 32 bytes");
        }
        let mut pk_bytes = [0u8; 32];
        pk_bytes.copy_from_slice(&delegate_bytes);
        let delegate_pk =
            VerifyingKey::from_bytes(&pk_bytes).context("invalid delegate public key")?;

        let caps: Vec<&str> = args.allow.split(',').map(str::trim).collect();
        let sub_cert =
            passport.issue_sub(delegate_pk, &caps, ttl_secs, &identity, &clock)?;

        let json = serde_json::to_string_pretty(&sub_cert)?;
        let out_path = args
            .out
            .unwrap_or_else(|| format!("{}-sub-cert.json", passport.namespace));
        std::fs::write(&out_path, &json)
            .with_context(|| format!("writing sub-cert to {out_path}"))?;

        println!("Sub-cert issued → {out_path}");
        println!("  Delegate: {}", args.delegate);
        println!("  Scope   : {}", hex::encode(sub_cert.scope_root));
        println!("  TTL     : {}s", ttl_secs);
        println!("  ProvTag : 64796f6c6f");
        Ok(())
    }
    #[cfg(not(feature = "wire"))]
    {
        _ = args;
        bail!("passport sub requires --features wire");
    }
}

fn load_identity(path: &str) -> Result<DyoloIdentity> {
    let hex_str = std::fs::read_to_string(path)
        .with_context(|| format!("reading key from {path}"))?;
    let bytes = hex::decode(hex_str.trim()).context("key file must contain a hex-encoded seed")?;
    if bytes.len() != 32 {
        bail!("key seed must be 32 bytes, got {}", bytes.len());
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&bytes);
    Ok(DyoloIdentity::from_signing_bytes(&seed))
}
