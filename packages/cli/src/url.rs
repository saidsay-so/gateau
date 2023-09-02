use std::{net::Ipv4Addr, str::FromStr};

use http::Uri;

/// Trait for extracting the base domain from a URL.
pub trait BaseDomain {
    /// Returns the base domain of the URL, if it is a valid domain.
    fn base_domain(&self) -> Option<String>;
}

impl BaseDomain for Uri {
    fn base_domain(&self) -> Option<String> {
        self.host().filter(is_domain).and_then(|host| {
            let mut parts = host.rsplitn(3, '.');
            let ext = parts.next()?;
            let base_domain = parts.next()?;

            Some([base_domain, ext].join("."))
        })
    }
}

fn is_domain(host: &&str) -> bool {
    !host.starts_with('[') && Ipv4Addr::from_str(host).is_err()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_domain() {
        let uri = Uri::from_static("https://example.com");
        assert_eq!(uri.base_domain(), Some(String::from("example.com")));
    }

    #[test]
    fn test_base_domain_with_port() {
        let uri = Uri::from_static("https://www.example.com:8080");
        assert_eq!(uri.base_domain(), Some(String::from("example.com")));
    }

    #[test]
    fn test_base_domain_with_ipv4() {
        let url = Uri::from_static("https://127.0.0.1/");
        assert_eq!(url.base_domain(), None);
    }

    #[test]
    fn test_base_domain_with_ipv6() {
        let url = Uri::from_static("https://[::1]/");
        assert_eq!(url.base_domain(), None);
    }

    #[test]
    fn test_base_domain_with_subdomain() {
        let url = Uri::from_static("https://www.example.com");
        assert_eq!(url.base_domain(), Some(String::from("example.com")));
    }
}
