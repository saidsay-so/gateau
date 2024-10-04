#![deny(unsafe_code)]

use std::{ffi::OsString, path::PathBuf, process::ExitCode, str::FromStr};

use app::App;
use bpaf::Bpaf;
use color_eyre::Result;
use http::Uri;

mod app;
mod url;

use gateau::Browser;

#[derive(Debug, Clone)]
enum WrappedCmd {
    Curl,
    Wget,
    HttpieHttp,
    HttpieHttps,
}

impl FromStr for WrappedCmd {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "curl" => Ok(WrappedCmd::Curl),
            "wget" => Ok(WrappedCmd::Wget),
            "httpie" | "https" => Ok(WrappedCmd::HttpieHttps),
            "http" => Ok(WrappedCmd::HttpieHttp),
            _ => Err(format!(
                "'{s}' is not one of the supported commands (curl, wget, http(s))"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum OutputFormat {
    Netscape,
    #[cfg(feature = "human")]
    Human,
    HttpieSession,
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "netscape" => Ok(OutputFormat::Netscape),
            #[cfg(feature = "human")]
            "human" => Ok(OutputFormat::Human),
            "httpie-session" | "httpie" => Ok(OutputFormat::HttpieSession),
            _ => Err(format!(
                "'{s}' is not one of the supported output formats (netscape, httpie-session)"
            )),
        }
    }
}

fn not_help(s: OsString) -> Option<OsString> {
    if s == "--help" {
        None
    } else {
        Some(s)
    }
}

#[derive(Debug, Clone, Bpaf)]
enum Mode {
    /// Output cookies to stdout in the specified format
    #[bpaf(command)]
    Output {
        /// Output format
        ///
        /// Supported formats: netscape, httpie-session
        format: Option<OutputFormat>,

        /// Hosts to filter cookies by
        #[bpaf(positional("HOSTS"), many)]
        hosts: Vec<Uri>,
    },

    /// Wrap a command with the imported cookies
    #[bpaf(command)]
    Wrap {
        /// Command which should be wrapped
        ///
        /// Supported commands: curl, wget, http, https
        #[bpaf(positional("COMMAND"))]
        command: WrappedCmd,

        /// Arguments for the wrapped command
        #[bpaf(any("ARGS", not_help), many)]
        forwarded_args: Vec<OsString>,
    },
}

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version)]
/// A simple wrapper to import cookies from browsers for curl, wget and httpie.
struct Args {
    /// Browser root path
    #[bpaf(short, long)]
    root_path: Option<PathBuf>,

    /// Open the browser in a new context and use the saved cookies when it closes
    #[bpaf(long)]
    session: bool,

    /// URLs to open in the session
    #[bpaf(long)]
    session_urls: Vec<Uri>,

    /// Browser(s) to import cookies from
    ///
    /// Supported browsers: chrome, chromium, firefox, edge
    #[bpaf(short, long)]
    browser: Option<Browser>,

    /// Bypass the lock on the database (can cause read errors)
    #[bpaf(long)]
    bypass_lock: bool,

    #[bpaf(external)]
    mode: Mode,
}

fn main() -> Result<ExitCode> {
    color_eyre::install()?;
    let args = args().run();

    if let Some(status) = App::new(args).run()? {
        let status: u8 = status.try_into().unwrap();
        Ok(ExitCode::from(status))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}
