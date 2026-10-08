use std::fs;
use std::path::{Path, PathBuf};

use semoxide_error::ErrorInfo;
use semoxide_schema::config::{Config, MergeMode};
use tempfile::TempDir;

use super::{LoadError, Loaded, load};
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
