//! The gate at the start of a run: whether it runs for real, as a dry run or stops (CLI.md,
//! `--dry-run` and `--no-ci`).

use tracing::{debug, info, warn};

use super::NoReleaseReason;
use crate::ci::{CiContext, PullRequest};

/// What the caller asked for: `--dry-run` or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Requested {
    Release,
    DryRun,
}

/// Whether running outside CI forces a dry run; `--no-ci` skips it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CiRule {
    Enforced,
    Skipped,
}

/// Whether the run is a dry run, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DryRun {
    No,
    Requested,
    /// Outside CI without `--no-ci`: the outcome is `NotCi` plus the would-be one.
    Forced,
}

/// How a run goes: dry or not, and whether it stops before the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Gate {
    dry_run: DryRun,
    stop: Option<NoReleaseReason>,
}

impl Gate {
    pub(crate) fn dry_run(self) -> DryRun {
        self.dry_run
    }

    /// The reason the run stops before the pipeline, if it does.
    pub(crate) fn stop(self) -> Option<NoReleaseReason> {
        self.stop
    }
}

/// The gate for a run in `ci` with what the caller asked for.
pub(crate) fn gate(ci: &CiContext, requested: Requested, ci_rule: CiRule) -> Gate {
    let dry_run = match (requested, ci_rule) {
        (Requested::DryRun, _) => DryRun::Requested,
        (Requested::Release, CiRule::Enforced) if !ci.is_ci() => {
            warn!(target: "semoxide::core", "not on CI, so this is a dry run");
            DryRun::Forced
        }
        (Requested::Release, _) => DryRun::No,
    };
    // `--no-ci` never lets a pull request release; `SEMOXIDE_CI_IS_PR=false` is the way out.
    let stop = matches!(ci.pull_request(), PullRequest::Yes { .. })
        .then_some(NoReleaseReason::PullRequest);
    debug!(
        target: "semoxide::core",
        is_ci = ci.is_ci(),
        pull_request = ?ci.pull_request(),
        ?requested,
        ?ci_rule,
        ?dry_run,
        ?stop,
        "run gate"
    );
    if stop.is_some() {
        info!(target: "semoxide::core", "pull request run, so nothing is released");
    }
    Gate { dry_run, stop }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::Env;
    use crate::ci::detect;
    use crate::env::Case;

    #[derive(Debug, Clone, Copy)]
    enum Where {
        Vendor,
        CiVariable,
        Outside,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Run {
        Push,
        PullRequest,
    }

    /// The context of a run; only the vendor's pull request has a number.
    fn ci(place: Where, run: Run) -> CiContext {
        let vars: &[(&str, &str)] = match (place, run) {
            (Where::Vendor, Run::Push) => &[
                ("GITHUB_ACTIONS", "true"),
                ("GITHUB_REF", "refs/heads/main"),
            ],
            (Where::Vendor, Run::PullRequest) => &[
                ("GITHUB_ACTIONS", "true"),
                ("GITHUB_EVENT_NAME", "pull_request"),
                ("GITHUB_REF", "refs/pull/1/merge"),
                ("GITHUB_BASE_REF", "main"),
            ],
            (Where::CiVariable, Run::Push) => &[("CI", "true")],
            (Where::CiVariable, Run::PullRequest) => {
                &[("CI", "true"), ("SEMOXIDE_CI_IS_PR", "true")]
            }
            (Where::Outside, Run::Push) => &[("PATH", "/bin")],
            (Where::Outside, Run::PullRequest) => {
                &[("PATH", "/bin"), ("SEMOXIDE_CI_IS_PR", "true")]
            }
        };
        let result = detect(&Env::with_case(Case::Sensitive, vars.iter().copied()));
        assert!(result.is_ok(), "detection should succeed: {result:?}");
        result.unwrap()
    }

    type Row = (Run, Requested, CiRule, DryRun, Option<NoReleaseReason>);

    const PR: Option<NoReleaseReason> = Some(NoReleaseReason::PullRequest);

    /// On CI `--no-ci` changes nothing; a pull request always stops.
    #[rustfmt::skip]
    const ON_CI: [Row; 8] = [
        (Run::Push,        Requested::Release, CiRule::Enforced, DryRun::No,        None),
        (Run::Push,        Requested::Release, CiRule::Skipped,  DryRun::No,        None),
        (Run::Push,        Requested::DryRun,  CiRule::Enforced, DryRun::Requested, None),
        (Run::Push,        Requested::DryRun,  CiRule::Skipped,  DryRun::Requested, None),
        (Run::PullRequest, Requested::Release, CiRule::Enforced, DryRun::No,        PR),
        (Run::PullRequest, Requested::Release, CiRule::Skipped,  DryRun::No,        PR),
        (Run::PullRequest, Requested::DryRun,  CiRule::Enforced, DryRun::Requested, PR),
        (Run::PullRequest, Requested::DryRun,  CiRule::Skipped,  DryRun::Requested, PR),
    ];

    /// Outside CI a dry run is forced unless `--dry-run` or `--no-ci` is given; a pull request
    /// still stops.
    #[rustfmt::skip]
    const OUTSIDE_CI: [Row; 8] = [
        (Run::Push,        Requested::Release, CiRule::Enforced, DryRun::Forced,    None),
        (Run::Push,        Requested::Release, CiRule::Skipped,  DryRun::No,        None),
        (Run::Push,        Requested::DryRun,  CiRule::Enforced, DryRun::Requested, None),
        (Run::Push,        Requested::DryRun,  CiRule::Skipped,  DryRun::Requested, None),
        (Run::PullRequest, Requested::Release, CiRule::Enforced, DryRun::Forced,    PR),
        (Run::PullRequest, Requested::Release, CiRule::Skipped,  DryRun::No,        PR),
        (Run::PullRequest, Requested::DryRun,  CiRule::Enforced, DryRun::Requested, PR),
        (Run::PullRequest, Requested::DryRun,  CiRule::Skipped,  DryRun::Requested, PR),
    ];

    #[rstest]
    #[case::vendor(Where::Vendor, ON_CI)]
    #[case::ci_variable(Where::CiVariable, ON_CI)]
    #[case::outside_ci(Where::Outside, OUTSIDE_CI)]
    fn the_gate_follows_the_rules(#[case] place: Where, #[case] table: [Row; 8]) {
        let actual: Vec<Row> = table
            .iter()
            .map(|&(run, requested, ci_rule, _, _)| {
                let gate = gate(&ci(place, run), requested, ci_rule);
                (run, requested, ci_rule, gate.dry_run(), gate.stop())
            })
            .collect();

        assert_eq!(actual, table);
    }
}
