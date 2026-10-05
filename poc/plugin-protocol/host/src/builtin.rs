//! A compiled-in plugin: tiny conventional-commits analyzer + notes.

use std::sync::Arc;

use async_trait::async_trait;

use crate::plugin::*;

pub const BUILTINS: &[&str] = &["conventional"];

pub fn builtin(name: &str) -> Option<Arc<dyn Plugin>> {
    match name {
        "conventional" => Some(Arc::new(Conventional)),
        _ => None,
    }
}

pub struct Conventional;

fn kind(msg: &str) -> Option<ReleaseType> {
    let head = msg.lines().next().unwrap_or("");
    if msg.contains("BREAKING CHANGE:") || head.split(':').next().is_some_and(|t| t.ends_with('!')) {
        Some(ReleaseType::Major)
    } else if head.starts_with("feat") {
        Some(ReleaseType::Minor)
    } else if head.starts_with("fix") || head.starts_with("perf") {
        Some(ReleaseType::Patch)
    } else {
        None
    }
}

#[async_trait]
impl Plugin for Conventional {
    fn name(&self) -> &str {
        "conventional"
    }
    fn steps(&self) -> StepSet {
        [Step::AnalyzeCommits, Step::GenerateNotes].into()
    }
    async fn analyze_commits(&self, ctx: &Context) -> Result<Option<ReleaseType>, PluginError> {
        let t = ctx.commits.iter().filter_map(|c| kind(&c.message)).max();
        ctx.logger.info(&format!("analyzed {} commits: {t:?}", ctx.commits.len()));
        Ok(t)
    }
    async fn generate_notes(&self, ctx: &Context) -> Result<Option<String>, PluginError> {
        let next = ctx.next_release.as_ref().expect("nextRelease set before generateNotes");
        let mut s = format!("## {}\n", next.version);
        for c in &ctx.commits {
            s.push_str(&format!("- {} ({})\n", c.message.lines().next().unwrap_or(""), &c.hash[..7.min(c.hash.len())]));
        }
        Ok(Some(s))
    }
}
