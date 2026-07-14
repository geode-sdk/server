use url::Url;

/// Add an `exp` query param to the passed URL.
pub fn set_url_exp(url: &mut Url, exp: u64) {
    url.query_pairs_mut()
        .append_pair("exp", &format!("{}", exp));
}

/// Returns a signed URL creeated using the passed URL.
///
/// The returned URL received 2 fields:
/// - `s`, the signature of the URL
///
/// If the passed URL contains `s` as a query param, it will be discarded and will
/// not be added to the final hash.
///
/// # Example
///
/// ```rust
/// let mut url = Url::parse("https://example.com?test=1").unwrap();
///
/// let signed = urlsign::sign_url(url.clone(), "Wmfd2893gb7"); // signed without an expiration
///
/// urlsign::set_url_exp(&mut url, 1691084580);
/// let signed_with_exp = urlsign::sign_url(url.clone(), "Wmfd2893gb7"); // signed with expiration date
/// ```
pub fn sign_url(mut url: Url, salt: &str) -> Url {
    let signed = do_sign_url(&mut url, salt);
    url.query_pairs_mut().append_pair("s", &signed);

    url
}

/// Check if an URL has a valid signarture.
///
/// The check happens on the `s` query param. If the param is missing, `false` is returned.
//
/// Should be safe against timing attacks, the check uses `constant_time_eq`.
///
/// # Example
///
/// ```rust
/// let url = Url::parse("https://example.com?test=1").unwrap();
///
/// let signed = urlsign::sign_url(url, "Wmfd2893gb7");
///
/// let valid = urlsign::check_signed_url(signed, "Wmfd2893gb7");
///
/// assert_eq!(valid, true);
/// ```
pub fn check_signed_url(url: &Url, salt: &str) -> bool {
    let existing_signature = match url.query_pairs().find(|(k, _)| k == "s") {
        Some((_, value)) => value.to_string(),
        None => return false,
    };

    let mut clone = url.clone();

    let signature = do_sign_url(&mut clone, salt);

    constant_time_eq::constant_time_eq((&existing_signature).as_bytes(), &(signature).as_bytes())
}

fn do_sign_url(url: &mut Url, salt: &str) -> String {
    let clone = url.clone();
    let new_query: Vec<(_, _)> = clone.query_pairs().filter(|(k, _)| k != "s").collect();

    url.set_query(None);
    for (k, v) in new_query {
        url.query_pairs_mut()
            .append_pair(&k.to_string()[..], &v.to_string()[..]);
    }

    sha256::digest(format!("{}{}", salt, url))
}
