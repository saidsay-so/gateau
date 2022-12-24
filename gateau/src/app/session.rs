use std::{
    ffi::OsString,
    process::{Command, Stdio},
    sync::Arc,
};

use color_eyre::eyre::Context;
use cookie::Cookie;
use http::Uri;
use tempfile::tempdir;

use crate::{app::filter_hosts, utils::sqlite_predicate_builder};

use super::Browser;

/// Builder for a session.
/// A session is a temporary browser instance that is used to retrieve cookies.
#[derive(Debug, Clone)]
#[must_use]
pub(crate) struct SessionBuilder {
    browser: Browser,
    urls: Vec<Uri>,
    hosts: Vec<Uri>,
}

impl<'a> SessionBuilder {
    pub fn new(browser: Browser, urls: Vec<Uri>, hosts: Vec<Uri>) -> Self {
        Self {
            browser,
            urls,
            hosts,
        }
    }

    pub fn build(self) -> color_eyre::Result<Session<'a>> {
        let session_context = tempdir()?;

        eprintln!("Opening a {} session", self.browser);

        let url: Vec<_> = self.urls.into_iter().map(|u| u.to_string()).collect();

        let hosts = Arc::from(self.hosts);

        match self.browser {
            Browser::Firefox => {
                let mut child = Command::new("firefox")
                    .arg("-no-remote")
                    .arg("-profile")
                    .arg(session_context.path())
                    .arg("-new-instance")
                    .args(url)
                    .stderr(Stdio::null())
                    .stdout(Stdio::null())
                    .spawn()
                    .wrap_err("Failed to run firefox")?;

                child.wait()?;

                let path_provider = crate::firefox::paths::PathProvider::new::<_, OsString>(
                    session_context.path(),
                    None,
                );

                let db_path = path_provider.cookies_database();

                let conn = crate::utils::get_connection(db_path, false)?;

                let hosts = Arc::clone(&hosts);
                sqlite_predicate_builder(&conn, "host_filter", move |host| {
                    filter_hosts(host, &hosts)
                })?;

                let cookies = crate::firefox::get_cookies(&conn)?;

                Ok(Session { cookies })
            }

            Browser::Chrome | Browser::Chromium => {
                const CHROMIUM_USER_DATA_DIR_FLAG: &str = "--user-data-dir=";

                let cmd = match self.browser {
                    Browser::Chrome => "google-chrome",
                    Browser::Chromium => "chromium",
                    _ => unreachable!(),
                };

                let user_data_arg = {
                    let capacity = CHROMIUM_USER_DATA_DIR_FLAG.len()
                        + session_context.path().as_os_str().len();
                    let mut arg = OsString::with_capacity(capacity);
                    arg.push(CHROMIUM_USER_DATA_DIR_FLAG);
                    arg.push(session_context.path());
                    arg
                };

                let mut child = Command::new(cmd)
                    .arg("--new-window")
                    .arg(user_data_arg)
                    .args(url)
                    .stderr(Stdio::null())
                    .stdout(Stdio::null())
                    .spawn()
                    .wrap_err_with(|| format!("Failed to run {cmd}"))?;

                child.wait()?;

                let chrome_variant = match self.browser {
                    Browser::Chrome => crate::chrome::ChromeVariant::Chrome,
                    Browser::Chromium => crate::chrome::ChromeVariant::Chromium,
                    _ => unreachable!(),
                };

                let path_provider = crate::chrome::paths::PathProvider::new::<_, OsString>(
                    session_context.path(),
                    None,
                );

                let db_path = path_provider.cookies_database();

                let conn = crate::utils::get_connection(db_path, false)?;

                let hosts = Arc::clone(&hosts);
                sqlite_predicate_builder(&conn, "host_filter", move |host| {
                    filter_hosts(host, &hosts)
                })?;

                let cookies = crate::chrome::get_cookies(&conn, chrome_variant, path_provider)?;

                Ok(Session { cookies })
            }
        }
    }
}

pub(crate) struct Session<'a> {
    cookies: Vec<Cookie<'a>>,
}

impl<'a> Session<'a> {
    pub fn cookies(&self) -> &[Cookie<'a>] {
        &self.cookies
    }
}
