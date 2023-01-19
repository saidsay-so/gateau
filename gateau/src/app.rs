use std::{
    ffi::OsStr,
    io::{self, Write},
    path::PathBuf,
    process::Command,
    str::FromStr,
    sync::Arc,
};

use color_eyre::{
    eyre::{ensure, Context},
    Result,
};
use cookie::Cookie;
use http::Uri;

use crate::{url::BaseDomain, utils::sqlite_predicate_builder, Args};

use crate::{
    chrome::{self, ChromeVariant},
    firefox,
};

use self::session::SessionBuilder;

mod output;
mod session;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Browser {
    Firefox,
    Chromium,
    Chrome,
}

impl std::fmt::Display for Browser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Browser::Firefox => write!(f, "Firefox"),
            Browser::Chromium => write!(f, "Chromium"),
            Browser::Chrome => write!(f, "Google Chrome"),
        }
    }
}

impl FromStr for Browser {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "firefox" => Ok(Browser::Firefox),
            "chromium" => Ok(Browser::Chromium),
            "chrome" => Ok(Browser::Chrome),
            _ => Err(format!(
                "'{s}' is not one of the supported browsers (firefox, chromium, chrome)"
            )),
        }
    }
}

pub struct App {
    args: Args,
}

impl App {
    pub(crate) fn new(args: Args) -> Self {
        Self { args }
    }

    /// Get the cookies matching the provided hosts from the specified browser.
    fn get_cookies(
        cookie_db_path: Option<PathBuf>,
        bypass_lock: bool,
        browser: Browser,
        hosts: Vec<Uri>,
    ) -> Result<Vec<Cookie<'static>>> {
        let hosts = Arc::from(hosts);

        match browser {
            Browser::Firefox => {
                let path_provider = firefox::paths::PathProvider::default_profile();

                let db_path = cookie_db_path.unwrap_or_else(|| path_provider.cookies_database());

                let conn = crate::utils::get_connection(db_path, bypass_lock)?;

                let hosts = Arc::clone(&hosts);
                sqlite_predicate_builder(&conn, "host_filter", move |cookie_host| {
                    filter_hosts(cookie_host, &hosts)
                })?;

                firefox::get_cookies(&conn)
            }

            Browser::Chrome | Browser::Chromium => {
                let chrome_variant = match browser {
                    Browser::Chrome => ChromeVariant::Chrome,
                    Browser::Chromium => ChromeVariant::Chromium,
                    _ => unreachable!(),
                };

                let path_provider = chrome::paths::PathProvider::default_profile(chrome_variant);

                let db_path = cookie_db_path.unwrap_or_else(|| path_provider.cookies_database());

                let conn = crate::utils::get_connection(db_path, bypass_lock)?;

                let hosts = Arc::clone(&hosts);
                sqlite_predicate_builder(&conn, "host_filter", move |host| {
                    filter_hosts(host, &hosts)
                })?;

                chrome::get_cookies(&conn, chrome_variant, path_provider)
            }
        }
    }

    /// Wraps the provided command while passing the cookies as a temporary file to the command.
    fn wrap_command<C, A, Args, O>(
        cmd: C,
        cookies_opt: A,
        forwarded_args: &[Args],
        formatted_cookies: O,
    ) -> Result<i32>
    where
        C: AsRef<OsStr>,
        A: AsRef<OsStr>,
        Args: AsRef<OsStr>,
        O: AsRef<[u8]>,
    {
        let mut tmp_cookie_file = tempfile::NamedTempFile::new()?;
        tmp_cookie_file.write_all(formatted_cookies.as_ref())?;
        let tmp_cookies_path = tmp_cookie_file.into_temp_path();

        let mut child = Command::new(cmd.as_ref())
            .arg(cookies_opt.as_ref())
            .arg(tmp_cookies_path)
            .args(forwarded_args)
            .spawn()?;

        let status = child.wait()?;
        ensure!(
            status.code().is_some(),
            "{cmd} has been killed by a signal",
            cmd = cmd.as_ref().to_string_lossy()
        );

        Ok(status.code().unwrap())
    }

    pub fn run(self) -> Result<Option<i32>> {
        let browser = self.args.browser.unwrap_or(Browser::Firefox);
        let session = self.args.session;
        let session_urls = self.args.session_urls;

        match self.args.mode {
            crate::Mode::Output { format, hosts } => {
                let cookies = if session {
                    let session = SessionBuilder::new(browser, session_urls, hosts).build()?;
                    session.cookies().to_vec()
                } else {
                    App::get_cookies(self.args.cookie_db, self.args.bypass_lock, browser, hosts)?
                };

                let mut stream = std::io::stdout().lock();

                let formatter = match format.unwrap_or(crate::OutputFormat::Netscape) {
                    crate::OutputFormat::Netscape => output::netscape,
                    #[cfg(feature = "human")]
                    crate::OutputFormat::Human => output::human,
                    crate::OutputFormat::HttpieSession => output::httpie_session,
                };

                formatter(&cookies, &mut stream)
                    .map(|_| None)
                    .or_else(|e| match e {
                        e if e.kind() == io::ErrorKind::BrokenPipe => Ok(None),
                        _ => Err(e),
                    })
                    .wrap_err("Could not output cookies to the provided stream")
            }

            crate::Mode::Wrap {
                command,
                forwarded_args,
            } => {
                let (cmd, option, formatter): (_, _, fn(_, _) -> _) = match command {
                    crate::WrappedCmd::Curl => ("curl", "-b", output::netscape),
                    crate::WrappedCmd::Wget => ("wget", "--load-cookies", output::netscape),
                    crate::WrappedCmd::HttpieHttp | crate::WrappedCmd::HttpieHttps => {
                        let cmd = match command {
                            crate::WrappedCmd::HttpieHttp => "http",
                            crate::WrappedCmd::HttpieHttps => "https",
                            _ => unreachable!(),
                        };

                        (cmd, "--session", output::httpie_session)
                    }
                };

                let cookies = if session {
                    let session = SessionBuilder::new(browser, session_urls, Vec::new()).build()?;
                    session.cookies().to_vec()
                } else {
                    App::get_cookies(
                        self.args.cookie_db,
                        self.args.bypass_lock,
                        browser,
                        Vec::new(),
                    )?
                };

                let capacity = (64 * cookies.len()).next_power_of_two();
                let mut cookies_buf = Vec::with_capacity(capacity);

                formatter(&cookies, &mut cookies_buf)?;

                App::wrap_command(cmd, option, &forwarded_args, cookies_buf).map(Some)
            }
        }
    }
}

fn filter_hosts(domain: &str, hosts: &[Uri]) -> bool {
    let cookie_valid_domain = match domain.chars().next() {
        Some('.') => domain.get(1..).unwrap(),
        _ => domain,
    };

    if cookie_valid_domain.is_empty() {
        return false;
    }

    hosts.is_empty()
        || hosts.iter().any(|h| {
            Some(cookie_valid_domain) == h.host()
                || h.base_domain()
                    .as_deref()
                    .or_else(|| h.host())
                    // either the base domain or the host should be Some
                    .unwrap()
                    .ends_with(cookie_valid_domain)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_hosts() {
        let hosts = vec![
            "https://www.example.com".parse().unwrap(),
            "https://www.example.org".parse().unwrap(),
        ];

        assert!(filter_hosts("example.com", &hosts));
        assert!(filter_hosts("example.org", &hosts));
        assert!(filter_hosts(".example.com", &hosts));
        assert!(filter_hosts(".example.org", &hosts));
        assert!(!filter_hosts("example.net", &hosts));
        assert!(!filter_hosts(".example.net", &hosts));
    }

    #[test]
    fn test_filter_with_empty_hosts() {
        let hosts = vec![];

        assert!(filter_hosts("example.com", &hosts));
        assert!(filter_hosts("example.org", &hosts));
        assert!(filter_hosts(".example.com", &hosts));
        assert!(filter_hosts(".example.org", &hosts));
        assert!(filter_hosts("example.net", &hosts));
        assert!(filter_hosts(".example.net", &hosts));
    }

    #[test]
    fn test_filter_with_empty_domain() {
        let hosts = vec!["https://www.example.com".parse().unwrap()];

        assert!(!filter_hosts("", &hosts));
    }

    #[test]
    fn test_filter_wildcard() {
        let hosts = vec!["https://www.example.com".parse().unwrap()];

        assert!(filter_hosts("example.com", &hosts));
        assert!(filter_hosts(".example.com", &hosts));
        assert!(filter_hosts("www.example.com", &hosts));
        assert!(filter_hosts(".www.example.com", &hosts));
        assert!(!filter_hosts("example.org", &hosts));
        assert!(!filter_hosts(".example.org", &hosts));
        assert!(!filter_hosts("www.example.org", &hosts));
        assert!(!filter_hosts(".www.example.org", &hosts));
    }
}
