use maud::{Markup, html};

use crate::email::partials;

pub fn setup_email(endpoint: &str) -> Markup {
    html! {
        (partials::salute())
        p {
            "Someone (hopefully you) requested to setup email sign-in for your Geode SDK developer account using this address."
        }
        p {
            "If you didn't request this, you can safely ignore this email. No changes will be made to your account."
        }
        p {
            "Visit"
            a href=(endpoint) { (endpoint) }
            "to confirm this email address"
        }
        p {
            "This link expires in 30 minutes."
        }
        (partials::footer())
    }
}

pub fn start_request(username: &str, endpoint: &str) -> Markup {
    html! {
        (partials::salute())
        p {
            "Someone (hopefully you) requested to change the email address for the Geode SDK developer account " (username) " to this one."
        }
        p {
            "If you didn't request this, you can safely ignore this email."
        }
        p {
            "Visit"
            a href=(endpoint) { (endpoint) }
            "to confirm this email address."
        }
        p {
            "This link expires in 30 minutes."
        }
        (partials::footer())
    }
}

pub fn confirmation(username: &str, new_email: &str) -> Markup {
    html!(
        (partials::salute())
        p {
            "You have changed your email for your Geode SDK developer account " (username) " to " (new_email) "."
        }
        p {
            "If you didn't request this, please contact an index admin immediately."
        }
        (partials::footer())
    )
}
