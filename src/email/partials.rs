use maud::{Markup, html};

pub fn salute(name: &str) -> Markup {
    html!(
        p {
            "Hi, " (name) "!"
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
