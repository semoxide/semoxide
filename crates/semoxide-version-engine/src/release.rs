//! [`Releases`]: the last release among a branch's tags, and the version after it.

mod error;

use std::fmt;

use semoxide_schema::config::{Level, VersionDomain};

pub use error::ReleaseError;

use crate::{CommitId, ReleaseBranch, ReleaseRanges, TagPattern, Version};

/// The release tags of a branch: its last release, and its highest prerelease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Releases {
    last: Option<LastRelease>,
    /// The tag and version of the highest prerelease.
    highest_prerelease: Option<(String, Version)>,
}

/// The highest release, not prerelease, among a branch's tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastRelease {
    version: Version,
    tag: String,
    commit: CommitId,
}

/// The version to release next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextRelease {
    version: Version,
    warning: Option<FirstReleaseWarning>,
}

/// The first release is lower than a prerelease the branch already has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstReleaseWarning {
    version: Version,
    tag: String,
}

impl Releases {
    /// The releases among `tags`, the tags reachable from the branch with their commits. Tags
    /// `pattern` doesn't parse are ignored.
    ///
    /// # Errors
    ///
    /// Returns [`ReleaseError`] when tags of the last release's version are on different
    /// commits.
    pub fn find<'a>(
        tags: impl IntoIterator<Item = (&'a str, &'a CommitId)>,
        pattern: &TagPattern,
    ) -> Result<Self, ReleaseError> {
        let mut tags: Vec<(&str, &CommitId, Version)> = tags
            .into_iter()
            .filter_map(|(name, commit)| Some((name, commit, pattern.parse(name)?)))
            .collect();
        tags.sort_by_key(|&(name, ..)| name);
        let (prereleases, releases): (Vec<_>, Vec<_>) = tags
            .into_iter()
            .partition(|(.., version)| !version.as_semver().pre.is_empty());
        // Reversed, so the first by name wins a tie.
        let highest_prerelease = prereleases
            .into_iter()
            .rev()
            .max_by(|(.., a), (.., b)| a.cmp(b))
            .map(|(name, _, version)| (name.to_owned(), version));
        let Some((tag, commit, version)) =
            releases.iter().rev().max_by(|(.., a), (.., b)| a.cmp(b))
        else {
            return Ok(Self {
                last: None,
                highest_prerelease,
            });
        };
        let same: Vec<&(&str, &CommitId, Version)> = releases
            .iter()
            .filter(|(.., other)| other == version)
            .collect();
        if same.iter().any(|&&(_, other, _)| other != *commit) {
            let names = same.iter().map(|&&(name, ..)| name.to_owned()).collect();
            return Err(ReleaseError::duplicate_tags(version, names));
        }
        Ok(Self {
            last: Some(LastRelease {
                version: version.clone(),
                tag: (*tag).to_owned(),
                commit: (*commit).clone(),
            }),
            highest_prerelease,
        })
    }

    /// The last release, or `None` before the first one.
    #[must_use]
    pub fn last(&self) -> Option<&LastRelease> {
        self.last.as_ref()
    }

    /// The version a `level` release from `branch` gets: `version.initial` for the first
    /// release, the last release bumped by `level` otherwise, mapped by `version.zero` while
    /// the last release is 0.x.
    ///
    /// # Errors
    ///
    /// Returns [`ReleaseError`] when the version is outside the branch's range in `ranges`,
    /// or its number would exceed `u64::MAX`.
    pub fn next(
        &self,
        level: Level,
        config: &VersionDomain,
        ranges: &ReleaseRanges<'_>,
        branch: &ReleaseBranch,
    ) -> Result<NextRelease, ReleaseError> {
        let releases: Vec<_> = ranges.releases().collect();
        let Some((position, &(_, range))) = releases
            .iter()
            .enumerate()
            .find(|(_, (release, _))| release.name() == branch.name())
        else {
            return Err(ReleaseError::not_release_branch(branch.name()));
        };
        let next = match &self.last {
            None => self.first(config),
            Some(last) => {
                let level = if last.version.as_semver().major == 0 {
                    zero_level(level, config)
                } else {
                    level
                };
                NextRelease {
                    version: last.version.bump(level).map_err(ReleaseError::overflow)?,
                    warning: None,
                }
            }
        };
        if range.contains(&next.version) {
            return Ok(next);
        }
        if !range.contains(range.min()) {
            let higher = releases
                .get(position + 1)
                .map(|(release, _)| release.name());
            return Err(ReleaseError::empty_range(branch.name(), range, higher));
        }
        let fits = releases
            .iter()
            .find(|(release, other)| {
                release.name() != branch.name() && other.contains(&next.version)
            })
            .map(|(release, _)| release.name());
        Err(ReleaseError::out_of_range(
            &next.version,
            branch.name(),
            range,
            fits,
        ))
    }

    /// `version.initial`, with a warning when a prerelease tag is higher.
    fn first(&self, config: &VersionDomain) -> NextRelease {
        let initial = config.initial();
        let version = Version::from_parts(
            initial.major,
            initial.minor,
            initial.patch,
            initial.pre.clone(),
        );
        let warning = self
            .highest_prerelease
            .as_ref()
            .filter(|(_, prerelease)| &version < prerelease)
            .map(|(tag, _)| FirstReleaseWarning {
                version: version.clone(),
                tag: tag.clone(),
            });
        NextRelease { version, warning }
    }
}

/// The level `level` gets while the version is 0.x.
fn zero_level(level: Level, config: &VersionDomain) -> Level {
    let zero = config.zero();
    match level {
        Level::Major => zero.breaking(),
        Level::Minor => zero.feature(),
        Level::Patch => zero.fix(),
    }
}

impl LastRelease {
    /// The released version.
    #[must_use]
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// The tag's name.
    #[must_use]
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// The commit the tag points at.
    #[must_use]
    pub fn commit(&self) -> &CommitId {
        &self.commit
    }
}

impl NextRelease {
    /// The version.
    #[must_use]
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Set when the first release is lower than an existing prerelease.
    #[must_use]
    pub fn warning(&self) -> Option<&FirstReleaseWarning> {
        self.warning.as_ref()
    }
}

impl FirstReleaseWarning {
    /// The tag of the higher prerelease.
    #[must_use]
    pub fn tag(&self) -> &str {
        &self.tag
    }
}

impl fmt::Display for FirstReleaseWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the first release `{}` is lower than the existing tag `{}`",
            self.version, self.tag
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rstest::rstest;
    use semoxide_error::{ErrorCode, ErrorInfo};
    use semoxide_schema::config::Config;

    use super::*;
    use crate::BranchModel;
    use crate::codes::{VERSION_DUPLICATE_TAGS, VERSION_OUT_OF_RANGE, VERSION_OVERFLOW};

    /// A branch's tags as tag name and commit, a commit written as one hex digit.
    type Tags<'a> = &'a [(&'a str, char)];

    type Failure = (ErrorCode, String, String);

    fn sha(commit: char) -> CommitId {
        CommitId::try_from(commit.to_string().repeat(40).as_str()).unwrap()
    }

    fn failure<T>(result: Result<T, ReleaseError>) -> Result<T, Failure> {
        result.map_err(|error| {
            (
                error.code(),
                error.to_string(),
                error.help().unwrap_or_default(),
            )
        })
    }

    fn find_with(tags: Tags<'_>, pattern: &TagPattern) -> Result<Releases, ReleaseError> {
        let tags: Vec<(&str, CommitId)> = tags
            .iter()
            .map(|&(name, commit)| (name, sha(commit)))
            .collect();
        Releases::find(tags.iter().map(|(name, commit)| (*name, commit)), pattern)
    }

    /// The last release among `tags` (format `v{version}`) as version, tag and commit.
    fn last(tags: Tags<'_>) -> Result<Option<(String, String, char)>, Failure> {
        let pattern = TagPattern::new(Config::default().tags().format());
        let releases = failure(find_with(tags, &pattern))?;
        Ok(releases.last().map(|last| {
            (
                last.version().to_string(),
                last.tag().to_owned(),
                last.commit().as_str().chars().next().unwrap(),
            )
        }))
    }

    fn release(version: &str, tag: &str, commit: char) -> (String, String, char) {
        (version.to_owned(), tag.to_owned(), commit)
    }

    /// The next version of a `level` release from `current`, with its warning, for the config
    /// `config` and each existing branch with its tags.
    fn next_on(
        config: &str,
        branches: &[(&str, Tags<'_>)],
        current: &str,
        level: Level,
    ) -> Result<(String, Option<String>), Failure> {
        let config = Config::from_table(config.parse().unwrap()).unwrap();
        let pattern = TagPattern::new(config.tags().format());
        let names: Vec<&str> = branches.iter().map(|(name, _)| *name).collect();
        let model = BranchModel::new(config.branches().rules(), &names).unwrap();
        let versions: HashMap<String, Vec<Version>> = branches
            .iter()
            .map(|(name, tags)| {
                let versions = tags.iter().filter_map(|(tag, _)| pattern.parse(tag));
                ((*name).to_owned(), versions.collect())
            })
            .collect();
        let initial: Version = config.version().initial().to_string().parse().unwrap();
        let ranges = model.ranges(&versions, &initial);
        let (_, tags) = branches.iter().find(|(name, _)| *name == current).unwrap();
        let branch = model
            .release()
            .iter()
            .find(|branch| branch.name() == current)
            .unwrap();

        let releases = failure(find_with(tags, &pattern))?;
        let next = failure(releases.next(level, config.version(), &ranges, branch))?;
        Ok((
            next.version().to_string(),
            next.warning().map(ToString::to_string),
        ))
    }

    /// The next version of a `level` release from `main`, the only branch.
    fn next(config: &str, tags: Tags<'_>, level: Level) -> Result<String, Failure> {
        next_on(config, &[("main", tags)], "main", level).map(|(version, _)| version)
    }

    fn levels(config: &str, tags: Tags<'_>) -> [Result<String, Failure>; 3] {
        [Level::Major, Level::Minor, Level::Patch].map(|level| next(config, tags, level))
    }

    fn ok(versions: [&str; 3]) -> [Result<String, Failure>; 3] {
        versions.map(|version| Ok(version.to_owned()))
    }

    // --- The last release ---

    #[test]
    fn a_branch_without_tags_has_no_last_release() {
        assert_eq!(last(&[]), Ok(None));
    }

    #[test]
    fn the_highest_version_wins_not_the_order() {
        assert_eq!(
            last(&[("v1.0.0", 'a'), ("v2.0.0", 'b'), ("v1.5.0", 'c')]),
            Ok(Some(release("2.0.0", "v2.0.0", 'b')))
        );
    }

    #[rstest]
    #[case::above_a_release(&[("v1.0.0", 'a'), ("v2.0.0-beta.1", 'b')], Ok(Some(release("1.0.0", "v1.0.0", 'a'))))]
    #[case::above_a_release_of_its_line(&[("v1.0.0", 'a'), ("v1.1.0-rc.1", 'b')], Ok(Some(release("1.0.0", "v1.0.0", 'a'))))]
    #[case::only_prereleases(&[("v1.0.0-beta.1", 'a'), ("v1.0.0-beta.2", 'b')], Ok(None))]
    fn a_prerelease_is_not_a_release(
        #[case] tags: Tags<'_>,
        #[case] expected: Result<Option<(String, String, char)>, Failure>,
    ) {
        assert_eq!(last(tags), expected);
    }

    #[test]
    fn tags_that_dont_match_the_format_are_ignored() {
        assert_eq!(
            last(&[
                ("v1.1.0", 'a'),
                ("2.0.0", 'b'),
                ("vv3.0.0", 'c'),
                ("vfoo", 'd'),
                ("v1.0", 'e'),
                ("release-1", 'f'),
            ]),
            Ok(Some(release("1.1.0", "v1.1.0", 'a')))
        );
    }

    #[test]
    fn build_metadata_stays_in_the_tag_only() {
        assert_eq!(
            last(&[("v1.0.0", 'a'), ("v1.1.0+build.5", 'b')]),
            Ok(Some(release("1.1.0", "v1.1.0+build.5", 'b')))
        );
    }

    #[test]
    fn two_versions_on_one_commit_give_the_higher() {
        assert_eq!(
            last(&[("v1.0.0", 'a'), ("v1.0.1", 'a')]),
            Ok(Some(release("1.0.1", "v1.0.1", 'a')))
        );
    }

    #[test]
    fn tags_of_one_version_on_one_commit_give_the_first_by_name() {
        assert_eq!(
            last(&[("v1.0.0+b", 'a'), ("v0.9.0", 'b'), ("v1.0.0", 'a')]),
            Ok(Some(release("1.0.0", "v1.0.0", 'a')))
        );
    }

    #[test]
    fn tags_of_the_last_version_on_different_commits_are_an_error() {
        assert_eq!(
            last(&[("v1.2.0+b", 'b'), ("v1.0.0", 'c'), ("v1.2.0", 'a')]),
            Err((
                VERSION_DUPLICATE_TAGS,
                String::from(
                    "tags `v1.2.0`, `v1.2.0+b` hold the last release `1.2.0` on different commits"
                ),
                String::from("Delete the tags on the wrong commits, so one commit holds `1.2.0`.")
            ))
        );
    }

    #[rstest]
    #[case::older_release(&[("v1.0.0", 'a'), ("v1.0.0+b", 'b'), ("v1.2.0", 'c')], Ok(Some(release("1.2.0", "v1.2.0", 'c'))))]
    #[case::prerelease(&[("v1.0.0", 'a'), ("v2.0.0-rc.1", 'b'), ("v2.0.0-rc.1+x", 'c')], Ok(Some(release("1.0.0", "v1.0.0", 'a'))))]
    fn other_duplicates_are_ignored(
        #[case] tags: Tags<'_>,
        #[case] expected: Result<Option<(String, String, char)>, Failure>,
    ) {
        assert_eq!(last(tags), expected);
    }

    #[test]
    fn versions_compare_by_precedence_not_as_text() {
        assert_eq!(
            last(&[("v9.0.0", 'a'), ("v10.0.0", 'b')]),
            Ok(Some(release("10.0.0", "v10.0.0", 'b')))
        );
    }

    #[test]
    fn any_tag_of_the_last_version_on_another_commit_is_an_error() {
        assert_eq!(
            last(&[("v1.2.0", 'a'), ("v1.2.0+b", 'b'), ("v1.2.0+c", 'a')])
                .map_err(|(code, ..)| code),
            Err(VERSION_DUPLICATE_TAGS)
        );
    }

    #[test]
    fn a_patch_on_another_commit_is_not_a_duplicate() {
        assert_eq!(
            last(&[("v1.2.0", 'a'), ("v1.2.1", 'b')]),
            Ok(Some(release("1.2.1", "v1.2.1", 'b')))
        );
    }

    // --- The next version ---

    #[rstest]
    #[case::default("", ["1.0.0", "1.0.0", "1.0.0"])]
    #[case::configured(r#"version.initial = "0.1.0""#, ["0.1.0", "0.1.0", "0.1.0"])]
    #[case::all_zero(r#"version.initial = "0.0.0""#, ["0.0.0", "0.0.0", "0.0.0"])]
    fn the_first_release_is_the_initial_version_at_any_level(
        #[case] config: &str,
        #[case] expected: [&str; 3],
    ) {
        assert_eq!(levels(config, &[]), ok(expected));
    }

    #[test]
    fn prerelease_tags_dont_count_as_a_first_release() {
        assert_eq!(
            levels("", &[("v1.0.0-beta.1", 'a')]),
            ok(["1.0.0", "1.0.0", "1.0.0"])
        );
    }

    #[rstest]
    #[case::release(&[("v1.2.3", 'a')], ["2.0.0", "1.3.0", "1.2.4"])]
    #[case::highest(&[("v2.0.0", 'a'), ("v1.5.0", 'b')], ["3.0.0", "2.1.0", "2.0.1"])]
    #[case::prerelease_ignored(&[("v1.0.0", 'a'), ("v1.1.0-rc.1", 'b')], ["2.0.0", "1.1.0", "1.0.1"])]
    fn the_last_release_is_bumped_by_the_level(
        #[case] tags: Tags<'_>,
        #[case] expected: [&str; 3],
    ) {
        assert_eq!(levels("", tags), ok(expected));
    }

    #[test]
    fn the_initial_version_is_ignored_once_there_is_a_release() {
        assert_eq!(
            levels(r#"version.initial = "2.0.0""#, &[("v1.4.0", 'a')]),
            ok(["2.0.0", "1.5.0", "1.4.1"])
        );
    }

    #[rstest]
    #[case::zero_minor(&[("v0.3.0", 'a')], ["0.4.0", "0.3.1", "0.3.1"])]
    #[case::zero_zero(&[("v0.0.5", 'a')], ["0.1.0", "0.0.6", "0.0.6"])]
    fn on_0_x_breaking_is_minor_and_the_rest_patch(
        #[case] tags: Tags<'_>,
        #[case] expected: [&str; 3],
    ) {
        assert_eq!(levels("", tags), ok(expected));
    }

    #[test]
    fn the_0_x_levels_are_configurable() {
        assert_eq!(
            levels(
                "[version.zero]\nbreaking = \"major\"\nfeature = \"minor\"\nfix = \"minor\"",
                &[("v0.3.0", 'a')]
            ),
            ok(["1.0.0", "0.4.0", "0.4.0"])
        );
    }

    #[test]
    fn the_0_x_levels_follow_the_last_release_not_the_initial_version() {
        assert_eq!(
            levels(r#"version.initial = "0.1.0""#, &[("v1.0.0", 'a')]),
            ok(["2.0.0", "1.1.0", "1.0.1"])
        );
    }

    #[test]
    fn the_0_x_levels_end_with_1_0_0() {
        assert_eq!(
            levels("", &[("v0.3.0", 'a'), ("v1.0.0", 'b')]),
            ok(["2.0.0", "1.1.0", "1.0.1"])
        );
    }

    #[test]
    fn a_first_release_below_a_prerelease_tag_comes_with_a_warning() {
        let result = next_on(
            r#"version.initial = "0.1.0""#,
            &[("main", &[("v1.0.0-rc.2", 'b'), ("v1.0.0-rc.1", 'a')])],
            "main",
            Level::Patch,
        );

        assert_eq!(
            result,
            Ok((
                String::from("0.1.0"),
                Some(String::from(
                    "the first release `0.1.0` is lower than the existing tag `v1.0.0-rc.2`"
                ))
            ))
        );
    }

    #[test]
    fn the_warning_names_the_tag() {
        let config = Config::from_table(r#"version.initial = "0.1.0""#.parse().unwrap()).unwrap();
        let pattern = TagPattern::new(config.tags().format());
        let model = BranchModel::new(config.branches().rules(), &["main"]).unwrap();
        let initial: Version = "0.1.0".parse().unwrap();
        let ranges = model.ranges(&HashMap::new(), &initial);

        let next = find_with(&[("v1.0.0-rc.1", 'a')], &pattern)
            .unwrap()
            .next(Level::Patch, config.version(), &ranges, &model.release()[0])
            .unwrap();

        assert_eq!(
            next.warning().map(FirstReleaseWarning::tag),
            Some("v1.0.0-rc.1")
        );
    }

    #[rstest]
    #[case::first_release_above("", &[("v1.0.0-rc.1", 'a')], "1.0.0")]
    #[case::after_a_release(
        r#"version.initial = "0.1.0""#,
        &[("v0.5.0", 'a'), ("v1.0.0-rc.1", 'b')],
        "0.5.1"
    )]
    fn no_warning_otherwise(#[case] config: &str, #[case] tags: Tags<'_>, #[case] version: &str) {
        assert_eq!(
            next_on(config, &[("main", tags)], "main", Level::Patch),
            Ok((version.to_owned(), None))
        );
    }

    #[test]
    fn a_bump_beyond_u64_is_an_error() {
        assert_eq!(
            next("", &[("v18446744073709551615.0.0", 'a')], Level::Major),
            Err((
                VERSION_OVERFLOW,
                String::from(
                    "`18446744073709551615.0.0` can't get a major bump: the number would exceed \
                     18446744073709551615"
                ),
                String::from("No higher version exists at this level; release a lower level.")
            ))
        );
    }

    /// The warning of a first patch release from `main` with `initial`.
    fn warning(tags: Tags<'_>, initial: &str) -> Option<String> {
        next_on(
            &format!("version.initial = \"{initial}\""),
            &[("main", tags)],
            "main",
            Level::Patch,
        )
        .unwrap()
        .1
    }

    #[test]
    fn the_warning_takes_the_first_tag_by_name_on_a_tie() {
        assert_eq!(
            warning(&[("v1.0.0-rc.1+b", 'b'), ("v1.0.0-rc.1", 'a')], "0.1.0"),
            Some(String::from(
                "the first release `0.1.0` is lower than the existing tag `v1.0.0-rc.1`"
            ))
        );
    }

    #[test]
    fn the_warning_takes_the_highest_prerelease_by_precedence() {
        assert_eq!(
            warning(&[("v1.0.0-rc.10", 'a'), ("v1.0.0-rc.9", 'b')], "0.1.0"),
            Some(String::from(
                "the first release `0.1.0` is lower than the existing tag `v1.0.0-rc.10`"
            ))
        );
    }

    #[test]
    fn a_first_release_below_a_prerelease_of_its_major_comes_with_a_warning() {
        assert_eq!(
            warning(&[("v1.1.0-rc.1", 'a')], "1.0.0"),
            Some(String::from(
                "the first release `1.0.0` is lower than the existing tag `v1.1.0-rc.1`"
            ))
        );
    }

    #[test]
    fn feature_and_fix_map_separately_on_0_x() {
        assert_eq!(
            levels(
                "[version.zero]
feature = \"minor\"
fix = \"patch\"",
                &[("v0.3.0", 'a')]
            ),
            ok(["0.4.0", "0.4.0", "0.3.1"])
        );
    }

    // --- The range check ---

    const MAIN_AND_NEXT: [(&str, Tags<'static>); 2] = [
        ("main", &[("v1.0.0", 'a')]),
        ("next", &[("v1.0.0", 'a'), ("v1.1.0", 'b')]),
    ];

    #[test]
    fn a_version_inside_the_range_is_released() {
        assert_eq!(
            [
                next_on("", &MAIN_AND_NEXT, "main", Level::Patch),
                next_on("", &MAIN_AND_NEXT, "next", Level::Minor),
            ],
            [
                Ok((String::from("1.0.1"), None)),
                Ok((String::from("1.2.0"), None))
            ]
        );
    }

    #[test]
    fn a_version_outside_the_range_names_the_branch_that_fits() {
        assert_eq!(
            next_on("", &MAIN_AND_NEXT, "main", Level::Minor),
            Err((
                VERSION_OUT_OF_RANGE,
                String::from(
                    "the release `1.1.0` on branch `main` is outside its range `>=1.0.0 <1.1.0`"
                ),
                String::from("Release it from `next`, whose range contains it.")
            ))
        );
    }

    #[test]
    fn a_version_no_branch_fits_gets_a_general_hint() {
        let branches: [(&str, Tags<'_>); 2] = [
            ("main", &[("v1.0.0", 'a')]),
            ("next", &[("v1.0.0", 'a'), ("v1.0.1", 'b'), ("v1.1.0", 'c')]),
        ];

        assert_eq!(
            next_on("", &branches, "main", Level::Patch),
            Err((
                VERSION_OUT_OF_RANGE,
                String::from(
                    "the release `1.0.1` on branch `main` is outside its range `>=1.0.0 <1.0.1`"
                ),
                String::from("Release it from a branch whose range contains it.")
            ))
        );
    }

    #[test]
    fn an_empty_range_says_so() {
        let branches: [(&str, Tags<'_>); 2] = [("main", &[]), ("next", &[("v1.0.0", 'a')])];

        assert_eq!(
            next_on("", &branches, "main", Level::Patch),
            Err((
                VERSION_OUT_OF_RANGE,
                String::from(
                    "branch `main` can't release any version: its range `>=1.0.0 <1.0.0` is empty"
                ),
                String::from("Merge `next` into `main`, or release from `next`.")
            ))
        );
    }

    #[test]
    fn a_branch_from_another_model_is_an_error() {
        let config = Config::default();
        let both = BranchModel::new(config.branches().rules(), &["main", "next"]).unwrap();
        let main_only = BranchModel::new(config.branches().rules(), &["main"]).unwrap();
        let initial: Version = "1.0.0".parse().unwrap();
        let ranges = main_only.ranges(&HashMap::new(), &initial);
        let releases = find_with(&[], &TagPattern::new(config.tags().format())).unwrap();

        let next = releases.next(Level::Patch, config.version(), &ranges, &both.release()[1]);

        assert_eq!(
            failure(next),
            Err((
                VERSION_OUT_OF_RANGE,
                String::from("branch `next` is not a release branch of these ranges"),
                String::from("Pass a release branch of the branch model the ranges come from.")
            ))
        );
    }

    #[test]
    fn an_empty_range_names_the_next_branch() {
        let branches: [(&str, Tags<'_>); 3] = [
            ("main", &[]),
            ("next", &[("v1.0.0", 'a')]),
            ("next-major", &[("v1.0.0", 'a'), ("v2.0.0", 'b')]),
        ];

        assert_eq!(
            next_on("", &branches, "main", Level::Patch).map_err(|(_, _, help)| help),
            Err(String::from(
                "Merge `next` into `main`, or release from `next`."
            ))
        );
    }

    #[test]
    fn a_range_with_its_bound_below_its_floor_is_empty() {
        let branches: [(&str, Tags<'_>); 2] = [("main", &[]), ("next", &[("v1.0.0", 'a')])];

        assert_eq!(
            next_on(
                r#"version.initial = "2.0.0""#,
                &branches,
                "main",
                Level::Patch
            ),
            Err((
                VERSION_OUT_OF_RANGE,
                String::from(
                    "branch `main` can't release any version: its range `>=2.0.0 <1.0.0` is empty"
                ),
                String::from("Merge `next` into `main`, or release from `next`.")
            ))
        );
    }

    #[test]
    fn the_hint_looks_past_the_next_branch() {
        let branches: [(&str, Tags<'_>); 3] = [
            ("main", &[("v1.0.0", 'a')]),
            ("next", &[("v1.0.0", 'a'), ("v1.1.0", 'b')]),
            (
                "next-major",
                &[("v1.0.0", 'a'), ("v1.1.0", 'b'), ("v2.0.0", 'c')],
            ),
        ];

        assert_eq!(
            next_on("", &branches, "main", Level::Major).map_err(|(_, _, help)| help),
            Err(String::from(
                "Release it from `next-major`, whose range contains it."
            ))
        );
    }

    /// A prerelease bound lets ranges overlap: the hint names the lowest branch that fits.
    #[test]
    fn the_hint_names_the_first_branch_that_fits() {
        let branches: [(&str, Tags<'_>); 3] = [
            ("main", &[("v1.0.0", 'a')]),
            ("next", &[("v1.0.0", 'a'), ("v1.1.0", 'b')]),
            (
                "next-major",
                &[("v1.0.0", 'a'), ("v1.1.0", 'b'), ("v3.0.0-rc.1", 'c')],
            ),
        ];

        assert_eq!(
            next_on("", &branches, "main", Level::Minor).map_err(|(_, _, help)| help),
            Err(String::from(
                "Release it from `next`, whose range contains it."
            ))
        );
    }

    #[test]
    fn an_unbounded_range_is_shown_without_a_bound() {
        let branches: [(&str, Tags<'_>); 2] =
            [("main", &[("v1.5.0", 'a')]), ("next", &[("v1.0.0", 'b')])];

        assert_eq!(
            next_on("", &branches, "next", Level::Patch).map_err(|(_, message, _)| message),
            Err(String::from(
                "the release `1.0.1` on branch `next` is outside its range `>=1.5.0`"
            ))
        );
    }
}
