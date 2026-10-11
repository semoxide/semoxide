//! The release run: its rules, steps and outcome (ARCHITECTURE, Orchestrator).

#[expect(dead_code, reason = "used by the run pipeline (#76)")]
mod gate;

/// Why a run ends without a release (OBSERVABILITY, no-release reasons).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NoReleaseReason {
    /// Outside CI: the run is forced to a dry run.
    NotCi,
    /// A pull or merge request run, which never releases.
    PullRequest,
}
