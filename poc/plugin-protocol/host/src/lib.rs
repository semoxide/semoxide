//! semoxide plugin-protocol PoC host (throwaway).

pub mod builtin;
pub mod plugin;
pub mod process;
pub mod protocol;
pub mod resolve;
#[cfg(feature = "wasm")]
pub mod wasm;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use plugin::*;
use process::{PluginCommand, ProcessPlugin};
use resolve::{Pinned, Resolved};

enum Source {
    Instance(Arc<dyn Plugin>),
    Named { name: String, config: Value, pinned: Option<Pinned> },
    Command(PluginCommand),
}

pub struct Builder {
    sources: Vec<Source>,
    env: BTreeMap<String, String>,
    cwd: PathBuf,
    sink: Arc<dyn LogSink>,
    timeout: Duration,
    /// Extra dirs searched before PATH (e.g. the host's own dir).
    extra_path: Vec<PathBuf>,
}

pub struct Semoxide {
    plugins: Vec<Arc<dyn Plugin>>,
    env: BTreeMap<String, String>,
    cwd: PathBuf,
    logger: Logger,
}

#[derive(Debug, thiserror::Error)]
#[error("{}", .0.iter().map(|(p, s, e)| format!("[{p}] {}: {e}", s.method())).collect::<Vec<_>>().join("; "))]
pub struct RunError(pub Vec<(String, Step, PluginError)>);

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error(transparent)]
    Resolve(#[from] resolve::ResolveError),
    #[error("plugin '{0}': {1}")]
    Spawn(String, PluginError),
}

pub struct RunInput {
    pub branch: Branch,
    pub commits: Vec<Commit>,
    pub last_release: Option<LastRelease>,
}

#[derive(Debug)]
pub enum Outcome {
    NoRelease,
    Released { next: NextRelease, releases: Vec<(String, PublishResult)> },
}

impl Semoxide {
    pub fn builder() -> Builder {
        Builder {
            sources: vec![],
            env: std::env::vars().collect(),
            cwd: std::env::current_dir().unwrap_or_default(),
            sink: Arc::new(StderrSink { debug: false }),
            timeout: Duration::from_secs(30),
            extra_path: vec![],
        }
    }
}

impl Builder {
    /// Embedder registration: any `impl Plugin`, compiled into the embedder.
    pub fn plugin(mut self, p: impl Plugin + 'static) -> Self {
        self.sources.push(Source::Instance(Arc::new(p)));
        self
    }
    /// Config-style entry: built-in name, else `semoxide-plugin-<name>` on PATH, else pinned file.
    pub fn named(mut self, name: &str, config: Value) -> Self {
        self.sources.push(Source::Named { name: name.into(), config, pinned: None });
        self
    }
    pub fn named_pinned(mut self, name: &str, pinned: Pinned) -> Self {
        self.sources.push(Source::Named { name: name.into(), config: json!({}), pinned: Some(pinned) });
        self
    }
    /// Explicit `command = [...]` entry.
    pub fn command(mut self, c: PluginCommand) -> Self {
        self.sources.push(Source::Command(c));
        self
    }
    pub fn env(mut self, env: BTreeMap<String, String>) -> Self {
        self.env = env;
        self
    }
    pub fn sink(mut self, s: Arc<dyn LogSink>) -> Self {
        self.sink = s;
        self
    }
    pub fn timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }
    pub fn extra_path(mut self, p: PathBuf) -> Self {
        self.extra_path.push(p);
        self
    }

    /// Resolve and spawn everything; process plugins are handshaken here so
    /// their step sets are known before the run.
    pub async fn build(self) -> Result<Semoxide, BuildError> {
        let masker = Arc::new(Masker::from_env(&self.env));
        let logger = Logger::new(self.sink.clone(), masker);
        let path_var = {
            let mut dirs = self.extra_path.clone();
            dirs.extend(std::env::split_paths(&OsString::from(self.env.get("PATH").cloned().unwrap_or_default())));
            std::env::join_paths(dirs).ok()
        };
        let pathext = self.env.get("PATHEXT").cloned();
        let mut plugins: Vec<Arc<dyn Plugin>> = vec![];
        for src in self.sources {
            let cmd = match src {
                Source::Instance(p) => {
                    plugins.push(p);
                    continue;
                }
                Source::Command(c) => c,
                Source::Named { name, config, pinned } => {
                    match resolve::resolve(&name, builtin::BUILTINS, path_var.as_ref(), pathext.as_deref(), pinned.as_ref())? {
                        Resolved::Builtin(b) => {
                            plugins.push(builtin::builtin(&b).unwrap());
                            continue;
                        }
                        Resolved::Executable(exe) => {
                            PluginCommand::new(&name, exe).config(config).timeout(self.timeout)
                        }
                    }
                }
            };
            let p = ProcessPlugin::spawn(&cmd, &self.env, &self.cwd, &logger)
                .await
                .map_err(|e| BuildError::Spawn(cmd.name.clone(), e))?;
            plugins.push(Arc::new(p));
        }
        Ok(Semoxide { plugins, env: self.env, cwd: self.cwd, logger })
    }
}

fn bump(last: Option<&LastRelease>, t: ReleaseType) -> String {
    let Some(l) = last else { return "1.0.0".into() };
    let v: Vec<u64> = l.version.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    let (ma, mi, pa) = (v[0], v[1], v[2]);
    match t {
        ReleaseType::Major => format!("{}.0.0", ma + 1),
        ReleaseType::Minor => format!("{ma}.{}.0", mi + 1),
        ReleaseType::Patch => format!("{ma}.{mi}.{}", pa + 1),
    }
}

impl Semoxide {
    pub fn plugins(&self) -> &[Arc<dyn Plugin>] {
        &self.plugins
    }

    fn with(&self, step: Step) -> impl Iterator<Item = &Arc<dyn Plugin>> {
        self.plugins.iter().filter(move |p| p.steps().contains(&step))
    }

    /// verifyConditions -> analyzeCommits -> generateNotes -> publish, then shutdown.
    pub async fn run(&self, input: RunInput) -> Result<Outcome, RunError> {
        let r = self.run_inner(input).await;
        for p in &self.plugins {
            p.shutdown().await;
        }
        r
    }

    async fn run_inner(&self, input: RunInput) -> Result<Outcome, RunError> {
        let mut ctx = Context {
            cwd: self.cwd.clone(),
            env: self.env.clone(),
            branch: input.branch,
            commits: input.commits,
            last_release: input.last_release,
            next_release: None,
            options: json!({}),
            logger: self.logger.clone(),
        };
        let scoped = |ctx: &Context, p: &Arc<dyn Plugin>| Context { logger: self.logger.scoped(p.name()), ..ctx.clone() };

        // verifyConditions: run all, aggregate errors.
        let mut errors = vec![];
        for p in self.with(Step::VerifyConditions) {
            if let Err(e) = p.verify_conditions(&scoped(&ctx, p)).await {
                errors.push((p.name().to_string(), Step::VerifyConditions, e));
            }
        }
        if !errors.is_empty() {
            return Err(RunError(errors));
        }

        // analyzeCommits: highest type wins.
        let mut kind = None;
        for p in self.with(Step::AnalyzeCommits) {
            let t = p.analyze_commits(&scoped(&ctx, p)).await.map_err(|e| RunError(vec![(p.name().into(), Step::AnalyzeCommits, e)]))?;
            kind = kind.max(t);
        }
        let Some(kind) = kind else {
            self.logger.info("no release");
            return Ok(Outcome::NoRelease);
        };
        let version = bump(ctx.last_release.as_ref(), kind);
        ctx.next_release = Some(NextRelease { kind, git_tag: format!("v{version}"), version, notes: String::new() });

        // generateNotes: concatenated with "\n\n"; each plugin sees prior notes.
        for p in self.with(Step::GenerateNotes) {
            let n = p.generate_notes(&scoped(&ctx, p)).await.map_err(|e| RunError(vec![(p.name().into(), Step::GenerateNotes, e)]))?;
            if let Some(n) = n.filter(|n| !n.is_empty()) {
                let next = ctx.next_release.as_mut().unwrap();
                next.notes = if next.notes.is_empty() { n } else { format!("{}\n\n{n}", next.notes) };
            }
        }

        // publish: sequential, stop on first error.
        let mut releases = vec![];
        for p in self.with(Step::Publish) {
            let r = p.publish(&scoped(&ctx, p)).await.map_err(|e| RunError(vec![(p.name().into(), Step::Publish, e)]))?;
            if let Some(r) = r {
                releases.push((p.name().to_string(), r));
            }
        }
        Ok(Outcome::Released { next: ctx.next_release.unwrap(), releases })
    }
}
