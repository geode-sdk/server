use std::pin::Pin;

use actix_web::{http::header::HeaderMap, web, FromRequest, HttpRequest};
use futures::Future;
use sqlx::PgConnection;

use crate::{
    auth::{github::client::GitHubOAuthClient, oauth_client::PollingOAuthClient},
    types::{
        api::ApiError,
        models::developer::{Developer, FetchedDeveloper},
    },
    AppData,
};

pub struct Auth {
    developer: Option<FetchedDeveloper>,
}

pub enum AuthProvider {
    GitHub,
}

enum GetDeveloperResult {
    Unauthorized,
    DevNotFound,
    Found(FetchedDeveloper),
}

impl Auth {
    /**
     * Returns Ok(developer) if token was valid in request or returns ApiError::Unauthorized otherwise
     */
    pub fn developer(&self) -> Result<FetchedDeveloper, ApiError> {
        match &self.developer {
            None => Err(ApiError::Unauthorized),
            Some(d) => Ok(d.clone()),
        }
    }
}

impl FromRequest for Auth {
    type Error = ApiError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _: &mut actix_web::dev::Payload) -> Self::Future {
        let data = req.app_data::<web::Data<AppData>>().unwrap().clone();
        let headers = req.headers().clone();
        Box::pin(async move {
            let auth = get_auth_from_headers(&headers);

            if auth.is_none() {
                return Ok(Auth { developer: None });
            }

            let auth = auth.unwrap();
            let mut pool = data.db.acquire().await.or(Err(ApiError::DbAcquireError))?;

            let dev = get_developer(
                auth.0,
                auth.1,
                &data.github_client_id,
                &data.github_client_secret,
                &mut pool,
            )
            .await?;

            match dev {
                GetDeveloperResult::Found(d) => Ok(Auth { developer: Some(d) }),
                _ => Ok(Auth { developer: None }),
            }
        })
    }
}

fn get_auth_from_headers(headers: &HeaderMap) -> Option<(String, AuthProvider)> {
    let token: Option<&str> = match headers.get("Authorization") {
        Some(t) => t.to_str().ok(),
        None => None,
    };

    let provider: Option<&str> = match headers.get("X-OAuth-Provider") {
        Some(t) => t.to_str().ok(),
        None => None,
    };

    token?;

    let token = token.unwrap().chars().skip(7).collect::<String>();

    let provider = match provider.unwrap_or("github") {
        "github" => AuthProvider::GitHub,
        _ => AuthProvider::GitHub,
    };

    Some((token, provider))
}

async fn get_developer(
    token: String,
    provider: AuthProvider,
    client_id: &str,
    client_secret: &str,
    connection: &mut PgConnection,
) -> Result<GetDeveloperResult, ApiError> {
    match provider {
        AuthProvider::GitHub => {
            let client = GitHubOAuthClient::new(client_id, client_secret);
            let user = match client.get_user_profile(&token).await? {
                Some(u) => u,
                None => return Ok(GetDeveloperResult::Unauthorized),
            };

            match Developer::get_by_github_id(user.0, connection).await? {
                Some(d) => Ok(GetDeveloperResult::Found(d)),
                None => Ok(GetDeveloperResult::DevNotFound),
            }
        }
    }
}
