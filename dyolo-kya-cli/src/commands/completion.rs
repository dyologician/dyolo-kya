use clap::{Args, CommandFactory};
use clap_complete::{generate, Shell};

#[derive(Args)]
pub struct CompletionArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
}

pub fn run(args: CompletionArgs) -> anyhow::Result<()> {
    let mut cmd = crate::Cli::command();
    let bin_name = cmd.get_name().to_string();
    generate(args.shell, &mut cmd, bin_name, &mut std::io::stdout());
    Ok(())
}