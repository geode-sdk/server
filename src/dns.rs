use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use reqwest::dns::{Name, Resolve, Resolving};

#[derive(thiserror::Error, Debug)]
pub enum ValidateDnsError {
    #[error("couldn't resolve DNS")]
    CantResolveDns,
}

#[derive(Clone, Default, Debug)]
pub struct ValidateDnsResolver;

impl Resolve for ValidateDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            tracing::debug!("resolving {}", name.as_str());

            let ips: Vec<SocketAddr> = parse_name_to_ips(&name)
                .await
                .inspect_err(|e| tracing::warn!("Failed to resolve DNS: {:?}", e))
                .unwrap_or_default()
                .into_iter()
                .map(|ip| SocketAddr::new(ip, 0))
                .collect();

            Ok(Box::new(ips.into_iter()) as Box<dyn Iterator<Item = SocketAddr> + Send>)
        })
    }
}

async fn parse_name_to_ips(name: &Name) -> Result<Vec<IpAddr>, ValidateDnsError> {
    Ok(tokio::net::lookup_host(name.as_str())
        .await
        .inspect_err(|e| tracing::warn!("ValidateDnsResolver DNS lookup failed: {:?}", e))
        .map_err(|_| ValidateDnsError::CantResolveDns)?
        .map(|s| s.ip())
        .filter(|&ip| !is_disallowed_ip(ip))
        .collect())
}

/// Most of this function has been stolen from IpAddr::is_global().
///
/// Since that's still an unstable rust feature, we'll use this for now.
fn is_disallowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback() // 127.0.0.0/8
                || v4.is_private() // 10.0.0.0/8 | 192.168.0.0/16 | 172.16.0.0/12
                || v4.is_link_local() // covers 169.254.169.254 cloud metadata
                || v4.is_unspecified() // 0.0.0.0
                || v4.is_broadcast() // 255.255.255.255
                || v4.is_documentation() // 192.0.2.0/24 | 198.51.100.0/24 | 203.0.113.0/24
                || v4.is_multicast() // 224.0.0.0/4
                || is_shared_nat(v4) // 100.64.0.0/10 CGNAT
                || is_ietf_protocol_assignment(v4) // 192.0.0.0/24, (except 192.0.0.9, 192.0.0.10)
                || is_reserved(v4) // 240.0.0.0/4
                || is_benchmarking(v4) // 198.18.0.0/15
        }
        IpAddr::V6(v6) => {
            v6.is_loopback() // ::1
            || v6.is_unspecified() // ::
            || v6.is_unique_local() // fc00::/7
            || v6.is_unicast_link_local() // fe80::/10
            || (v6.segments()[0] == 0x2001 && v6.segments()[1] == 0xdb8) // 2001:db8::/32 documentation
            || v6.is_multicast() // ff00::/8
            || v6
                .to_ipv4_mapped()
                .is_some_and(|v4| is_disallowed_ip(IpAddr::V4(v4)))
        }
    }
}

/// Denies ipv4 like `100.64.0.0/10`, used for CGNAT
fn is_shared_nat(v4: Ipv4Addr) -> bool {
    let o = v4.octets();
    o[0] == 100 && (o[1] & 0b1100_0000) == 0b0100_0000
}

fn is_ietf_protocol_assignment(v4: Ipv4Addr) -> bool {
    v4.octets()[0] == 192
        && v4.octets()[1] == 0
        && v4.octets()[2] == 0
        && v4.octets()[3] != 9
        && v4.octets()[3] != 10
}

fn is_reserved(v4: Ipv4Addr) -> bool {
    (v4.octets()[0] & 0xf0) == 240
}

fn is_benchmarking(v4: Ipv4Addr) -> bool {
    v4.octets()[0] == 198 && (v4.octets()[1] & 0xfe) == 18
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

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
