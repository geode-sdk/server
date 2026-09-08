use argon2::{Algorithm, Argon2, Params, Version};

pub fn argon2(pepper: Option<&String>) -> Argon2<'_> {
    match pepper {
        Some(pepper) => Argon2::new_with_secret(
            pepper.as_bytes(),
            Algorithm::default(),
            Version::default(),
            Params::default(),
        )
        .unwrap(), // This only fails if the secret is too long, it'll be fine!
        None => Argon2::default(),
    }
}
