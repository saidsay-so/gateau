use std::str::FromStr;

use self::chrome::ChromeVariant;

pub mod chrome;
pub mod firefox;

/// Function to filter hosts.
pub type HostFilterFn = dyn FnMut(&str) -> bool + Send + Sync;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Browser {
    Firefox,
    ChromeVariant(ChromeVariant),
}

impl std::fmt::Display for Browser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Browser::Firefox => write!(f, "Firefox"),
            Browser::ChromeVariant(ChromeVariant::Chromium) => write!(f, "Chromium"),
            Browser::ChromeVariant(ChromeVariant::Chrome) => write!(f, "Google Chrome"),
            Browser::ChromeVariant(ChromeVariant::Edge) => write!(f, "Microsoft Edge"),
        }
    }
}

impl FromStr for Browser {
    type Err = String;

    /// Parse a browser from a string.
    ///
    /// Supported browsers are:
    /// - firefox
    /// - chromium
    /// - chrome
    /// - edge
    ///
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "firefox" => Ok(Browser::Firefox),
            "chromium" => Ok(Browser::ChromeVariant(ChromeVariant::Chromium)),
            "chrome" => Ok(Browser::ChromeVariant(ChromeVariant::Chrome)),
            "edge" => Ok(Browser::ChromeVariant(ChromeVariant::Edge)),
            _ => Err(format!(
                "'{s}' is not one of the supported browsers (firefox, chromium, chrome, edge)"
            )),
        }
    }
}
