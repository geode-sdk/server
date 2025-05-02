use crate::config::AppData;
use crate::jobs;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run an internal job
    #[command(subcommand)]
    Job(JobCommand),
}

#[derive(Debug, Subcommand)]
enum JobCommand {
    /// Cleans up mod_downloads from more than 30 days ago
    CleanupDownloads,
    /// Cleans up auth and refresh tokens that are expired
    CleanupTokens,
    /// Emergency logout for a developer
    LogoutDeveloper {
        /// Username of the developer
        username: String,
    },
    /// Runs migrations
    Migrate,
    /// Intern stuff
    Fixeroo
}

pub async fn maybe_cli(data: &AppData) -> anyhow::Result<bool> {
    let cli = Args::parse();

    if let Some(c) = cli.command {
        return match c {
            Commands::Job(job) => match job {
                JobCommand::Migrate => {
                    let mut conn = data.db().acquire().await?;
                    jobs::migrate::migrate(&mut conn).await?;

                    Ok(true)
                }
                JobCommand::CleanupDownloads => {
                    let mut conn = data.db().acquire().await?;
                    jobs::cleanup_downloads::cleanup_downloads(&mut conn).await?;

                    Ok(true)
                }
                JobCommand::LogoutDeveloper { username } => {
                    let mut conn = data.db().acquire().await?;
                    jobs::logout_user::logout_user(&username, &mut conn).await?;

                    Ok(true)
                }
                JobCommand::CleanupTokens => {
                    let mut conn = data.db().acquire().await?;
                    jobs::token_cleanup::token_cleanup(&mut conn).await?;

                    Ok(true)
                },
                JobCommand::Fixeroo => {
                    let mut conn = data.db().acquire().await?;
                    jobs::fixeroo::fixeroo(data.max_download_mb(), &mut conn).await?;

                    Ok(true)
                }
            },
        };
    }
    Ok(false)
}
