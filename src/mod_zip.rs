use std::cell::Cell;
use std::io::Seek;
use std::io::{BufReader, Cursor, Read};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

use actix_web::web::Bytes;
use image::codecs::png::PngDecoder;
use image::codecs::png::PngEncoder;
use image::{DynamicImage, GenericImageView};
use image::{ImageEncoder, ImageError};
use url::Url;
use zip::ZipArchive;
use zip::read::ZipFile;
use zip::result::ZipError;

use crate::pin_dns::PINNED_ADDR;

const DOWNLOAD_DENYLIST_DOMAINS: [&str; 1] = ["localhost"];
const DOWNLOAD_DENYLIST_TLDS: [&str; 4] = [".host", ".lan", ".local", ".internal"];
const MAX_REDIRECTS: u8 = 10;

#[derive(thiserror::Error, Debug)]
pub enum ModZipError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Image operation error: {0}")]
    ImageError(#[from] ImageError),
    #[error("Failed to unzip .geode file: {0}")]
    ZipError(#[from] ZipError),
    #[error("Failed to parse JSON: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("Invalid mod logo: {0}")]
    InvalidLogo(String),
    #[error("Download link is invalid")]
    InvalidModFileUrl,
    #[error("Too many redirects when downloading .geode file")]
    TooManyRedirects,
    #[error("Invalid Location header on download URL redirect")]
    InvalidRedirect,
    #[error(".geode file hash mismatch: {0} doesn't match {1}")]
    ModFileHashMismatch(String, String),
    #[error("Failed to fetch .geode file: {0}")]
    ModFileFetchError(#[from] reqwest::Error),
    #[error(".geode file is too large ({0} MB), maximum is {1} MB")]
    ModFileTooLarge(u64, u64),
    #[error(".geode file is too large after uncompression ({0} MB), maximum is {1} MB")]
    ModFileTooLargeUncompressed(u64, u64),
    #[error("Invalid mod.json: {0}")]
    InvalidModJson(String),
    #[error("Invalid binaries: {0}")]
    InvalidBinaries(String),
}

pub fn extract_mod_logo<R: Read>(file: &mut ZipFile<R>) -> Result<Vec<u8>, ModZipError> {
    const FIVE_MEGABYTES: u64 = 5 * 1000 * 1000;
    if file.size() > FIVE_MEGABYTES {
        return Err(ModZipError::InvalidLogo(
            "Logo size excedes max allowed size (5 MB)".into(),
        ));
    }

    let mut logo: Vec<u8> = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut logo)
        .inspect_err(|e| tracing::error!("logo.png read fail: {}", e))?;

    let mut reader = BufReader::new(Cursor::new(logo));

    let mut img = PngDecoder::new(&mut reader)
        .and_then(DynamicImage::from_decoder)
        .inspect_err(|e| tracing::error!("Failed to create PngDecoder: {}", e))?;

    let dimensions = img.dimensions();

    if dimensions.0 != dimensions.1 {
        return Err(ModZipError::InvalidLogo(format!(
            "Mod logo must have 1:1 aspect ratio. Current size is {}x{}",
            dimensions.0, dimensions.1
        )));
    }

    if (dimensions.0 > 336) || (dimensions.1 > 336) {
        img = img.resize(336, 336, image::imageops::FilterType::Lanczos3);
    }

    let mut cursor: Cursor<Vec<u8>> = Cursor::new(vec![]);

    let encoder = PngEncoder::new_with_quality(
        &mut cursor,
        image::codecs::png::CompressionType::Best,
        image::codecs::png::FilterType::NoFilter,
    );

    let (width, height) = img.dimensions();

    encoder
        .write_image(img.as_bytes(), width, height, img.color().into())
        .inspect_err(|e| tracing::error!("Failed to downscale image to 336x336: {}", e))?;

    cursor.seek(std::io::SeekFrom::Start(0)).unwrap();

    let mut bytes: Vec<u8> = vec![];
    cursor.read_to_end(&mut bytes).unwrap();

    Ok(bytes)
}

pub fn validate_mod_logo<R: Read>(file: &mut ZipFile<R>) -> Result<(), ModZipError> {
    const FIVE_MEGABYTES: u64 = 5 * 1000 * 1000;
    if file.size() > FIVE_MEGABYTES {
        return Err(ModZipError::InvalidLogo(
            "Logo size excedes max allowed size (5 MB)".into(),
        ));
    }

    let mut logo: Vec<u8> = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut logo)
        .inspect_err(|e| tracing::error!("logo.png read fail: {}", e))?;

    let mut reader = BufReader::new(Cursor::new(logo));

    let img = PngDecoder::new(&mut reader)
        .and_then(DynamicImage::from_decoder)
        .inspect_err(|e| tracing::error!("Failed to create PngDecoder: {}", e))?;

    let dimensions = img.dimensions();

    if dimensions.0 != dimensions.1 {
        Err(ModZipError::InvalidLogo(format!(
            "Mod logo must have 1:1 aspect ratio. Current size is {}x{}",
            dimensions.0, dimensions.1
        )))
    } else {
        Ok(())
    }
}

pub async fn download_mod(
    http_client: &reqwest::Client,
    url: &str,
    limit_mb: u32,
) -> Result<Bytes, ModZipError> {
    download(http_client, url, limit_mb).await
}

pub async fn download_mod_hash_comp(
    http_client: &reqwest::Client,
    url: &str,
    hash: &str,
    limit_mb: u32,
) -> Result<Bytes, ModZipError> {
    let bytes = download(http_client, url, limit_mb).await?;

    let slice: &[u8] = &bytes;

    let new_hash = sha256::digest(slice);
    if new_hash != hash {
        return Err(ModZipError::ModFileHashMismatch(hash.into(), new_hash));
    }

    Ok(bytes)
}

pub fn bytes_to_ziparchive(bytes: &[u8]) -> Result<ZipArchive<Cursor<&[u8]>>, ModZipError> {
    ZipArchive::new(Cursor::new(bytes))
        .inspect_err(|e| tracing::error!("Failed to create ZipArchive: {}", e))
        .map_err(|e| e.into())
}

async fn download(
    http_client: &reqwest::Client,
    url: &str,
    limit_mb: u32,
) -> Result<Bytes, ModZipError> {
    let mut current_url = Url::parse(url).map_err(|_| ModZipError::InvalidModFileUrl)?;

    tracing::debug!("fetching mod from {current_url}");

    let limit_bytes: u64 = limit_mb as u64 * 1_000_000;

    for i in 0..MAX_REDIRECTS {
        tracing::debug!("starting hop {}", i + 1);
        let addrs = validate_download_url(&current_url)?;
        let port = current_url.port_or_known_default().unwrap_or(443);
        let addr = std::net::SocketAddr::new(addrs[0], port);
        tracing::debug!("DNS validated as {addr}");

        // Pin the validated ip address in our cool custom resolver
        let response = PINNED_ADDR.scope(Cell::new(Some(addr)), async {
            http_client.get(url)
                .send()
                .await
                .inspect_err(|e| tracing::error!("Failed to fetch .geode file: {e}"))
        }).await?;

        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .ok_or(ModZipError::InvalidRedirect)?
                .to_str()
                .map_err(|_| ModZipError::InvalidRedirect)?;
            current_url = current_url
                .join(location)
                .map_err(|_| ModZipError::InvalidRedirect)?;
            continue;
        }

        let mut response = response
            .error_for_status()
            .inspect_err(|e| tracing::error!("Failed to fetch .geode file: {e}"))?;

        // Check Content-Length, but the server can lie about this, so we'll also stream the file
        // If the header is somehow unavailable, we'll just check the size when streaming
        let content_length = response.content_length().unwrap_or(0);

        if content_length > limit_bytes {
            let len_mb = content_length / 1_000_000;
            return Err(ModZipError::ModFileTooLarge(len_mb, limit_mb.into()));
        }

        let mut data: Vec<u8> = Vec::with_capacity(content_length as usize);

        let mut streamed: u64 = 0;
        while let Some(chunk) = response.chunk().await? {
            streamed += chunk.len() as u64;

            if streamed > limit_bytes {
                let len_mb = streamed / 1_000_000;
                return Err(ModZipError::ModFileTooLarge(len_mb, limit_mb.into()));
            }

            data.extend_from_slice(&chunk);
        }

        return Ok(Bytes::from(data));
    }

    Err(ModZipError::TooManyRedirects)
}

/// Hopefully this gets rid of all nasty ips
fn is_disallowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()   // covers 169.254.169.254 cloud metadata
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || is_shared_nat(v4) // 100.64.0.0/10 CGNAT
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || is_unique_local(v6)      // fc00::/7
                || is_ipv6_link_local(v6)   // fe80::/10
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| is_disallowed_ip(IpAddr::V4(v4)))
        }
    }
}

/// Denies ipv4 like `100.64.0.0/10`
fn is_shared_nat(v4: Ipv4Addr) -> bool {
    let o = v4.octets();
    o[0] == 100 && (o[1] & 0b1100_0000) == 0b0100_0000
}

/// Denies ipv6 like `fc00::/7`
fn is_unique_local(v6: Ipv6Addr) -> bool {
    (v6.segments()[0] & 0xfe00) == 0xfc00
}

/// Denies ipv6 like `fe80::/10`
fn is_ipv6_link_local(v6: Ipv6Addr) -> bool {
    (v6.segments()[0] & 0xffc0) == 0xfe80
}

fn allowed_scheme(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

fn ends_with_label(host: &str, suffix_with_dot: &str) -> bool {
    let label = &suffix_with_dot[1..];
    host == label || host.ends_with(suffix_with_dot)
}

fn is_denied_host(host: &str) -> bool {
    DOWNLOAD_DENYLIST_DOMAINS.contains(&host) || DOWNLOAD_DENYLIST_TLDS.iter().any(|&i| ends_with_label(host, i))
}

fn validate_download_url(url: &Url) -> Result<Vec<IpAddr>, ModZipError> {
    // First, validate the domain
    if !allowed_scheme(url) {
        return Err(ModZipError::InvalidModFileUrl);
    }

    let domain = url.domain().ok_or(ModZipError::InvalidModFileUrl)?;

    if is_denied_host(domain) {
        return Err(ModZipError::InvalidModFileUrl);
    }

    // Now resolve and validate the IP itself to make sure
    // the DNS isn't pointing to something bad
    let host = url
        .host_str()
        .ok_or(ModZipError::InvalidModFileUrl)
        .inspect_err(|_| tracing::warn!("host_str() returned None - very weird!"))?;
    let port = url.port_or_known_default().unwrap_or(443);

    let addrs: Vec<IpAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|_| ModZipError::InvalidModFileUrl)?
        .map(|s| s.ip())
        .collect();

    if addrs.is_empty() {
        tracing::warn!("{host}:{port} failed DNS resolution");
        return Err(ModZipError::InvalidModFileUrl);
    }

    if let Some(ip) = addrs.iter().find(|&ip| is_disallowed_ip(*ip)) {
        tracing::warn!("{host}:{port} resolved to disallowed ip {ip}");
        return Err(ModZipError::InvalidModFileUrl);
    }

    Ok(addrs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    #[test]
    fn rejects_non_http_schemes() {
        assert!(!allowed_scheme(&Url::parse("file:///etc/passwd").unwrap()));
        assert!(!allowed_scheme(&Url::parse("ftp://example.com/x").unwrap()));

        assert!(allowed_scheme(&Url::parse("https://example.com").unwrap()));
        assert!(allowed_scheme(&Url::parse("http://example.com").unwrap()));
    }

    #[test]
    fn exact_domain_match() {
        assert!(is_denied_host("localhost"));
    }

    #[test]
    fn exact_domain_is_case_sensitive_by_design() {
        // url::Url normalizes host to lowercase during parsing, so this
        // function assumes lowercase input. Document that assumption here.
        assert!(!is_denied_host("LOCALHOST"));
    }

    #[test]
    fn tld_exact_match() {
        // host == the bare suffix itself, no subdomain
        assert!(is_denied_host("internal"));
        assert!(is_denied_host("local"));
    }

    #[test]
    fn tld_subdomain_match() {
        assert!(is_denied_host("foo.internal"));
        assert!(is_denied_host("service.lan"));
        assert!(is_denied_host("printer.local"));
        assert!(is_denied_host("db.host"));
        assert!(is_denied_host("deep.nested.sub.internal"));
    }

    #[test]
    fn allows_unrelated_domains() {
        assert!(!is_denied_host("example.com"));
        assert!(!is_denied_host("github.com"));
        assert!(!is_denied_host("sub.example.com"));
    }

    #[test]
    fn rejects_ip_literal_hosts() {
        assert!(validate_download_url(&Url::parse("http://127.0.0.1/x").unwrap()).is_err());
        assert!(validate_download_url(&Url::parse("http://[::1]/download").unwrap()).is_err());
    }

    #[test]
    fn flags_private_and_special_ranges() {
        let cases: &[(&str, bool)] = &[
            ("127.0.0.1", true),
            ("10.0.0.5", true),
            ("172.16.0.1", true),
            ("192.168.1.1", true),
            ("169.254.169.254", true), // cloud metadata
            ("100.64.0.1", true),      // CGNAT
            ("8.8.8.8", false),
            ("1.1.1.1", false),
        ];
        for (ip, expected) in cases {
            let addr: IpAddr = ip.parse().unwrap();
            assert_eq!(is_disallowed_ip(addr), *expected, "failed for {ip}");
        }
    }

    #[test]
    fn flags_ipv6_special_ranges() {
        assert!(is_disallowed_ip("::1".parse().unwrap()));
        assert!(is_disallowed_ip("fc00::1".parse().unwrap()));
        assert!(is_disallowed_ip("fe80::1".parse().unwrap()));
        assert!(!is_disallowed_ip("2606:4700:4700::1111".parse().unwrap())); // public
    }
}
