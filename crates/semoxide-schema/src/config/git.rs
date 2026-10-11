//! `[git]`: the remote semoxide fetches from and publishes to.

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use toml::{Table, Value};

use super::schema::{Property, closed_table};
use super::{ConfigError, Fields};

/// `[git]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDomain {
    remote: String,
    url: Option<String>,
}

/// The keys of `[git]`.
pub(super) const KEYS: [&str; 2] = ["remote", "url"];

impl GitDomain {
    pub(super) fn defaults() -> Self {
        Self {
            remote: String::from("origin"),
            url: None,
        }
    }

    /// `git.remote`
    #[must_use]
    pub fn remote(&self) -> &str {
        &self.remote
    }

    /// `git.url`: replaces the remote's URL for publishing.
    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    pub(super) fn parse(fields: Fields) -> Result<Self, ConfigError> {
        fields.finish(&KEYS)?;
        Ok(Self::defaults())
    }

    pub(super) fn to_table(&self) -> Table {
        let mut table =
            Table::from_iter([(String::from("remote"), Value::String(self.remote.clone()))]);
        if let Some(url) = &self.url {
            table.insert(String::from("url"), Value::String(url.clone()));
        }
        table
    }
}

impl JsonSchema for GitDomain {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("GitDomain")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        closed_table(
            "The git remote semoxide fetches from and publishes to.",
            [
                Property::new(
                    "remote",
                    "The name of the remote to fetch from and push to.",
                    json_schema!({ "type": "string" }),
                ),
                Property::new(
                    "url",
                    "A URL that replaces the remote's URL for publishing.",
                    json_schema!({ "type": "string" }),
                ),
            ],
            &Self::defaults().to_table(),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::config::test_support::Schema::{self, Rejects};
    use crate::config::test_support::schema_verdict;
    use rstest::rstest;
    use semoxide_error::ErrorCode;

    use crate::codes::{CONFIG_INVALID_VALUE, CONFIG_UNKNOWN_KEY};
    use crate::config::test_support::{loaded, rejection};

    #[test]
    fn remote_and_url_default_to_origin_and_none() {
        let set = loaded(
            r#"
[git]
remote = "upstream"
url = "https://github.com/owner/repo.git"
"#,
        );
        let defaults = loaded("");

        assert_eq!(
            (
                (set.git().remote(), set.git().url()),
                (defaults.git().remote(), defaults.git().url())
            ),
            (
                ("upstream", Some("https://github.com/owner/repo.git")),
                ("origin", None)
            )
        );
    }

    /// Values are used as given: git rejects a remote named `" "` when it is looked up.
    #[rstest]
    #[case::one_character_remote(r#"git.remote = "o""#, "o")]
    #[case::blank_remote(r#"git.remote = " ""#, " ")]
    fn any_non_empty_remote_loads_as_given(#[case] text: &str, #[case] remote: &str) {
        assert_eq!(loaded(text).git().remote(), remote);
    }

    #[rstest]
    #[case::remote_empty(r#"git.remote = """#, CONFIG_INVALID_VALUE, "git.remote", Rejects)]
    #[case::remote_not_a_string("git.remote = 1", CONFIG_INVALID_VALUE, "git.remote", Rejects)]
    #[case::url_empty(r#"git.url = """#, CONFIG_INVALID_VALUE, "git.url", Rejects)]
    #[case::url_not_a_string("git.url = false", CONFIG_INVALID_VALUE, "git.url", Rejects)]
    #[case::git_unknown_key(r#"git.branch = "main""#, CONFIG_UNKNOWN_KEY, "git.branch", Rejects)]
    #[case::domain_not_a_table(r#"git = "origin""#, CONFIG_INVALID_VALUE, "git", Rejects)]
    fn invalid_config_is_rejected(
        #[case] text: &str,
        #[case] code: ErrorCode,
        #[case] path: &str,
        #[case] schema: Schema,
    ) {
        assert_eq!(rejection(text), Err((code, path.to_owned())));
        assert_eq!(schema_verdict(text), schema, "{text}");
    }
}
