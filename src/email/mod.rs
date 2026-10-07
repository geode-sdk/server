use std::fmt::{Display, Write};
use std::str::FromStr;

use anyhow::anyhow;
use idna::AsciiDenyList;
use idna::uts46::Uts46;
use validator::ValidateEmail;

use crate::email::blocklist::BlocklistError;

pub mod blocklist;
pub mod lettre;
pub mod mailer;
pub mod partials;

#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    #[error("email address parse error: {0}")]
    ParseError(#[from] EmailAddressParseError),
    #[error("email is not allowed: {0}")]
    BlocklistError(#[from] BlocklistError),
}

#[derive(Debug, thiserror::Error)]
pub enum EmailAddressParseError {
    #[error("invalid email address")]
    InvalidEmail,
}

#[derive(Clone, Debug)]
pub struct EmailAddress {
    local_part: String,
    domain: String,
}

#[derive(PartialEq)]
pub enum SmtpSecurity {
    None,
    StartTls,
    Wrapper,
}

impl std::str::FromStr for SmtpSecurity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lowercase = s.to_lowercase();

        match lowercase.as_str() {
            "none" => Ok(SmtpSecurity::None),
            "starttls" => Ok(SmtpSecurity::StartTls),
            "wrapper" => Ok(SmtpSecurity::Wrapper),
            other => Err(format!("invalid SMTP_TLS value: {other}")),
        }
    }
}

pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub security: SmtpSecurity,
    pub username: Option<String>,
    pub password: Option<String>,
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
            host: dotenvy::var("SMTP_HOST")?,
            port: dotenvy::var("SMTP_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(DEFAULT_SMTP_PORT),
            security: dotenvy::var("SMTP_SECURITY")
                .unwrap_or_else(|_| "starttls".into())
                .parse()
                .map_err(|e| anyhow!("{}", e))?,
            username: dotenvy::var("SMTP_USERNAME").ok(),
            password: dotenvy::var("SMTP_PASSWORD").ok(),
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

        let domain = idna::domain_to_ascii_cow(domain.as_bytes(), AsciiDenyList::STD3)
            .map_err(|_| EmailAddressParseError::InvalidEmail)?
            .trim_end_matches('.')
            .to_owned();

        Ok(EmailAddress {
            local_part: local_part.into(),
            domain,
        })
    }
}

impl EmailAddress {
    pub fn local_part(&self) -> &str {
        &self.local_part
    }

    pub fn local_part_masked(&self) -> &str {
        let shown = if self.local_part.chars().count() <= 2 {
            1
        } else {
            2
        };

        let end = self.local_part.char_indices().nth(shown).map_or(self.local_part.len(), |(i, _)| i);

        &self.local_part[..end]
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub fn to_masked_string(&self) -> String {
        let (display, result) = Uts46::new().to_user_interface(
            self.domain.as_bytes(),
            AsciiDenyList::STD3,
            idna::uts46::Hyphens::Allow,
            |_label, _tld, _is_bidi| true,
        );

        let domain: &str = if result.is_ok() { display.as_ref() } else { &self.domain };
        let local_part_masked = self.local_part_masked();

        let mut ret = format!("{}***@{}", local_part_masked, domain);

        if self.domain != domain {
            write!(ret, " ({}***@{})", local_part_masked, self.domain).ok();
        }

        ret
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
