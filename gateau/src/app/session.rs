use std::{
    ffi::OsString,
    process::{Command, Stdio},
    sync::Arc,
};

use color_eyre::eyre::Context;
use cookie::Cookie;
use http::Uri;
use tempfile::tempdir;

use crate::{
    app::filter_hosts,
    browser::chrome::{ChromeManager, ChromeVariant},
    utils::sqlite_predicate_builder,
};

use crate::browser::Browser;

/// Builder for a session.
/// A session is a temporary browser instance.
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

    /// Build a browser session.
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

                let path_provider = crate::browser::firefox::paths::PathProvider::new::<_, OsString>(
                    session_context.path(),
                    None,
                );

                let db_path = path_provider.cookies_database();

                let conn = crate::utils::get_connection(db_path, false)?;

                let hosts = Arc::clone(&hosts);
                sqlite_predicate_builder(&conn, "host_filter", move |host| {
                    filter_hosts(host, &hosts)
                })?;

                let cookies = crate::browser::firefox::get_cookies(&conn)?;

                Ok(Session { cookies })
            }

            Browser::ChromeVariant(chrome_variant) => {
                const CHROMIUM_USER_DATA_DIR_FLAG: &str = "--user-data-dir=";

                let cmd = match chrome_variant {
                    ChromeVariant::Chrome => "google-chrome",
                    ChromeVariant::Chromium => "chromium",
                    ChromeVariant::Edge => "edge",
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

                let path_provider = crate::browser::chrome::paths::PathProvider::new::<_, OsString>(
                    session_context.path(),
                    None,
                );

                let db_path = path_provider.cookies_database();

                let conn = crate::utils::get_connection(db_path, false)?;

                let hosts = Arc::clone(&hosts);
                sqlite_predicate_builder(&conn, "host_filter", move |host| {
                    filter_hosts(host, &hosts)
                })?;

                let manager = ChromeManager::new(chrome_variant, path_provider)?;
                let cookies =
                    manager.get_cookies()?;

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
