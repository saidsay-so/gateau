use std::{
    ffi::OsString,
    process::{Command, Stdio},
};

use cookie::Cookie;
use http::Uri;
use tempfile::tempdir;

use super::Browser;

pub(crate) struct SessionBuilder {
    browser: Browser,
    first_url: Option<Uri>,
}

impl<'a> SessionBuilder {
    pub fn new<U: Into<Uri>>(browser: Browser, first_url: Option<U>) -> Self {
        Self {
            browser,
            first_url: first_url.map(|u| u.into()),
        }
    }

    pub fn build(self) -> color_eyre::Result<Session<'a>> {
        let session_context = tempdir()?;

        let url = self
            .first_url
            .map(|u| u.to_string())
            .unwrap_or(String::from("about:blank"));

        match self.browser {
            Browser::Firefox => {
                let mut child = Command::new("firefox")
                    .arg("-no-remote")
                    .arg("-profile")
                    .arg(session_context.path())
                    .arg("-new-instance")
                    .arg(url)
                    .stderr(Stdio::null())
                    .stdout(Stdio::null())
                    .spawn()?;

                child.wait()?;

                let path_provider =
                    crate::firefox::paths::PathProvider::new(session_context.path(), Some(""));

                let db_path = path_provider.cookies_database();

                let conn = crate::utils::get_connection(db_path, false)?;
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
                    arg.push(session_context.path().as_os_str());
                    arg
                };

                let mut child = Command::new(cmd)
                    .arg("--new-window")
                    .arg(user_data_arg)
                    .arg(url)
                    .stderr(Stdio::null())
                    .stdout(Stdio::null())
                    .spawn()?;

                child.wait()?;

                let chrome_variant = match self.browser {
                    Browser::Chrome => crate::chrome::ChromeVariant::Chrome,
                    Browser::Chromium => crate::chrome::ChromeVariant::Chromium,
                    _ => unreachable!(),
                };

                let path_provider =
                    crate::chrome::paths::PathProvider::new(session_context.path(), Some(""));

                let db_path = path_provider.cookies_database();

                let conn = crate::utils::get_connection(db_path, false)?;
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
