use super::ChromeVariant;

use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

/// Path provider for Chrome.
pub struct PathProvider {
    _base_dir: PathBuf,
    _profile: OsString,
    profile_dir: PathBuf,
}

impl PathProvider {
    /// Create a new path provider for the given profile and variant.
    /// If no profile is given, the root dir is used as the profile dir.
    pub fn new<R: AsRef<Path>, P: AsRef<OsStr>>(root_dir: R, profile: Option<P>) -> Self {
        let base_dir = root_dir.as_ref().to_owned();
        let profile = profile
            .as_ref()
            .map(|p| p.as_ref())
            .unwrap_or_else(|| OsStr::new("Default"));

        Self {
            profile_dir: if cfg!(windows) {
                base_dir.join("User Data").join(profile)
            } else {
                base_dir.join(profile)
            },
            _profile: profile.to_owned(),
            _base_dir: base_dir,
        }
    }

    pub fn from_root<P: AsRef<Path>>(root_dir: P) -> Self {
        Self::new::<_, &OsStr>(root_dir, None)
    }

    /// Returns a path provider for the default profile of the given browser variant.
    pub fn default_profile(variant: ChromeVariant) -> Self {
        let root_dir = if cfg!(windows) {
            dirs_next::data_local_dir()
        } else {
            dirs_next::config_dir()
        }
        .unwrap()
        .join(PathProvider::variant_base_folder(variant));

        const DEFAULT_PROFILE: &str = "Default";

        Self::new(root_dir, Some(DEFAULT_PROFILE))
    }

    /// Returns the subpath of the base directory which changes depending on the variant.
    const fn variant_base_folder(variant: ChromeVariant) -> &'static str {
        if cfg!(any(windows, target_os = "macos")) {
            match variant {
                ChromeVariant::Chromium => "Chromium",
                ChromeVariant::Chrome => "Google/Chrome",
                ChromeVariant::Edge => "Microsoft/Edge",
            }
        } else {
            match variant {
                ChromeVariant::Chromium => "chromium",
                ChromeVariant::Chrome => "google-chrome",
                ChromeVariant::Edge => "microsoft-edge",
            }
        }
    }

    /// Returns the path to the local state file.
    #[cfg(windows)]
    pub(crate) fn local_state(&self) -> PathBuf {
        self._base_dir.join("Local State")
    }

    /// Returns the path to the cookies database.
    pub fn cookies_database(&self) -> PathBuf {
        // The cookies database is stored in a subfolder called "Network" in newer versions of
        // Chromium (on Windows it seems). If this folder does not exist, we fall back to the old location.
        let new_path = self.profile_dir.join("Network").join("Cookies");

        if new_path.exists() {
            new_path
        } else {
            self.profile_dir.join("Cookies")
        }
    }
}
