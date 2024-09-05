use sqlx::PgConnection;
use uuid::Uuid;

use crate::types::{api::ApiError, models::oauth_attempt::OAuthAttempt};

/// Done contains the `OAuth` provider auth token
pub enum PollResult {
    Done((String, String)),
    Pending,
    Expired,
    TooFast,
}

pub trait PollingOAuthClient {
    fn new(client_id: &str, client_secret: &str) -> Self;

    /// Returns a tuple with URL needed to auth and the stored attempt
    async fn start_auth(
        &self,
        callback_url: &str,
        connection: &mut PgConnection,
    ) -> Result<(String, OAuthAttempt), ApiError>;
    async fn poll(
        &self,
        secret: Uuid,
        connection: &mut PgConnection,
    ) -> Result<PollResult, ApiError>;

    async fn exchange_tokens(&self, code: String) -> Result<Option<(String, String)>, ApiError>;

    /// Returns ID and username
    async fn get_user_profile(&self, oauth_token: &str) -> Result<Option<(i64, String)>, ApiError>;
}
