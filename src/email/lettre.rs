use lettre::{
    Address, AsyncSmtpTransport, Message, Tokio1Executor,
    address::AddressError,
    message::{Mailbox, MultiPart, SinglePart, header::ContentType},
    transport::smtp::authentication::Credentials,
};

use crate::email::{
    EmailAddress, SmtpConfig,
    mailer::{BoxFuture, EmailBody, MailerBackend, MailerError, OutgoingEmail},
};

pub struct LettreBackend {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl LettreBackend {
    pub fn new(config: &SmtpConfig) -> anyhow::Result<Self> {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)?
            .port(config.port)
            .credentials(Credentials::new(
                config.username.clone(),
                config.password.clone(),
            ))
            .build();

        Ok(Self {
            transport,
            from: Mailbox::new(config.from_name.clone(), config.from_address.parse()?),
        })
    }
}

impl MailerBackend for LettreBackend {
    fn send<'a>(&'a self, email: &'a OutgoingEmail) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            let address = Address::try_from(email.to.email().clone())
                .map_err(|e| MailerError::InvalidMessage(e.to_string()))?;

            let builder = Message::builder()
                .from(self.from.clone())
                .to(Mailbox::new(None, address))
                .subject(&email.subject);

            let message = match &email.body {
                EmailBody::Plaintext(text) => {
                    builder.header(ContentType::TEXT_PLAIN).body(text.clone())
                }
                EmailBody::Html(html) => builder.header(ContentType::TEXT_HTML).body(html.clone()),
                EmailBody::Multipart { text, html } => builder.multipart(
                    MultiPart::alternative()
                        .singlepart(SinglePart::plain(text.clone()))
                        .singlepart(SinglePart::html(html.clone())),
                ),
            }
            .map_err(|e| MailerError::InvalidMessage(e.to_string()))?;

            self.transport
                .send(message)
                .await
                .inspect_err(|e| tracing::error!("{:?}", e))
                .map_err(|e| MailerError::Send(e.to_string()))?;

            Ok(())
        })
    }
}

impl TryFrom<EmailAddress> for Address {
    type Error = AddressError;

    fn try_from(value: EmailAddress) -> Result<Self, Self::Error> {
        Address::new(value.local_part(), value.domain())
    }
}
