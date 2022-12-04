use std::{net::Ipv4Addr, str::FromStr};

use http::Uri;

/// Trait for extracting the base domain from a URL.
pub(crate) trait BaseDomain {
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
