use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

/// Path provider for Firefox.
pub struct PathProvider {
    _base_dir: PathBuf,
    profile_dir: PathBuf,
}

impl PathProvider {
    /// Create a new path provider for the given profile.
    /// If no profile is given, the root dir is used as the profile dir.
    pub fn new<R: AsRef<Path>, P: AsRef<OsStr>>(root_dir: R, profile: Option<P>) -> Self {
        let base_dir = root_dir.as_ref().to_owned();

        Self {
            _base_dir: base_dir.clone(),
            profile_dir: if let Some(profile) = profile.as_ref().map(|p| p.as_ref()) {
                base_dir.join(profile)
            } else {
                base_dir
            },
        }
    }

    pub fn from_root<R: AsRef<Path>>(root_dir: R) -> Self {
        Self::new::<_, &OsStr>(root_dir, None)
    }

    /// Returns a path provider for the default profile.
    ///
    /// # Panics
    ///
    /// This function panics if no default profile can be found.
    pub fn default_profile() -> Self {
        let root_dir = if cfg!(any(windows, target_os = "macos")) {
            dirs_next::config_dir()
        } else {
            dirs_next::home_dir()
        }
        .unwrap()
        .join(if cfg!(any(windows, target_os = "macos")) {
            "Mozilla/Firefox"
        } else {
            ".mozilla/firefox"
        });

        let profiles = tini::Ini::from_file(&root_dir.join("profiles.ini"))
            .expect("Cannot parse Firefox profiles.ini file");

        let default = PathProvider::get_default_profile_path(profiles)
            .expect("Cannot get Firefox default profile");

        Self::new(root_dir, Some(default))
    }

    /// Get the default profile's path from the profiles config.
    /// It selects the profile which is in the first `Install$INSTALL_HASH$` section found,
    /// or the first `Profile` section with `Default=1` if no `Install$INSTALL_HASH$` section is found.
    fn get_default_profile_path(profile_config: tini::Ini) -> Option<String> {
        if let Some(section) = profile_config
            .iter()
            .filter(|(name, _)| name.starts_with("Install"))
            .map(|(_, section)| section)
            .next()
        {
            section.get("Default")
        } else {
            profile_config
                .iter()
                .filter(|(name, _)| name.starts_with("Profile"))
                .filter(|(_, section)| section.get::<String>("Default").as_deref() == Some("1"))
                .map(|(_, section)| section)
                .next()
                .and_then(|section| section.get("Path"))
        }
    }

    /// Returns the path to the cookies database.
    pub fn cookies_database(&self) -> PathBuf {
        self.profile_dir.join("cookies.sqlite")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOWS_PROFILE: &str = r#"
[Install308046B0AF4A39CB]
Default=Profiles/i5izpoj2.default-release
Locked=1

[Profile1]
Name=default
IsRelative=1
Path=Profiles/3u2tt9lg.default
Default=1

[Profile0]
Name=default-release
IsRelative=1
Path=Profiles/i5izpoj2.default-release

[General]
StartWithLastProfile=1
Version=2

[BackgroundTasksProfiles]
MozillaBackgroundTask-308046B0AF4A39CB-backgroundupdate=2flhubqu.MozillaBackgroundTask-308046B0AF4A39CB-backgroundupdate"#;

    const LINUX_PROFILE: &str = r#"
[Install4F96D1932A9F858E]
Default=npf4bci2.default-release-1602083895780
Locked=1

[Profile1]
Name=default
IsRelative=1
Path=1fi7auz8.default
Default=1

[Profile0]
Name=default-release
IsRelative=1
Path=npf4bci2.default-release-1602083895780

[General]
StartWithLastProfile=1
Version=2"#;

    #[test]
    fn test_get_default_profile() {
        let profiles = tini::Ini::from_string(WINDOWS_PROFILE).unwrap();
        assert_eq!(
            PathProvider::get_default_profile_path(profiles),
            Some("Profiles/i5izpoj2.default-release".to_string())
        );

        let profiles = tini::Ini::from_string(LINUX_PROFILE).unwrap();
        assert_eq!(
            PathProvider::get_default_profile_path(profiles),
            Some("npf4bci2.default-release-1602083895780".to_string())
        );
    }
}
