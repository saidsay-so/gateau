use std::{ffi::OsStr, io::Write, process::Command, str::FromStr};

use color_eyre::{
    eyre::{ensure, eyre, Context},
    Result,
};
use cookie::Cookie;

use crate::{url::BaseDomain, Args};

use crate::{
    chrome::{self, ChromeVariant},
    firefox,
};

mod output;

#[derive(Debug, Clone, Copy)]
pub enum Browser {
    Firefox,
    Chromium,
    Chrome,
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

    fn get_cookies(&self, browser: Browser) -> Result<Vec<Cookie<'static>>> {
        match browser {
            Browser::Firefox => {
                let path_provider = firefox::paths::PathProvider::default_profile();

                let db_path = self
                    .args
                    .cookie_db
                    .clone()
                    .ok_or_else(|| eyre!("No path found for database path"))
                    .unwrap_or_else(|_| path_provider.cookies_database());

                let conn = crate::utils::get_connection(db_path, self.args.bypass_lock)?;
                firefox::get_cookies(&conn)
            }

            Browser::Chrome | Browser::Chromium => {
                let chrome_variant = match browser {
                    Browser::Chrome => ChromeVariant::Chrome,
                    Browser::Chromium => ChromeVariant::Chromium,
                    _ => unreachable!(),
                };

                let path_provider = chrome::paths::PathProvider::default_profile(chrome_variant);

                let db_path = self
                    .args
                    .cookie_db
                    .clone()
                    .ok_or_else(|| eyre!("No path found for database path"))
                    .unwrap_or_else(|_| path_provider.cookies_database());

                let conn = crate::utils::get_connection(db_path, self.args.bypass_lock)?;
                chrome::get_cookies(&conn, chrome_variant, path_provider)
            }
        }
    }

    /// Wraps the provided command while passing the cookies as a temporary file to the command.
    fn wrap_command<'a, C, A, Args, O>(
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

    pub fn run(&mut self) -> Result<Option<i32>> {
        let browser = self.args.browser.unwrap_or(Browser::Firefox);

        let mut cookies = self.get_cookies(browser)?;

        match &self.args.mode {
            crate::Mode::Output { format, hosts } => {
                // Filter cookies by domain
                if !hosts.is_empty() {
                    cookies.retain(|cookie| {
                        let domain = cookie.domain().unwrap();
                        let cookie_valid_domain = match domain.chars().next() {
                            Some('.') => domain.get(1..).unwrap(),
                            _ => domain,
                        };

                        hosts.iter().any(|h| {
                            domain == h
                                || h.base_domain()
                                    .as_deref()
                                    .or_else(|| h.host())
                                    // either the base domain or the host should be Some
                                    .unwrap()
                                    .ends_with(cookie_valid_domain)
                        })
                    });
                }

                let formatter = match format.unwrap_or(crate::OutputFormat::Netscape) {
                    crate::OutputFormat::Netscape => output::netscape,
                    crate::OutputFormat::Human => output::human,
                    crate::OutputFormat::HttpieSession => output::httpie_session,
                };
                let mut stream = std::io::stdout().lock();

                formatter(&cookies, &mut stream)
                    .map(|_| None)
                    .wrap_err("Could not output cookies to the provided stream")
            }

            crate::Mode::Wrap {
                command,
                forwarded_args,
            } => {
                let (cmd, option, cookies_formatter): (
                    _,
                    _,
                    fn(&[Cookie], _) -> std::io::Result<()>,
                ) = match command {
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

                let capacity = (128 * cookies.len()).next_power_of_two();
                let mut cookies_buf = Vec::with_capacity(capacity);

                cookies_formatter(&cookies, &mut cookies_buf)?;

                App::wrap_command(cmd, option, forwarded_args, cookies_buf).map(Some)
            }
        }
    }
}
