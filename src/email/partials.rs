use maud::{Markup, html};

pub fn salute() -> Markup {
    html!(
        p {
            "Hi!"
        }
    )
}

pub fn footer() -> Markup {
    html!(
        p {
            "Thanks," br;
            "The Geode Team"
        }
    )
}
