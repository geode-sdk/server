use crate::{jobs, AppData};
use anyhow::anyhow;
use clap::{Parser, Subcommand};
use sqlx::Acquire;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run an internal job
    #[command(subcommand)]
    Job(JobCommand),
}

#[derive(Debug, Subcommand)]
pub enum JobCommand {
    /// Cleans up mod_downloads from more than 30 days ago
    #[command(subcommand)]
    CleanupDownloads,
    /// Runs migrations
    Migrate,
}

pub async fn maybe_cli(data: &AppData) -> anyhow::Result<bool> {
    let cli = Args::parse();

    if let Some(c) = cli.command {
        return match c {
            Commands::Job(job) => match job {
                JobCommand::Migrate => {
                    let mut conn = data.db.acquire().await?;
                    jobs::migrate::migrate(&mut conn).await?;

                    Ok(true)
                },
                JobCommand::CleanupDownloads => {
                    let mut conn = data.db.acquire().await?;
                    jobs::cleanup_downloads::cleanup_downloads(&mut *conn).await?;

                    Ok(true)
                }
            },
        };
    }
    Ok(false)
}
