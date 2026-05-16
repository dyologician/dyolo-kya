use clap::Args;

#[derive(Args)]
pub struct MigrateArgs {
    /// PostgreSQL connection URL
    ///
    /// Example: postgres://user:pass@localhost/mydb
    #[arg(long, env = "DATABASE_URL")]
    database_url: Option<String>,

    /// Print the migration DDL to stdout without running it
    #[arg(long)]
    print: bool,
}

pub async fn run(args: MigrateArgs) -> anyhow::Result<()> {
    use a1_pg::MIGRATION_DDL;

    if args.print {
        println!("{}", MIGRATION_DDL.trim());
        return Ok(());
    }

    let url = args.database_url.ok_or_else(|| {
        anyhow::anyhow!(
            "Provide --database-url or set DATABASE_URL.\n\
             Use --print to emit the DDL for manual application."
        )
    })?;

    let pool = sqlx::PgPool::connect(&url).await?;
    a1_pg::PgRevocationStore::run_migration(&pool).await?;
    pool.close().await;

    eprintln!("a1: migration applied successfully.");
    Ok(())
}
