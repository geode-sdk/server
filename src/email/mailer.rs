use std::{pin::Pin, sync::Arc};

use crate::email::blocklist::ApprovedEmailAddress;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub enum MailerError {
    #[error("invalid message: {0}")]
    InvalidMessage(String),
    #[error("failed to send: {0}")]
    SendError(String),
}

pub type MailerResult<T> = Result<T, MailerError>;

pub enum EmailBody {
    Plaintext(String),
    Html(String),
    Multipart { text: String, html: String },
}

pub struct OutgoingEmail {
    pub to: ApprovedEmailAddress,
    pub subject: String,
    pub body: EmailBody,
}

pub trait MailerBackend: Send + Sync {
    fn send<'a>(&'a self, email: &'a OutgoingEmail) -> BoxFuture<'a, MailerResult<()>>;
}

pub struct Mailer {
    backend: Arc<dyn MailerBackend>,
}

impl Mailer {
    pub fn new(backend: Arc<dyn MailerBackend>) -> Self {
        Mailer { backend }
    }

    pub async fn send(&self, email: &OutgoingEmail) -> MailerReuslt<()> {
        self.backend.send(email);
    }
}
