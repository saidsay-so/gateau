//! Output functions.
//! The functions in this module are used to output the cookies in various formats.
//!
//!
//! ## Output formats
//!
//! ### Netscape
//!
//! The Netscape format is the one used by the `cookies.txt` file.
//! It is recognized by `curl` and `wget`.
//!
//! ### HTTPie session
//!
//! The HTTPie session format is the one used by the `httpie` tool.
//! It is not stable nor documented, therefore the structs can change and break at any time.
//! The structs are based on the `httpie` 3.2.1 source code.
//!
//! ### Human
//!
//! The human format is a custom format that is easy to read.

use std::{
    collections::HashMap,
    io::{self, Write},
};

use cookie::{time::format_description, Cookie};

use serde::Serialize;

/// Output cookies in Netscape (cookies.txt) format, recognized by curl and wget.
///
/// ## Panics
///
/// Panics if one the cookie's optional parameters is `None` or the expiration date is not a date.
pub fn netscape<'a, W: Write>(cookies: &'a [Cookie<'a>], writer: &mut W) -> io::Result<()> {
    const NETSCAPE_HEADER: &[u8] = b"# Netscape HTTP Cookie File\n";

    fn bool_to_uppercase(b: bool) -> &'static str {
        if b {
            "TRUE"
        } else {
            "FALSE"
        }
    }

    writer.write(NETSCAPE_HEADER)?;

    writer.write_all(
        cookies
            .iter()
            .map(|cookie| {
                format!(
                    "{domain}\t{flag}\t{path}\t{secure}\t{expiration}\t{name}\t{value}",
                    domain = cookie.domain().unwrap(),
                    flag = bool_to_uppercase(cookie.domain().map(|d| d.starts_with('.')).unwrap()),
                    path = cookie.path().unwrap(),
                    secure = bool_to_uppercase(cookie.secure().unwrap()),
                    expiration = cookie
                        .expires()
                        .and_then(|t| t.datetime())
                        .unwrap()
                        .unix_timestamp(),
                    name = cookie.name(),
                    value = cookie.value()
                )
            })
            .collect::<Vec<String>>()
            .join("\n")
            .as_bytes(),
    )
}

pub fn human<'a, W: Write>(cookies: &'a [Cookie<'a>], writer: &mut W) -> io::Result<()> {
    let format =
        format_description::parse("[weekday], [day] [month] [year] [hour]:[minute]:[second] GMT")
            .unwrap();

    writer.write_all(cookies.iter()
        .map(|cookie| {
            format!(
                "{name}={value}; Domain={domain}; Path={path}; Secure={secure}; HttpOnly={http_only}; SameSite={same_site}; Expires={expires}",
                name = cookie.name(),
                value = cookie.value(),
                domain = cookie.domain().unwrap(),
                path = cookie.path().unwrap(),
                secure = cookie.secure().unwrap(),
                http_only = cookie.http_only().unwrap(),
                same_site = cookie.same_site().unwrap(),
                expires = cookie
                    .expires()
                    .and_then(|t| t.datetime())
                    .unwrap()
                    .format(&format).unwrap()
            )
        })
        .collect::<Vec<String>>()
        .join("\n").as_bytes())
}

/// Raw cookie data as it is stored in the session file.
/// The format is based on the accepted arguments of the `create_cookie` function
/// from `requests` Python library.
#[derive(Debug, Clone, Serialize)]
struct RawHttpieCookieV0 {
    name: String,
    value: String,
    port: Option<u16>,
    domain: String,
    path: String,
    secure: bool,
    /// The cookie's expiration date, in seconds since the Unix epoch.
    expires: Option<i64>,
    discard: bool,
    comment: Option<String>,
    comment_url: Option<String>,
    rest: HashMap<String, serde_json::Value>,
    rfc2109: bool,
}

#[derive(Debug, Clone, Serialize)]
struct RawHttpieHeader {
    name: String,
    value: String,
}

#[derive(Debug, Clone, Serialize)]
struct RawHttpieAuth {
    #[serde(rename = "type")]
    auth_type: Option<String>,
    username: Option<String>,
    password: Option<String>,
}

/// A HTTPie session containing headers, cookies and authentication information.
/// See <https://httpie.org/doc#sessions>.
/// Note that the format is not stable nor documented,
/// therefore the structs can change and break at any time.
/// The structs are based on the `httpie` 3.2.1 source code.
#[derive(Debug, Clone, Serialize)]
struct RawHttpieSession {
    headers: Vec<RawHttpieHeader>,
    cookies: Vec<RawHttpieCookieV0>,
    auth: RawHttpieAuth,
}

/// Output cookies in HTTPie session format.
///
/// ## Panics
///
/// Panics if one the cookie's optional parameters is `None` or the expiration date is not a date.
pub(crate) fn httpie_session<'a, W: Write>(
    cookies: &'a [Cookie<'a>],
    writer: &mut W,
) -> io::Result<()> {
    let cookies = cookies
        .iter()
        .map(|cookie| RawHttpieCookieV0 {
            name: cookie.name().to_string(),
            value: cookie.value().to_string(),
            port: cookie
                .domain()
                .and_then(|d| d.rsplit(':').next().and_then(|p| p.parse().ok())),
            domain: cookie.domain().unwrap().to_string(),
            path: cookie.path().unwrap().to_string(),
            secure: cookie.secure().unwrap(),
            expires: cookie
                .expires()
                .and_then(|t| t.datetime())
                .map(|t| t.unix_timestamp()),
            discard: false,
            comment: None,
            comment_url: None,
            rest: HashMap::new(),
            rfc2109: false,
        })
        .collect::<Vec<_>>();

    serde_json::to_writer(
        writer,
        &RawHttpieSession {
            headers: Vec::new(),
            cookies,
            auth: RawHttpieAuth {
                auth_type: None,
                username: None,
                password: None,
            },
        },
    )
    .unwrap();

    Ok(())
}
