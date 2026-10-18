//! [`BranchModel`]: the existing branches sorted into release, prerelease and maintenance
//! branches by `branches.rules`.

mod error;
mod ranges;

use std::collections::{HashMap, HashSet};
use std::fmt;

use semoxide_schema::config::{BranchRule, Channel as RuleChannel, Prerelease};

use crate::{PrereleaseId, Range, Version};

pub use error::BranchError;
pub use ranges::{Current, ReleaseRanges};

/// The existing branches that `branches.rules` claims. Rules are taken in order; each claims
/// the branches it matches that no earlier rule claimed, in byte order of their names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchModel {
    release: Vec<ReleaseBranch>,
    prerelease: Vec<PrereleaseBranch>,
    maintenance: Vec<MaintenanceBranch>,
    overlaps: Vec<Overlap>,
}

impl BranchModel {
    /// Sorts `branches`, the names of the existing branches, by `rules`.
    ///
    /// # Errors
    ///
    /// Returns [`BranchError`] when there are not 1 to 3 release branches, a prerelease branch
    /// has an invalid identifier, two prerelease branches share one, or two maintenance branches
    /// cover the same range.
    pub fn new(rules: &[BranchRule], branches: &[&str]) -> Result<Self, BranchError> {
        let mut names = branches.to_vec();
        names.sort_unstable();
        names.dedup();
        let mut model = Self {
            release: Vec::new(),
            prerelease: Vec::new(),
            maintenance: Vec::new(),
            overlaps: Vec::new(),
        };
        let mut claimed = HashSet::new();
        for (index, rule) in rules.iter().enumerate() {
            for &name in &names {
                if !matches(rule, name) || !claimed.insert(name) {
                    continue;
                }
                let others: Vec<usize> = rules
                    .iter()
                    .enumerate()
                    .skip(index + 1)
                    .filter(|(_, later)| matches(later, name))
                    .map(|(later, _)| later)
                    .collect();
                if !others.is_empty() {
                    model.overlaps.push(Overlap {
                        branch: name.to_owned(),
                        rule: index,
                        others,
                    });
                }
                model.add(rule, name)?;
            }
        }
        model.check()?;
        Ok(model)
    }

    fn add(&mut self, rule: &BranchRule, name: &str) -> Result<(), BranchError> {
        match rule {
            BranchRule::Release(rule) => {
                let main = self.release.is_empty();
                let channel = match rule.channel() {
                    None if main => Channel::Default,
                    channel => Channel::resolve(channel, name),
                };
                self.release.push(ReleaseBranch {
                    name: name.to_owned(),
                    channel,
                    main,
                });
            }
            BranchRule::Prerelease(rule) => {
                let id = match rule.prerelease() {
                    Prerelease::BranchName => name.to_owned(),
                    Prerelease::Id(template) => fill(template.as_str(), name),
                };
                let id = PrereleaseId::try_from(id.as_str())
                    .map_err(|_| BranchError::invalid_prerelease_id(name, &id))?;
                self.prerelease.push(PrereleaseBranch {
                    name: name.to_owned(),
                    id,
                    channel: Channel::resolve(rule.channel(), name),
                });
            }
            BranchRule::Maintenance(rule) => {
                // A name the rule matches always has a range the schema checked.
                let Some(line) = rule.range_for(name).and_then(MaintenanceLine::parse) else {
                    return Ok(());
                };
                self.maintenance.push(MaintenanceBranch {
                    name: name.to_owned(),
                    line,
                    channel: Channel::resolve(rule.channel(), name),
                });
            }
        }
        Ok(())
    }

    /// The checks across branches.
    fn check(&self) -> Result<(), BranchError> {
        if self.release.is_empty() || self.release.len() > MAX_RELEASE_BRANCHES {
            return Err(BranchError::release_branches(
                self.release.iter().map(ReleaseBranch::name),
            ));
        }
        for branch in &self.prerelease {
            let sharing: Vec<&str> = self
                .prerelease
                .iter()
                .filter(|other| other.id == branch.id)
                .map(PrereleaseBranch::name)
                .collect();
            if sharing.len() > 1 {
                return Err(BranchError::duplicate_prerelease_ids(&branch.id, &sharing));
            }
        }
        for branch in &self.maintenance {
            let sharing: Vec<&str> = self
                .maintenance
                .iter()
                .filter(|other| other.line == branch.line)
                .map(MaintenanceBranch::name)
                .collect();
            if sharing.len() > 1 {
                return Err(BranchError::duplicate_maintenance_ranges(
                    branch.line,
                    &sharing,
                ));
            }
        }
        Ok(())
    }

    /// The release branches in order; the first is the main line.
    #[must_use]
    pub fn release(&self) -> &[ReleaseBranch] {
        &self.release
    }

    /// The prerelease branches in order.
    #[must_use]
    pub fn prerelease(&self) -> &[PrereleaseBranch] {
        &self.prerelease
    }

    /// The maintenance branches in order.
    #[must_use]
    pub fn maintenance(&self) -> &[MaintenanceBranch] {
        &self.maintenance
    }

    /// The branches that later rules match too.
    #[must_use]
    pub fn overlaps(&self) -> &[Overlap] {
        &self.overlaps
    }

    /// The range of each release branch, from the versions on each (prereleases included; a
    /// branch missing from `versions` has none) and `initial`, the floor while the main line has
    /// no release.
    #[must_use]
    pub fn ranges(
        &self,
        versions: &HashMap<String, Vec<Version>>,
        initial: &Version,
    ) -> ReleaseRanges<'_> {
        let on =
            |branch: &ReleaseBranch| versions.get(branch.name()).map_or(&[][..], Vec::as_slice);
        let mut ranges: Vec<Range> = Vec::new();
        // The highest version on the branches so far, prereleases included.
        let mut highest: Option<&Version> = None;
        for (index, branch) in self.release.iter().enumerate() {
            let own = on(branch);
            let latest = own
                .iter()
                .filter(|version| version.as_semver().pre.is_empty())
                .max();
            let floor = match (ranges.last().map(Range::min), latest) {
                (None, latest) => latest.unwrap_or(initial),
                (Some(previous), Some(latest)) => previous.max(latest),
                (Some(previous), None) => previous,
            }
            .clone();
            highest = highest.max(own.iter().max());
            let bound = self.release.get(index + 1).and_then(|next| {
                on(next)
                    .iter()
                    .filter(|version| highest.is_none_or(|highest| *version > highest))
                    .min()
                    .cloned()
            });
            ranges.push(Range::new(floor, bound));
        }
        ReleaseRanges::new(self, ranges)
    }
}

/// `branches.rules` may give at most this many release branches.
const MAX_RELEASE_BRANCHES: usize = 3;

fn matches(rule: &BranchRule, branch: &str) -> bool {
    match rule {
        BranchRule::Release(rule) => rule.matches(branch),
        BranchRule::Prerelease(rule) => rule.matches(branch),
        BranchRule::Maintenance(rule) => rule.matches(branch),
    }
}

/// `template` with every `{name}` replaced by the branch name.
fn fill(template: &str, branch: &str) -> String {
    template.replace("{name}", branch)
}

/// A release branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseBranch {
    name: String,
    channel: Channel,
    main: bool,
}

impl ReleaseBranch {
    /// The branch name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The channel it releases to.
    #[must_use]
    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    /// Whether it is the main line, the first release branch.
    #[must_use]
    pub fn is_main(&self) -> bool {
        self.main
    }
}

/// A prerelease branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrereleaseBranch {
    name: String,
    id: PrereleaseId,
    channel: Channel,
}

impl PrereleaseBranch {
    /// The branch name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The identifier its versions carry.
    #[must_use]
    pub fn id(&self) -> &PrereleaseId {
        &self.id
    }

    /// The channel it releases to.
    #[must_use]
    pub fn channel(&self) -> &Channel {
        &self.channel
    }
}

/// A maintenance branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaintenanceBranch {
    name: String,
    line: MaintenanceLine,
    channel: Channel,
}

impl MaintenanceBranch {
    /// The branch name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The versions it maintains.
    #[must_use]
    pub fn line(&self) -> MaintenanceLine {
        self.line
    }

    /// The channel it releases to.
    #[must_use]
    pub fn channel(&self) -> &Channel {
        &self.channel
    }
}

/// The versions a maintenance branch maintains: a major (`1.x`, also written `1.x.x`) or a
/// minor (`1.2.x`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaintenanceLine {
    major: u64,
    minor: Option<u64>,
}

impl MaintenanceLine {
    /// The major version.
    #[must_use]
    pub fn major(self) -> u64 {
        self.major
    }

    /// The minor version, for a minor line.
    #[must_use]
    pub fn minor(self) -> Option<u64> {
        self.minor
    }
}

impl MaintenanceLine {
    /// 1.x, 1.x.x or 1.2.x; None for anything else.
    fn parse(range: &str) -> Option<Self> {
        let is_x = |part: &str| part.eq_ignore_ascii_case("x");
        match range.split('.').collect::<Vec<_>>().as_slice() {
            [major, x] if is_x(x) => Some(Self {
                major: major.parse().ok()?,
                minor: None,
            }),
            [major, minor, x] if is_x(x) => Some(Self {
                major: major.parse().ok()?,
                minor: if is_x(minor) {
                    None
                } else {
                    Some(minor.parse().ok()?)
                },
            }),
            _ => None,
        }
    }
}

impl fmt::Display for MaintenanceLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.minor {
            Some(minor) => write!(f, "{}.{minor}.x", self.major),
            None => write!(f, "{}.x", self.major),
        }
    }
}

/// The channel a branch releases to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Channel {
    /// The default channel.
    Default,
    /// A named channel.
    Named(String),
}

impl Channel {
    /// The channel a rule's channel gives ranch: its name when unset.
    fn resolve(channel: Option<&RuleChannel>, branch: &str) -> Self {
        match channel {
            None => Self::Named(branch.to_owned()),
            Some(RuleChannel::Default) => Self::Default,
            Some(RuleChannel::Named(template)) => Self::Named(fill(template.as_str(), branch)),
        }
    }
}

/// A branch that later rules match too: the first matching rule claims it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overlap {
    branch: String,
    rule: usize,
    others: Vec<usize>,
}

impl Overlap {
    /// The branch name.
    #[must_use]
    pub fn branch(&self) -> &str {
        &self.branch
    }

    /// The index in `branches.rules` of the rule that claims it.
    #[must_use]
    pub fn rule(&self) -> usize {
        self.rule
    }

    /// The indexes of the later rules that match it too.
    #[must_use]
    pub fn others(&self) -> &[usize] {
        &self.others
    }
}

/// Helpers shared by the branch modules' unit tests.
#[cfg(test)]
mod test_support {
    use semoxide_schema::config::{BranchRule, Config};

    use super::{BranchError, BranchModel, Channel};

    /// The rules of a config holding `branches.rules = …`; `""` gives the default rules.
    pub(super) fn rules(text: &str) -> Vec<BranchRule> {
        Config::from_table(text.parse().unwrap())
            .unwrap()
            .branches()
            .rules()
            .to_vec()
    }

    pub(super) fn model(text: &str, branches: &[&str]) -> Result<BranchModel, BranchError> {
        BranchModel::new(&rules(text), branches)
    }

    pub(super) fn channel(channel: &Channel) -> String {
        match channel {
            Channel::Default => String::from("default"),
            Channel::Named(name) => format!("'{name}'"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{channel, model};
    use super::*;

    /// Every branch and overlap of the model in one line each, so a test can compare whole
    /// models.
    fn describe(model: &BranchModel) -> Vec<String> {
        let release = model.release().iter().map(|branch| {
            let main = if branch.is_main() { " main" } else { "" };
            format!(
                "release {} channel={}{main}",
                branch.name(),
                channel(branch.channel())
            )
        });
        let prerelease = model.prerelease().iter().map(|branch| {
            format!(
                "prerelease {} id={} channel={}",
                branch.name(),
                branch.id(),
                channel(branch.channel())
            )
        });
        let maintenance = model.maintenance().iter().map(|branch| {
            format!(
                "maintenance {} line={} channel={}",
                branch.name(),
                branch.line(),
                channel(branch.channel())
            )
        });
        let overlaps = model.overlaps().iter().map(|overlap| {
            format!(
                "overlap {} rule={} others={:?}",
                overlap.branch(),
                overlap.rule(),
                overlap.others()
            )
        });
        release
            .chain(prerelease)
            .chain(maintenance)
            .chain(overlaps)
            .collect()
    }

    fn described(text: &str, branches: &[&str]) -> Vec<String> {
        describe(&model(text, branches).unwrap())
    }

    #[test]
    fn the_default_rules_sort_every_kind() {
        assert_eq!(
            described(
                "",
                &["main", "beta", "alpha", "1.x", "1.2.x", "feature/x", "next"]
            ),
            [
                "release main channel=default main",
                "release next channel='next'",
                "prerelease beta id=beta channel='beta'",
                "prerelease alpha id=alpha channel='alpha'",
                "maintenance 1.2.x line=1.2.x channel='1.2.x'",
                "maintenance 1.x line=1.x channel='1.x'",
            ]
        );
    }

    #[test]
    fn master_comes_before_main_and_takes_the_main_line() {
        assert_eq!(
            described("", &["main", "master"]),
            [
                "release master channel=default main",
                "release main channel='main'",
            ]
        );
    }

    #[test]
    fn a_rule_matching_no_branch_is_dropped() {
        assert_eq!(
            described("", &["next", "beta"]),
            [
                "release next channel=default main",
                "prerelease beta id=beta channel='beta'",
            ]
        );
    }

    #[test]
    fn a_glob_takes_its_branches_in_name_order() {
        assert_eq!(
            described(
                r#"branches.rules = ["rel/*"]"#,
                &["rel/c", "rel/a", "other", "rel/b"]
            ),
            [
                "release rel/a channel=default main",
                "release rel/b channel='rel/b'",
                "release rel/c channel='rel/c'",
            ]
        );
    }

    #[test]
    fn release_channels() {
        let text = r#"branches.rules = [
            { name = "main", channel = "latest" },
            { name = "next", channel = false },
            { name = "next-major", channel = "ch-{name}" },
        ]"#;

        assert_eq!(
            described(text, &["main", "next", "next-major"]),
            [
                "release main channel='latest' main",
                "release next channel=default",
                "release next-major channel='ch-next-major'",
            ]
        );
    }

    #[test]
    fn the_main_line_can_set_the_default_channel_explicitly() {
        assert_eq!(
            described(
                r#"branches.rules = [{ name = "main", channel = false }, "next"]"#,
                &["main", "next"]
            ),
            [
                "release main channel=default main",
                "release next channel='next'",
            ]
        );
    }

    #[test]
    fn prerelease_identifiers_and_channels() {
        let text = r#"branches.rules = [
            "main",
            { name = "beta", prerelease = "rc" },
            { name = "pre-*", prerelease = "{name}", channel = "{name}-ch" },
            { name = "alpha", prerelease = true, channel = false },
        ]"#;

        assert_eq!(
            described(text, &["main", "beta", "pre-a", "alpha"]),
            [
                "release main channel=default main",
                "prerelease beta id=rc channel='beta'",
                "prerelease pre-a id=pre-a channel='pre-a-ch'",
                "prerelease alpha id=alpha channel=default",
            ]
        );
    }

    #[test]
    fn every_name_placeholder_is_filled() {
        let text = r#"branches.rules = [
            "main",
            { name = "pre-*", prerelease = "{name}.{name}", channel = "{name}/{name}" },
        ]"#;

        assert_eq!(
            described(text, &["main", "pre-a"]),
            [
                "release main channel=default main",
                "prerelease pre-a id=pre-a.pre-a channel='pre-a/pre-a'",
            ]
        );
    }

    /// Prerelease identifiers are case-sensitive, as in Semantic Versioning.
    #[test]
    fn prerelease_identifiers_differing_in_case_are_different() {
        let text = r#"branches.rules = [
            "main",
            { name = "beta", prerelease = "rc" },
            { name = "alpha", prerelease = "RC" },
        ]"#;

        assert_eq!(
            described(text, &["main", "beta", "alpha"]),
            [
                "release main channel=default main",
                "prerelease beta id=rc channel='beta'",
                "prerelease alpha id=RC channel='alpha'",
            ]
        );
    }

    #[test]
    fn maintenance_lines_and_channels() {
        let text = r#"branches.rules = [
            { maintenance = "N.x" },
            { maintenance = "release/N.x", channel = "v{name}" },
            { maintenance = "legacy", range = "0.x", channel = false },
            "main",
        ]"#;

        assert_eq!(
            described(text, &["2.3.x", "1.x.x", "release/4.x", "legacy", "main"]),
            [
                "release main channel=default main",
                "maintenance 1.x.x line=1.x channel='1.x.x'",
                "maintenance 2.3.x line=2.3.x channel='2.3.x'",
                "maintenance release/4.x line=4.x channel='vrelease/4.x'",
                "maintenance legacy line=0.x channel=default",
            ]
        );
    }

    #[test]
    fn the_first_matching_rule_claims_a_branch_and_overlaps_are_listed() {
        let text = r#"branches.rules = ["main", "ma*", { name = "*", prerelease = "dev" }]"#;

        assert_eq!(
            described(text, &["main", "mabc", "zed"]),
            [
                "release main channel=default main",
                "release mabc channel='mabc'",
                "prerelease zed id=dev channel='zed'",
                "overlap main rule=0 others=[1, 2]",
                "overlap mabc rule=1 others=[2]",
            ]
        );
    }

    #[test]
    fn a_maintenance_line_has_its_parts() {
        let model = model("", &["main", "1.x", "2.3.x"]).unwrap();

        assert_eq!(
            model
                .maintenance()
                .iter()
                .map(|branch| (branch.line().major(), branch.line().minor()))
                .collect::<Vec<_>>(),
            [(1, None), (2, Some(3))]
        );
    }

    #[test]
    fn one_prerelease_branch_of_a_shared_identifier_is_fine() {
        let text = r#"branches.rules = [
            "main",
            { name = "beta", prerelease = "rc" },
            { name = "alpha", prerelease = "rc" },
        ]"#;

        assert_eq!(
            described(text, &["main", "beta"]),
            [
                "release main channel=default main",
                "prerelease beta id=rc channel='beta'",
            ]
        );
    }

    #[test]
    fn a_major_and_a_minor_line_of_it_are_different_ranges() {
        assert_eq!(
            described("", &["main", "1.x", "1.2.x"]),
            [
                "release main channel=default main",
                "maintenance 1.2.x line=1.2.x channel='1.2.x'",
                "maintenance 1.x line=1.x channel='1.x'",
            ]
        );
    }
}
