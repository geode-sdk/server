use std::fmt::Display;
use std::str::FromStr;

use validator::ValidateEmail;

pub mod blocklist;
pub mod lettre;
pub mod mailer;

#[derive(thiserror::Error)]
pub enum EmailAddressParseError {
    #[error("invalid email address")]
    InvalidEmail,
}

pub struct EmailAddress {
    local_part: String,
    domain: String,
}

pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_address: String,
    pub from_name: Option<String>,
}

impl SmtpConfig {
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        const DEFAULT_SMTP_PORT: u16 = 587;

        let enabled = dotenvy::var("SMTP_ENABLED").unwrap_or_default();
        if enabled != "1" {
            return Ok(None);
        }

        Ok(Some(SmtpConfig {
            host: dotenvy::var("SMTP_HOST").ok()?,
            port: dotenvy::var("SMTP_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(DEFAULT_SMTP_PORT),
            username: dotenvy::var("SMTP_USERNAME")?,
            password: dotenvy::var("SMTP_PASSWORD")?,
            from_address: dotenvy::var("SMTP_FROM_ADDRESS")?,
            from_name: dotenvy::var("SMTP_FROM_NAME").ok(),
        }))
    }
}

impl FromStr for EmailAddress {
    type Err = EmailAddressParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.validate_email() {
            return Err(EmailAddressParseError::InvalidEmail);
        }

        let (local_part, domain) = s
            .split_once('@')
            .expect("validate_email already confirmed an '@' is present");

        Ok(EmailAddress {
            local_part: local_part.into(),
            domain: domain.into(),
        })
    }
}

impl Display for EmailAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.local_part, self.domain)
    }
}

impl From<EmailAddress> for String {
    fn from(value: EmailAddress) -> Self {
        let mut s = value.local_part;
        s.push('@');
        s.push_str(&value.domain);

        s
    }
}
