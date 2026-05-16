mod commands;

use clap::{Parser, Subcommand};

/// A1 — One Identity. Full Provenance.
///
/// Issue delegation certificates, revoke them, inspect chains, and verify
/// authorization decisions — without writing a line of Rust.
#[derive(Parser)]
#[command(name = "a1", version, about, long_about = None)]
struct Cli {
    /// Gateway base URL
    #[arg(
        long,
        env = "A1_GATEWAY_URL",
        default_value = "http://localhost:8080",
        global = true
    )]
    gateway: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a new Ed25519 keypair
    Keygen(commands::keygen::KeygenArgs),
    /// Issue a delegation certificate
    Issue(commands::issue::IssueArgs),
    /// Apply a YAML delegation policy to issue certs
    Policy(commands::policy::PolicyArgs),
    /// Revoke a certificate by fingerprint
    Revoke(commands::revoke::RevokeArgs),
    /// Revoke multiple certificates by fingerprint
    RevokeBatch(commands::revoke_batch::RevokeBatchArgs),
    /// Check a certificate's revocation status
    Inspect(commands::inspect::InspectArgs),
    /// Verify a VerifiedToken (HMAC receipt) from the gateway
    Verify(commands::verify::VerifyArgs),
    /// Decode a DelegationCert JSON file and print its fields
    Decode(commands::decode::DecodeArgs),
    /// Generate shell completions
    Completion(commands::completion::CompletionArgs),
    /// Run the PostgreSQL schema migration for a1-pg
    Migrate(commands::migrate::MigrateArgs),
    /// Manage agent passports (issue, inspect, delegate)
    Passport(commands::passport::PassportArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let client = reqwest::Client::new();

    match cli.command {
        Command::Keygen(args) => commands::keygen::run(args),
        Command::Issue(args) => commands::issue::run(&cli.gateway, &client, args).await,
        Command::Policy(args) => commands::policy::run(&cli.gateway, &client, args).await,
        Command::Revoke(args) => commands::revoke::run(&cli.gateway, &client, args).await,
        Command::RevokeBatch(args) => {
            commands::revoke_batch::run(&cli.gateway, &client, args).await
        }
        Command::Inspect(args) => commands::inspect::run(&cli.gateway, &client, args).await,
        Command::Verify(args) => commands::verify::run(&cli.gateway, &client, args).await,
        Command::Decode(args) => commands::decode::run(args),
        Command::Completion(args) => commands::completion::run(args),
        Command::Migrate(args) => commands::migrate::run(args).await,
        Command::Passport(args) => commands::passport::run(args),
    }
}
