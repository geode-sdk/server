use crate::{config::AppData, database, jobs};

#[derive(Debug, clap::Subcommand)]
pub enum JobCommand {
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
}

pub async fn run(job: JobCommand, data: AppData) -> anyhow::Result<()> {
    let mut conn = data.db().acquire().await?;

    match job {
        JobCommand::Migrate => {
            database::migrate(&mut conn).await?;
        }
        JobCommand::CleanupDownloads => {
            jobs::cleanup_downloads::cleanup_downloads(&mut conn).await?;
        }
        JobCommand::LogoutDeveloper { username } => {
            jobs::logout_user::logout_user(&username, &mut conn).await?;
        }
        JobCommand::CleanupTokens => {
            jobs::token_cleanup::token_cleanup(&mut conn).await?;
        }
    }

    Ok(())
}
