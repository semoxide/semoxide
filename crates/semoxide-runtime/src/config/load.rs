//! Loading the config of a directory: discovery (`semoxide.toml`, then `.config/semoxide.toml`),
//! the layer merge with the `--set` flags, and validation (CONFIG.md §1).

use std::fmt;
use std::path::{Path, PathBuf};

use semoxide_error::{ErrorCode, ErrorInfo};
use semoxide_schema::config::{Config, ConfigError, MergeMode};
use toml::Table;

use super::flag::{FlagError, parse_flag};
use super::merge::{Layer, Merged, Source, merge};
use crate::codes::{CONFIG_INVALID_TOML, CONFIG_UNREADABLE};

/// A loaded config: the validated [`Config`], the merged table with the source of every value,
/// the file used and any other config file that was ignored.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    config: Config,
    merged: Merged,
    file: Option<PathBuf>,
    ignored: Vec<PathBuf>,
}

impl Loaded {
    /// The validated config.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The merged table and the source of every value.
    #[must_use]
    pub fn merged(&self) -> &Merged {
        &self.merged
    }

    /// The config file used, or `None` when none was found.
    #[must_use]
    pub fn file(&self) -> Option<&Path> {
        self.file.as_deref()
    }

    /// Config files found but not used, e.g. `.config/semoxide.toml` next to `semoxide.toml`.
    #[must_use]
    pub fn ignored(&self) -> &[PathBuf] {
        &self.ignored
    }
}

/// Config files looked for in a directory, in order; the first one found is used.
const CANDIDATES: [&str; 2] = ["semoxide.toml", ".config/semoxide.toml"];

/// Loads the config of `dir` with the `--set` flags, in order.
///
/// # Errors
///
/// Returns [`LoadError`] if a flag is malformed, the config file can't be read or isn't TOML,
/// or the merged config is invalid.
pub fn load(dir: &Path, flags: &[String]) -> Result<Loaded, LoadError> {
    let flags = flags
        .iter()
        .enumerate()
        .map(|(index, flag)| parse_flag(index + 1, flag))
        .collect::<Result<Vec<_>, _>>()
        .map_err(LoadError::Flag)?;
    let mut found = CANDIDATES
        .iter()
        .map(|name| dir.join(name))
        .filter(|path| path.exists());
    let file = found.next();
    let ignored: Vec<PathBuf> = found.collect();
    let layers = match &file {
        Some(path) => vec![Layer::new(Source::File(path.clone()), read(path)?)],
        None => Vec::new(),
    };
    // With a single user layer, shallow and deep give the same result; `config.merge` takes effect
    // once `extends` adds a second one.
    let merged = merge(&Config::defaults_table(), &layers, &flags, MergeMode::Deep);
    let config = Config::from_table(merged.table().clone()).map_err(LoadError::Config)?;
    Ok(Loaded {
        config,
        merged,
        file,
        ignored,
    })
}

fn read(path: &Path) -> Result<Table, LoadError> {
    let file_error = |code: ErrorCode, message: String| {
        LoadError::File(FileError {
            code,
            path: path.to_path_buf(),
            message,
        })
    };
    let text = std::fs::read_to_string(path)
        .map_err(|error| file_error(CONFIG_UNREADABLE, error.to_string()))?;
    text.parse::<Table>()
        .map_err(|error| file_error(CONFIG_INVALID_TOML, error.to_string().trim_end().to_owned()))
}

/// Why a config couldn't be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// A malformed `--set` flag.
    Flag(FlagError),
    /// The config file can't be read or isn't TOML.
    File(FileError),
    /// The merged config is invalid.
    Config(ConfigError),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flag(error) => error.fmt(f),
            Self::File(error) => error.fmt(f),
            Self::Config(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for LoadError {}

impl ErrorInfo for LoadError {
    fn code(&self) -> ErrorCode {
        match self {
            Self::Flag(error) => error.code(),
            Self::File(error) => error.code(),
            Self::Config(error) => error.code(),
        }
    }

    fn help(&self) -> Option<String> {
        match self {
            Self::Flag(error) => error.help(),
            Self::File(error) => error.help(),
            Self::Config(error) => error.help(),
        }
    }
}

/// A config file that can't be read or isn't TOML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileError {
    code: ErrorCode,
    path: PathBuf,
    message: String,
}

impl FileError {
    /// The config file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}`: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for FileError {}

impl ErrorInfo for FileError {
    fn code(&self) -> ErrorCode {
        self.code.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use rstest::rstest;
    use semoxide_error::{ErrorInfo, Location};
    use semoxide_schema::config::{Config, MergeMode};
    use tempfile::TempDir;

    use super::*;
    use crate::codes::{CONFIG_INVALID_FLAG, CONFIG_INVALID_TOML, CONFIG_UNREADABLE};
    use crate::config::Source;

    const MAIN: &str = "semoxide.toml";
    const FALLBACK: &str = ".config/semoxide.toml";

    /// A directory with the given files.
    fn dir(files: &[(&str, &str)]) -> TempDir {
        let dir = TempDir::new().unwrap();
        for (name, text) in files {
            let path = dir.path().join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, text).unwrap();
        }
        dir
    }

    fn flags(flags: &[&str]) -> Vec<String> {
        flags.iter().map(|flag| (*flag).to_owned()).collect()
    }

    /// The loaded config, failing the test with an assertion if it doesn't load.
    fn loaded(dir: &Path, set: &[&str]) -> Loaded {
        let result = load(dir, &flags(set));
        assert!(result.is_ok(), "the config should load: {result:?}");
        result.unwrap()
    }

    fn file_source(dir: &TempDir, name: &str) -> Source {
        Source::File(dir.path().join(name))
    }

    // --- Discovery ---

    #[test]
    fn no_file_gives_the_defaults() {
        let dir = dir(&[]);

        let loaded = loaded(dir.path(), &[]);

        assert_eq!(loaded.config(), &Config::default());
        assert_eq!(loaded.file(), None);
        assert_eq!(loaded.ignored(), &[] as &[PathBuf]);
        assert!(
            loaded
                .merged()
                .sources()
                .values()
                .all(|source| *source == Source::Default),
            "{:?}",
            loaded.merged().sources()
        );
    }

    #[test]
    fn semoxide_toml_is_used() {
        let dir = dir(&[(MAIN, "version.initial = \"0.1.0\"")]);

        let loaded = loaded(dir.path(), &[]);

        assert_eq!(loaded.file(), Some(dir.path().join(MAIN).as_path()));
        assert_eq!(loaded.config().version().initial().to_string(), "0.1.0");
        assert_eq!(
            loaded.merged().source("version.initial"),
            Some(&file_source(&dir, MAIN))
        );
    }

    #[test]
    fn dot_config_is_the_fallback() {
        let dir = dir(&[(FALLBACK, "version.initial = \"0.2.0\"")]);

        let loaded = loaded(dir.path(), &[]);

        assert_eq!(loaded.file(), Some(dir.path().join(FALLBACK).as_path()));
        assert_eq!(loaded.config().version().initial().to_string(), "0.2.0");
        assert_eq!(loaded.ignored(), &[] as &[PathBuf]);
    }

    #[test]
    fn semoxide_toml_wins_and_the_fallback_is_reported_as_ignored() {
        let dir = dir(&[
            (MAIN, "version.initial = \"0.1.0\""),
            (FALLBACK, "version.initial = \"0.2.0\""),
        ]);

        let loaded = loaded(dir.path(), &[]);

        assert_eq!(loaded.file(), Some(dir.path().join(MAIN).as_path()));
        assert_eq!(loaded.config().version().initial().to_string(), "0.1.0");
        assert_eq!(loaded.ignored(), [dir.path().join(FALLBACK)]);
    }

    #[test]
    fn parent_directories_are_not_searched() {
        let parent = dir(&[
            (MAIN, "version.initial = \"0.1.0\""),
            ("child/README.md", ""),
        ]);

        let loaded = loaded(&parent.path().join("child"), &[]);

        assert_eq!(loaded.file(), None);
        assert_eq!(loaded.config(), &Config::default());
    }

    // --- Flags and merge mode ---

    #[test]
    fn flags_override_the_file() {
        let dir = dir(&[(MAIN, "tags.format = \"release-{version}\"")]);

        let loaded = loaded(dir.path(), &["tags.format=v{version}-rc"]);

        assert_eq!(loaded.config().tags().format().as_str(), "v{version}-rc");
        assert_eq!(
            loaded.merged().source("tags.format"),
            Some(&Source::Flag(1))
        );
    }

    #[test]
    fn flags_apply_without_a_file() {
        let dir = dir(&[]);

        let loaded = loaded(
            dir.path(),
            &["version.initial=0.3.0", "tags.format=r{version}"],
        );

        assert_eq!(loaded.config().version().initial().to_string(), "0.3.0");
        assert_eq!(
            loaded.merged().source("version.initial"),
            Some(&Source::Flag(1))
        );
        assert_eq!(
            loaded.merged().source("tags.format"),
            Some(&Source::Flag(2))
        );
    }

    #[test]
    fn shallow_file_keeps_the_defaults() {
        let dir = dir(&[(
            MAIN,
            "[config]\nmerge = \"shallow\"\n\n[version]\ninitial = \"0.1.0\"",
        )]);

        let loaded = loaded(dir.path(), &[]);

        assert_eq!(loaded.config().config().merge(), MergeMode::Shallow);
        assert_eq!(
            loaded.config().version().zero(),
            Config::default().version().zero()
        );
        assert_eq!(
            loaded.merged().source("version.zero.breaking"),
            Some(&Source::Default)
        );
    }

    // --- Errors ---

    /// The code of the load error, and the file it names when it's about a file.
    fn failure(dir: &Path, set: &[&str]) -> Option<(String, Option<PathBuf>)> {
        load(dir, &flags(set)).err().map(|error| {
            let file = match &error {
                LoadError::File(file) => Some(file.path().to_path_buf()),
                _ => None,
            };
            (error.code().to_string(), file)
        })
    }

    #[test]
    fn invalid_toml_names_the_file_and_the_position() {
        let dir = dir(&[(MAIN, "tags.format = \"v{version}")]);

        assert_eq!(
            failure(dir.path(), &[]),
            Some((CONFIG_INVALID_TOML.to_string(), Some(dir.path().join(MAIN))))
        );
        let message = load(dir.path(), &[]).err().map(|error| error.to_string());
        assert!(
            message
                .as_deref()
                .is_some_and(|message| message.contains("line 1")),
            "{message:?}"
        );
    }

    #[test]
    fn a_directory_named_semoxide_toml_is_unreadable() {
        let dir = dir(&[("semoxide.toml/README.md", "")]);

        assert_eq!(
            failure(dir.path(), &[]),
            Some((CONFIG_UNREADABLE.to_string(), Some(dir.path().join(MAIN))))
        );
    }

    #[test]
    fn a_file_that_is_not_utf8_is_unreadable() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(MAIN), [0xff, 0xfe, 0x00]).unwrap();

        assert_eq!(
            failure(dir.path(), &[]),
            Some((CONFIG_UNREADABLE.to_string(), Some(dir.path().join(MAIN))))
        );
    }

    #[test]
    fn an_invalid_flag_fails_the_load() {
        let dir = dir(&[]);

        assert_eq!(
            failure(dir.path(), &["tags.format"]),
            Some((CONFIG_INVALID_FLAG.to_string(), None))
        );
    }

    /// The code and key path of a config validation error.
    fn invalid_config(dir: &Path) -> Option<(String, String)> {
        match load(dir, &[]).err()? {
            LoadError::Config(error) => Some((error.code().to_string(), error.path().to_owned())),
            _ => None,
        }
    }

    #[test]
    fn an_invalid_value_fails_with_its_key_path() {
        let dir = dir(&[(MAIN, "tags.format = \"release\"")]);

        assert_eq!(
            invalid_config(dir.path()),
            Some((
                String::from("config::invalid_value"),
                String::from("tags.format")
            ))
        );
    }

    #[test]
    fn an_invalid_merge_mode_is_reported_by_validation() {
        let dir = dir(&[(MAIN, "config.merge = \"partial\"")]);

        assert_eq!(
            invalid_config(dir.path()),
            Some((
                String::from("config::invalid_value"),
                String::from("config.merge")
            ))
        );
    }

    #[test]
    fn the_did_you_mean_hint_survives_loading() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("semoxide.toml"),
            "tags.formta = \"v{version}\"",
        )
        .unwrap();

        let help = load(dir.path(), &[]).err().and_then(|error| error.help());

        assert_eq!(help.as_deref(), Some("Use `tags.format` instead."));
    }

    // --- Locations ---

    /// Where the load error points, as `file:line:column` (the file relative to `dir`), `file`
    /// for a whole file, or `flag #n: <flag>`.
    fn pointer(dir: &TempDir, set: &[&str]) -> Option<String> {
        let location = load(dir.path(), &flags(set)).err()?.location()?;
        Some(match location {
            Location::File(file) => {
                let path = file
                    .path()
                    .strip_prefix(dir.path())
                    .unwrap_or_else(|_| file.path())
                    .to_string_lossy()
                    .replace('\\', "/");
                match (file.line(), file.column()) {
                    (Some(line), Some(column)) => format!("{path}:{line}:{column}"),
                    _ => path,
                }
            }
            Location::Flag(flag) => format!("flag #{}: {}", flag.position(), flag.flag()),
            other => format!("{other:?}"),
        })
    }

    #[rstest]
    #[case::invalid_value_at_the_value("[tags]\nformat = \"release\"", "semoxide.toml:2:10")]
    #[case::dotted_key_value("tags.format = \"release\"", "semoxide.toml:1:15")]
    #[case::unknown_key_at_the_key("[tags]\nformta = \"v{version}\"", "semoxide.toml:2:1")]
    #[case::unknown_domain_header("[tgas]\nformat = \"v{version}\"", "semoxide.toml:1:2")]
    #[case::unsupported_section("[packages]\na = 1", "semoxide.toml:1:2")]
    #[case::conflicting_keys(
        "[[branches.rules]]\nmaintenance = \"1.x\"\nname = \"main\"",
        "semoxide.toml:2:1"
    )]
    #[case::plugin_not_enabled("[plugins.github]\ndraft = true", "semoxide.toml:1:10")]
    #[case::array_item("steps.publish.order = [\"github\"]", "semoxide.toml:1:24")]
    #[case::inline_table_key(
        "branches.rules = [{ name = \"main\", chnanel = \"stable\" }]",
        "semoxide.toml:1:36"
    )]
    #[case::missing_key_at_its_entry(
        "branches.rules = [{ channel = \"stable\" }]",
        "semoxide.toml:1:19"
    )]
    #[case::second_table_of_an_array(
        "[[branches.rules]]\nname = \"main\"\n\n[[branches.rules]]\nchannel = \"next\"",
        "semoxide.toml:4:1"
    )]
    #[case::invalid_toml("tags.format = \"v{version}", "semoxide.toml:1:26")]
    #[case::invalid_toml_on_a_later_line("a = 1\nb = = 2", "semoxide.toml:2:5")]
    fn a_file_error_points_at_the_line_and_column(#[case] text: &str, #[case] expected: &str) {
        let dir = dir(&[(MAIN, text)]);

        assert_eq!(pointer(&dir, &[]).as_deref(), Some(expected));
    }

    #[test]
    fn the_fallback_file_is_named() {
        let dir = dir(&[(FALLBACK, "tags.format = \"release\"")]);

        assert_eq!(
            pointer(&dir, &[]).as_deref(),
            Some(".config/semoxide.toml:1:15")
        );
    }

    #[test]
    fn a_flag_elsewhere_does_not_move_a_file_error() {
        let dir = dir(&[(MAIN, "[tags]\nformat = \"release\"")]);

        assert_eq!(
            pointer(&dir, &["version.initial=0.1.0"]).as_deref(),
            Some("semoxide.toml:2:10")
        );
    }

    #[rstest]
    #[case::invalid_flag(&["tags.format"], "flag #1: --set tags.format")]
    #[case::invalid_value(&["tags.format=release"], "flag #1: --set tags.format=release")]
    #[case::second_flag(
        &["version.initial=0.1.0", "tags.format=release"],
        "flag #2: --set tags.format=release"
    )]
    #[case::unknown_key(&["tags.formta=v{version}"], "flag #1: --set tags.formta=v{version}")]
    #[case::unsupported_section(&["packages.a=1"], "flag #1: --set packages.a=1")]
    fn a_flag_error_points_at_the_flag(#[case] set: &[&str], #[case] expected: &str) {
        let dir = dir(&[]);

        assert_eq!(pointer(&dir, set).as_deref(), Some(expected));
    }

    #[test]
    fn a_flag_overriding_the_file_is_blamed() {
        let dir = dir(&[(MAIN, "tags.format = \"v{version}\"")]);

        assert_eq!(
            pointer(&dir, &["tags.format=release"]).as_deref(),
            Some("flag #1: --set tags.format=release")
        );
    }

    #[test]
    fn an_unreadable_file_points_at_the_whole_file() {
        let dir = dir(&[("semoxide.toml/README.md", "")]);

        assert_eq!(pointer(&dir, &[]).as_deref(), Some("semoxide.toml"));
    }

    #[test]
    fn the_file_location_carries_the_parsed_text() {
        let text = "[tags]\nformat = \"release\"";
        let dir = dir(&[(MAIN, text)]);

        let location = load(dir.path(), &[])
            .err()
            .and_then(|error| error.location());

        let Some(Location::File(file)) = location else {
            panic!("expected a file location: {location:?}");
        };
        assert_eq!(file.text(), Some(text));
        assert_eq!(file.span(), Some(16..25));
    }
}
