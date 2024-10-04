use std::{
    ffi::OsString,
    fs::{copy, create_dir_all, remove_dir_all},
    io::{stderr, Write},
    path::PathBuf,
};

use bpaf::Bpaf;
use cargo_metadata::{Message, MetadataCommand};
use xshell::{cmd, Shell};

#[derive(Debug, Clone)]
enum ArchiveFormat {
    TarGz,
    Zip,
}

fn default_format() -> ArchiveFormat {
    if cfg!(windows) {
        ArchiveFormat::Zip
    } else {
        ArchiveFormat::TarGz
    }
}

impl std::str::FromStr for ArchiveFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tgz" => Ok(ArchiveFormat::TarGz),
            "zip" => Ok(ArchiveFormat::Zip),
            _ => Err(format!(
                "'{s}' is not one of the supported archive formats (tar.gz, zip)"
            )),
        }
    }
}

impl std::fmt::Display for ArchiveFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveFormat::TarGz => write!(f, "tar.gz"),
            ArchiveFormat::Zip => write!(f, "zip"),
        }
    }
}

fn comma_separated<T: std::str::FromStr>(s: Option<String>) -> Result<Vec<T>, T::Err> {
    s.map_or_else(
        || Ok(Vec::new()),
        |s| s.split(',').map(|s| s.parse()).collect(),
    )
}

#[derive(Debug, Clone, Bpaf)]
enum Subcommand {
    /// Create an archive with the given format
    /// containing the binary for the current platform
    /// and the given files.
    #[bpaf(command)]
    Dist {
        /// The archive format to use
        ///
        /// Supported formats: tgz, zip
        #[bpaf(long, fallback(default_format()))]
        format: ArchiveFormat,

        /// The profile to build for
        profile: Option<String>,

        /// The target triple to build for
        target: Option<String>,

        /// Additional features to enable
        #[bpaf(argument::<String>("features"), optional, parse(comma_separated))]
        features: Vec<String>,

        /// Archive name
        ///
        /// If not provided, it will have the form gateau.target.format
        #[bpaf(short, long)]
        name: Option<OsString>,

        /// Additional files to include in the archive
        #[bpaf(positional("FILES"), many)]
        files: Vec<PathBuf>,
    },
}

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version)]
struct Args {
    #[bpaf(external)]
    subcommand: Subcommand,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let args = args().run();

    match args.subcommand {
        Subcommand::Dist {
            format,
            profile,
            target,
            features,
            name,
            files,
        } => dist_subcommand(format, profile, target, features, name, files)?,
    }

    Ok(())
}

fn dist_subcommand(
    format: ArchiveFormat,
    profile: Option<String>,
    target: Option<String>,
    features: Vec<String>,
    name: Option<OsString>,
    files: Vec<PathBuf>,
) -> color_eyre::Result<()> {
    const BIN_NAME: &str = "gateau";
    const TARGET_BIN_NAME: &str = "cli";

    let target = target.unwrap_or_else(|| {
        let sh = Shell::new().unwrap();
        let output = cmd!(sh, "rustc -vV").ignore_stderr().read().unwrap();
        let mut lines = output.lines();
        let target = lines
            .find(|l| l.starts_with("host: "))
            .unwrap()
            .split(' ')
            .nth(1)
            .unwrap();
        target.to_string()
    });

    let target_path = MetadataCommand::new().exec()?.target_directory;

    // Build phase
    let profile = profile.unwrap_or_else(|| "release".to_string());

    let sh = Shell::new()?;

    let mut c_build = cmd!(sh, "cargo build --verbose --locked --message-format=json")
        .args(["--bin", TARGET_BIN_NAME])
        .args(["--target", &target])
        .args(["--profile", &profile]);

    if !features.is_empty() {
        c_build = c_build.args(["--features", &features.join(",")]);
    }

    eprintln!("$ {}", c_build);
    let output = c_build.read()?;
    let mut messages = Message::parse_stream(output.as_bytes());

    let bin_path = loop {
        if let Some(message) = messages.next() {
            match message? {
                Message::CompilerArtifact(artifact) => {
                    if artifact.target.kind.iter().any(|k| k == "bin")
                        && artifact.target.name == TARGET_BIN_NAME
                    {
                        break Ok(artifact.filenames.into_iter().next().unwrap());
                    }
                }
                _ => {}
            }
        } else {
            break Err(color_eyre::eyre::eyre!("Could not find binary path"));
        }
    }?;

    // Archive phase

    let mut archive_name = name.unwrap_or_else(|| OsString::from(BIN_NAME));
    archive_name.push(format!(".{}", target));
    archive_name.push(format!(".{}", format));

    let parent_dir = target_path.join("dist");
    let base_archive_dir = parent_dir.join(BIN_NAME);

    remove_dir_all(&parent_dir).or_else(|err| match err {
        e if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(err),
    })?;
    create_dir_all(&base_archive_dir)?;
    copy(bin_path, base_archive_dir.join(BIN_NAME))?;
    for file in files.iter() {
        copy(
            file,
            base_archive_dir
                .as_std_path()
                .join(file.file_name().unwrap()),
        )?;
    }

    let archive_path = parent_dir.as_std_path().join(&archive_name);

    sh.change_dir(parent_dir.as_std_path());

    let output = match format {
        ArchiveFormat::TarGz => cmd!(sh, "tar czvf {archive_path} -C {parent_dir} {BIN_NAME}"),
        ArchiveFormat::Zip => cmd!(sh, "7z a {archive_path} {base_archive_dir}'/*'"),
    }
    .output()?;

    stderr().lock().write_all(&output.stderr)?;
    stderr().lock().write_all(&output.stdout)?;

    if !output.status.success() {
        return Err(color_eyre::eyre::eyre!("Failed to create archive"));
    }

    println!("{}", archive_path.display());

    Ok(())
}
