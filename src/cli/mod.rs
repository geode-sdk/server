use crate::{cli::serve::ServeArgs, config::AppData};
use clap::{Parser, Subcommand};

mod jobs;
mod serve;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run the HTTP server (default)
    Serve(serve::ServeArgs),
    /// Run an internal job
    #[command(subcommand)]
    Job(jobs::JobCommand),
}

#[tracing::instrument(skip_all, fields(command = ?args.command))]
pub async fn run(args: Args, data: AppData) -> anyhow::Result<()> {
    match args
        .command
        .unwrap_or_else(|| Commands::Serve(ServeArgs::default()))
    {
        Commands::Serve(serve_args) => serve::run(serve_args, data).await,
        Commands::Job(job) => jobs::run(job, data).await,
    }
}
