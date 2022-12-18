use super::ChromeVariant;

use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

pub(crate) struct PathProvider {
    _base_dir: PathBuf,
    _profile: OsString,
    profile_dir: PathBuf,
}

impl PathProvider {
    /// Create a new path provider for the given profile and variant.
    /// If no profile is given, the default profile is used.
    pub(crate) fn new<R: AsRef<Path>, P: AsRef<OsStr>>(root_dir: R, profile: Option<P>) -> Self {
        let base_dir = root_dir.as_ref().to_owned();

        let profile = profile
            .map(|p| p.as_ref().into())
            .unwrap_or_else(|| OsString::from("Default"));

        Self {
            profile_dir: if cfg!(windows) {
                base_dir.join("User Data").join(&profile)
            } else {
                base_dir.join(&profile)
            },
            _profile: profile,
            _base_dir: base_dir,
        }
    }

    /// Returns a path provider for the default profile of the given browser variant.
    pub(crate) fn default_profile(variant: ChromeVariant) -> Self {
        let root_dir = if cfg!(windows) {
            dirs_next::data_local_dir()
        } else {
            dirs_next::config_dir()
        }
        .unwrap()
        .join(PathProvider::variant_base_folder(variant));

        Self::new::<_, &OsStr>(root_dir, None)
    }

    /// Returns the subpath of the base directory which changes depending on the variant.
    fn variant_base_folder(variant: ChromeVariant) -> &'static str {
        if cfg!(any(windows, target_os = "macos")) {
            match variant {
                ChromeVariant::Chromium => "Chromium",
                ChromeVariant::Chrome => "Google/Chrome",
            }
        } else {
            match variant {
                ChromeVariant::Chromium => "chromium",
                ChromeVariant::Chrome => "google-chrome",
            }
        }
    }

    /// Returns the path to the local state file.
    pub(crate) fn local_state(&self) -> PathBuf {
        self.profile_dir.join("Local State")
    }

    /// Returns the path to the cookies database.
    pub(crate) fn cookies_database(&self) -> PathBuf {
        self.profile_dir.join("Cookies")
    }
}
