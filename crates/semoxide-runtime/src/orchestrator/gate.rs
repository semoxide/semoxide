//! The gate at the start of a run: whether it runs for real, as a dry run or stops (CLI.md,
//! `--dry-run` and `--no-ci`).

use super::NoReleaseReason;
use crate::ci::CiContext;

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
pub(crate) fn gate(_ci: &CiContext, _requested: Requested, _ci_rule: CiRule) -> Gate {
    Gate {
        dry_run: DryRun::No,
        stop: None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rstest::rstest;
    use tracing::field::{Field, Visit};
    use tracing::{Event, Level, Subscriber};
    use tracing_subscriber::Registry;
    use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

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

    // --- Logging ---

    type Logged = (Level, String, String);

    /// Records each event's level, target and message.
    #[derive(Clone, Default)]
    struct Events(Arc<Mutex<Vec<Logged>>>);

    impl<S: Subscriber> Layer<S> for Events {
        fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
            let mut message = Message::default();
            event.record(&mut message);
            let metadata = event.metadata();
            self.0.lock().unwrap().push((
                *metadata.level(),
                metadata.target().to_owned(),
                message.0,
            ));
        }
    }

    #[derive(Default)]
    struct Message(String);

    impl Visit for Message {
        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            if field.name() == "message" {
                self.0 = format!("{value:?}");
            }
        }
    }

    /// The events the gate logs for a run.
    fn logged(place: Where, run: Run, requested: Requested, ci_rule: CiRule) -> Vec<Logged> {
        let context = ci(place, run);
        let events = Events::default();
        let subscriber = Registry::default().with(events.clone());
        tracing::subscriber::with_default(subscriber, || gate(&context, requested, ci_rule));
        events.0.lock().unwrap().clone()
    }

    fn warn_not_ci() -> Logged {
        (
            Level::WARN,
            String::from("semoxide::core"),
            String::from("not on CI, so this is a dry run"),
        )
    }

    fn info_pull_request() -> Logged {
        (
            Level::INFO,
            String::from("semoxide::core"),
            String::from("pull request run, so nothing is released"),
        )
    }

    #[test]
    fn a_forced_dry_run_warns() {
        assert_eq!(
            logged(
                Where::Outside,
                Run::Push,
                Requested::Release,
                CiRule::Enforced
            ),
            [warn_not_ci()]
        );
    }

    #[test]
    fn a_pull_request_stop_is_logged() {
        assert_eq!(
            (
                logged(
                    Where::Vendor,
                    Run::PullRequest,
                    Requested::Release,
                    CiRule::Enforced
                ),
                logged(
                    Where::Vendor,
                    Run::PullRequest,
                    Requested::DryRun,
                    CiRule::Skipped
                )
            ),
            (vec![info_pull_request()], vec![info_pull_request()])
        );
    }

    #[test]
    fn outside_ci_a_pull_request_logs_the_forced_dry_run_then_the_stop() {
        assert_eq!(
            logged(
                Where::Outside,
                Run::PullRequest,
                Requested::Release,
                CiRule::Enforced
            ),
            [warn_not_ci(), info_pull_request()]
        );
    }

    #[test]
    fn only_a_forced_dry_run_or_a_stop_is_logged() {
        let quiet = [
            logged(
                Where::Vendor,
                Run::Push,
                Requested::Release,
                CiRule::Enforced,
            ),
            logged(
                Where::CiVariable,
                Run::Push,
                Requested::DryRun,
                CiRule::Enforced,
            ),
            logged(
                Where::Outside,
                Run::Push,
                Requested::Release,
                CiRule::Skipped,
            ),
            logged(
                Where::Outside,
                Run::Push,
                Requested::DryRun,
                CiRule::Enforced,
            ),
        ];
        let forced = logged(
            Where::Outside,
            Run::Push,
            Requested::Release,
            CiRule::Enforced,
        );

        assert_eq!((quiet.concat(), forced.len()), (Vec::new(), 1));
    }
}
