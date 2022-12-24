use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

pub(crate) struct PathProvider {
    _base_dir: PathBuf,
    _profile: Option<OsString>,
    profile_dir: PathBuf,
}

impl PathProvider {
    /// Create a new path provider for the given profile.
    /// If no profile is given, the default profile is queried and used.
    pub(crate) fn new<R: AsRef<Path>, P: AsRef<OsStr>>(root_dir: R, profile: Option<P>) -> Self {
        let base_dir = root_dir.as_ref().to_owned();

        Self {
            _base_dir: base_dir.clone(),
            profile_dir: if let Some(profile) = profile.as_ref().map(|p| p.as_ref()) {
                if cfg!(any(windows, target_os = "macos")) {
                    base_dir.join("Profiles").join(profile)
                } else {
                    base_dir.join(profile)
                }
            } else {
                base_dir
            },
            _profile: profile.map(|p| p.as_ref().into()),
        }
    }

    /// Returns a path provider for the default profile.
    ///
    /// # Panics
    ///
    /// This function panics if no default profile can be found.
    pub(crate) fn default_profile() -> Self {
        let root_dir = if cfg!(any(windows, target_os = "macos")) {
            dirs_next::config_dir().unwrap()
        } else {
            dirs_next::home_dir().unwrap()
        }
        .join(if cfg!(any(windows, target_os = "macos")) {
            "Mozilla/Firefox"
        } else {
            ".mozilla/firefox"
        });

        let profiles = tini::Ini::from_file(&root_dir.join("profiles.ini"))
            .expect("Cannot parse Firefox profiles.ini file");

        let default = PathProvider::get_default_profile(profiles)
            .expect("Cannot get Firefox default profile");

        Self::new(root_dir, Some(default))
    }

    /// Get the default profile's name from the profiles.ini file.
    /// It selects the profile which is in the first `Install$INSTALL_HASH$` section found,
    /// or the first `Profile` section with `Default=1` if no `Install$INSTALL_HASH$` section is found.
    fn get_default_profile(profile_config: tini::Ini) -> Option<String> {
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
    pub(crate) fn cookies_database(&self) -> PathBuf {
        self.profile_dir.join("cookies.sqlite")
    }
}
