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
    /// Caches download counts for all mods currently stored
    #[command(subcommand)]
    CacheDownloads,
    /// Runs migrations
    #[command(subcommand)]
    Migrate,
}

pub async fn maybe_cli(data: &AppData) -> anyhow::Result<bool> {
    let cli = Args::parse();

    if let Some(c) = cli.command {
        return match c {
            Commands::Job(job) => match job {
                JobCommand::CacheDownloads => {
                    let mut conn = data.db.acquire().await?;
                    let mut transaction = conn.begin().await?;

                    jobs::download_cache::start(&mut transaction)
                        .await
                        .map_err(|e| anyhow!("Failed to update download cache {}", e))?;

                    transaction.commit().await?;
                    Ok(true)
                }
                JobCommand::Migrate => {
                    let mut conn = data.db.acquire().await?;
                    jobs::migrate::migrate(&mut conn).await?;

                    Ok(true)
                }
            },
        };
    }
    Ok(false)
}
