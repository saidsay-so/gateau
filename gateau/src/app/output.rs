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

use cookie::Cookie;

use serde::Serialize;

/// Output cookies in Netscape (cookies.txt) format, recognized by curl and wget.
///
/// ## Panics
///
/// Panics if one the cookie's optional parameters is `None` or the expiration date is not a date.
pub fn netscape<W: Write>(cookies: &[Cookie<'_>], writer: &mut W) -> io::Result<()> {
    const NETSCAPE_HEADER: &[u8] = b"# Netscape HTTP Cookie File\n";

    const fn bool_to_uppercase(b: bool) -> &'static str {
        if b {
            "TRUE"
        } else {
            "FALSE"
        }
    }

    writer.write_all(NETSCAPE_HEADER)?;

    for cookie in cookies {
        writeln!(
            writer,
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
        )?;
    }

    Ok(())
}

#[cfg(feature = "human")]
pub fn human<W: Write>(cookies: &[Cookie<'_>], writer: &mut W) -> io::Result<()> {
    use color_eyre::owo_colors::OwoColorize;
    use cookie::time::format_description;
    use itertools::Itertools;

    let format =
        format_description::parse("[weekday], [day] [month] [year] [hour]:[minute]:[second] GMT")
            .unwrap();

    macro_rules! human_field {
        ($name:ident, $value:expr) => {
            format!("{}: {}", stringify!($name).bold(), $value)
        };
    }

    for (domain, cookies) in cookies
        .iter()
        .into_group_map_by(|cookie| cookie.domain().unwrap())
        .into_iter()
        .sorted_by(|c1, c2| {
            let c1 = if c1.0.starts_with('.') {
                c1.0.get(1..).unwrap()
            } else {
                c1.0
            };

            let c2 = if c2.0.starts_with('.') {
                c2.0.get(1..).unwrap()
            } else {
                c2.0
            };

            c1.cmp(c2)
        })
    {
        writeln!(writer, "{}", domain.bold().blue())?;

        writeln!(writer)?;

        for cookie in cookies {
            writeln!(writer, "{}", "--------------------".bold().bright_black())?;

            writeln!(writer)?;

            writeln!(writer, "{}", human_field!(Name, cookie.name()))?;
            writeln!(writer, "{}", human_field!(Value, cookie.value()))?;
            writeln!(
                writer,
                "{}",
                human_field!(Path, cookie.path().unwrap().italic())
            )?;
            writeln!(writer, "{}", human_field!(Secure, cookie.secure().unwrap()))?;
            writeln!(
                writer,
                "{}",
                human_field!(HttpOnly, cookie.http_only().unwrap())
            )?;
            writeln!(
                writer,
                "{}",
                human_field!(SameSite, cookie.same_site().unwrap())
            )?;
            writeln!(
                writer,
                "{}",
                human_field!(
                    Expires,
                    cookie
                        .expires()
                        .and_then(|t| t.datetime())
                        .unwrap()
                        .format(&format)
                        .unwrap()
                )
            )?;

            writeln!(writer)?;
        }

        writeln!(writer)?;
    }

    Ok(())
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
    cookies: &[Cookie<'_>],
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
