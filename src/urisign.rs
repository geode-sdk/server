use chrono::Utc;
use http::{Uri, uri::PathAndQuery};
use sha2::Digest;
use std::fmt::Write;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum CheckUriError {
    #[error("signature has expired")]
    Expired,
    #[error("signature is invalid")]
    Invalid,
}

/// Returns a signed URI creeated using the passed URI.
///
/// The returned URI contains a new `s` query param, containing
/// the signature of the URI.
///
/// If the passed URL contains `s` as a query param, it will be discarded and will
/// not be added to the final hash.
///
/// You can optionally offer a UNIX timestamp expiration for
/// the signed URI, which will be added to the query params as `exp`.
///
/// IMPORTANT: query params named `s` or `exp` WILL BE REMOVED from the URI.
///
/// # Example
///
/// ```rust
/// use http::Uri;
///
/// let uri = Uri::try_from("/path?test=1").unwrap();
/// let signed = urisign::sign_uri(uri, "Wmfd2893gb7", None); // signed without an expiration
///
/// let uri = Uri::try_from("/path?test=1").unwrap();
/// let signed_with_exp = urisign::sign_uri(uri, "Wmfd2893gb7", Some(1691084580)); // signed with expiration date
/// ```
pub fn sign_uri(uri: Uri, salt: &str, exp: Option<u64>) -> Uri {
    let mut parts = uri.into_parts();

    let path_and_query = parts
        .path_and_query
        .unwrap_or(PathAndQuery::from_static(""));

    let query: String = path_and_query.query().unwrap_or_default().into();

    let (_, mut query) = strip_param(&query, "exp");

    if let Some(exp) = exp {
        let prefix = if query.is_empty() { "" } else { "&" };
        // This shouldn't fail (hopefully!)
        let _ = write!(&mut query, "{}exp={}", prefix, exp);
    }

    let (_, query) = strip_param(&query, "s");

    let signature = compute_signature(salt, path_and_query.path(), &query);

    let signature_prefix = if query.is_empty() { "" } else { "&" };
    let path_and_query = PathAndQuery::try_from(format!(
        "{}?{}{}s={}",
        path_and_query.path(),
        query,
        signature_prefix,
        signature
    ))
    .expect("rebuilt query is always valid");

    parts.path_and_query = Some(path_and_query);

    Uri::from_parts(parts).expect("rebuilt Uri should be valid")
}

/// Check if an URI has a valid signarture.
///
/// The check happens on the `s` query param. If the param is missing, or the
/// signature doesn't match, `Err(CheckUriError::Invalid)` is returned. If the
/// `exp` param is present and in the past, `Err(CheckUriError::Expired)` is
/// returned instead.
///
/// Should be safe against timing attacks, the check uses `constant_time_eq`.
///
/// # Example
///
/// ```rust
/// use http::Uri;
///
/// let uri = Uri::try_from("/path?test=1").unwrap();
///
/// let signed = urisign::sign_uri(uri, "Wmfd2893gb7", None);
///
/// let valid = urisign::check_signed_uri(&signed, "Wmfd2893gb7");
///
/// assert!(valid.is_ok());
/// ```
pub fn check_signed_uri(uri: &Uri, salt: &str) -> Result<(), CheckUriError> {
    let query: String = uri.query().unwrap_or_default().into();

    let (signature, query) = strip_param(&query, "s");

    if signature.is_none() {
        return Err(CheckUriError::Invalid);
    }

    let exp: Option<Result<i64, std::num::ParseIntError>> =
        find_param(&query, "exp").map(|exp| exp.parse());

    if let Some(exp) = exp {
        let now = Utc::now().timestamp();
        match exp {
            Ok(exp) => {
                if exp < now {
                    return Err(CheckUriError::Expired);
                }
            }
            Err(_) => {
                return Err(CheckUriError::Invalid);
            }
        }
    }

    let signature = signature.unwrap();
    let computed = compute_signature(salt, uri.path(), &query);

    if constant_time_eq::constant_time_eq(signature.as_bytes(), computed.as_bytes()) {
        Ok(())
    } else {
        Err(CheckUriError::Invalid)
    }
}

fn compute_signature(salt: &str, path: &str, query_without_s: &str) -> String {
    let mut hash = sha2::Sha256::new();

    hash.update(salt.as_bytes());
    hash.update(path.as_bytes());
    hash.update(b"?");
    hash.update(query_without_s.as_bytes());

    hex::encode(hash.finalize())
}

fn find_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    for component in query.split("&") {
        let maybe_stripped = component
            .strip_prefix(key)
            .and_then(|rest| rest.strip_prefix("="));

        if let Some(stripped) = maybe_stripped {
            return Some(stripped);
        }
    }

    None
}

fn strip_param<'a>(query: &'a str, key: &str) -> (Option<&'a str>, String) {
    let mut ret = String::with_capacity(query.len());

    let mut found: Option<&str> = None;

    for component in query.split("&") {
        let maybe_stripped = component
            .strip_prefix(key)
            .and_then(|rest| rest.strip_prefix("="));

        if let Some(stripped) = maybe_stripped {
            found = Some(stripped);
        } else {
            ret.push_str(component);
            ret.push('&');
        }
    }

    if !ret.is_empty() {
        ret.truncate(ret.trim_end_matches("&").len());
    }

    (found, ret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_uri_is_valid() {
        let uri: Uri = "/path?test=1".parse().unwrap();
        let signed = sign_uri(uri, "salt", None);

        assert_eq!(check_signed_uri(&signed, "salt"), Ok(()));
    }

    #[test]
    fn wrong_salt_is_rejected() {
        let uri: Uri = "/path?test=1".parse().unwrap();
        let signed = sign_uri(uri, "salt", None);

        assert_eq!(
            check_signed_uri(&signed, "wrong-salt"),
            Err(CheckUriError::Invalid)
        );
    }

    #[test]
    fn missing_signature_is_rejected() {
        let uri: Uri = "/path?test=1".parse().unwrap();

        assert_eq!(check_signed_uri(&uri, "salt"), Err(CheckUriError::Invalid));
    }

    #[test]
    fn tampered_query_is_rejected() {
        let uri: Uri = "/path?test=1".parse().unwrap();
        let signed = sign_uri(uri, "salt", None);
        let existing_sig = find_param(signed.query().unwrap(), "s").unwrap();

        let tampered: Uri = format!("/path?test=2&s={}", existing_sig).parse().unwrap();

        assert_eq!(
            check_signed_uri(&tampered, "salt"),
            Err(CheckUriError::Invalid)
        );
    }

    #[test]
    fn tampered_signature_is_rejected() {
        let uri: Uri = "/path?test=1".parse().unwrap();
        let signed = sign_uri(uri, "salt", None);
        let existing_sig = find_param(signed.query().unwrap(), "s").unwrap();

        let bogus_sig = "0".repeat(existing_sig.len());
        let tampered: Uri = format!("/path?test=1&s={}", bogus_sig).parse().unwrap();

        assert_eq!(
            check_signed_uri(&tampered, "salt"),
            Err(CheckUriError::Invalid)
        );
    }

    #[test]
    fn signing_ignores_existing_s_param() {
        let uri: Uri = "/path?test=1&s=bogus".parse().unwrap();
        let signed = sign_uri(uri, "salt", None);

        assert_eq!(check_signed_uri(&signed, "salt"), Ok(()));
    }

    #[test]
    fn exp_param_changes_signature() {
        let uri: Uri = "/path?test=1".parse().unwrap();
        let future = Utc::now().timestamp() as u64 + 3600;

        let signed_without_exp = sign_uri(uri.clone(), "salt", None);
        let signed_with_exp = sign_uri(uri, "salt", Some(future));

        assert_ne!(signed_without_exp, signed_with_exp);
        assert_eq!(check_signed_uri(&signed_with_exp, "salt"), Ok(()));
    }

    #[test]
    fn future_exp_is_valid() {
        let uri: Uri = "/path?test=1".parse().unwrap();
        let future = Utc::now().timestamp() as u64 + 3600;

        let signed = sign_uri(uri, "salt", Some(future));

        assert_eq!(check_signed_uri(&signed, "salt"), Ok(()));
    }

    #[test]
    fn expired_uri_is_rejected() {
        let uri: Uri = "/path?test=1".parse().unwrap();
        let past = Utc::now().timestamp() as u64 - 3600;

        let signed = sign_uri(uri, "salt", Some(past));

        assert_eq!(check_signed_uri(&signed, "salt"), Err(CheckUriError::Expired));
    }

    #[test]
    fn malformed_exp_is_rejected() {
        // Build a URI whose signature is genuinely valid for its own
        // (malformed) `exp`, so this isolates the parse-failure path
        // from a plain signature mismatch.
        let query = "test=1&exp=not-a-number";
        let signature = compute_signature("salt", "/path", query);
        let uri: Uri = format!("/path?{}&s={}", query, signature).parse().unwrap();

        assert_eq!(check_signed_uri(&uri, "salt"), Err(CheckUriError::Invalid));
    }
}
