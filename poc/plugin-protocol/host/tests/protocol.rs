use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use serde_json::json;
use semoxide_poc_host::plugin::*;
use semoxide_poc_host::process::{PluginCommand, ProcessPlugin};
use semoxide_poc_host::resolve::{self, Pinned, Resolved, ResolveError};
use semoxide_poc_host::{BuildError, Outcome, RunInput, Semoxide};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
const SECRET: &str = "ghp_FAKEsecret123";

/// Builds the demo plugin into its own target dir (a bin of another package
/// has no CARGO_BIN_EXE_ here).
fn demo_exe() -> &'static Path {
    static EXE: OnceLock<PathBuf> = OnceLock::new();
    EXE.get_or_init(|| {
        let target = Path::new(ROOT).join("target/test-plugins");
        let st = std::process::Command::new(std::env::var("CARGO").unwrap_or("cargo".into()))
            .args(["build", "-q", "-p", "semoxide-plugin-demo", "--target-dir"])
            .arg(&target)
            .current_dir(ROOT)
            .status()
            .unwrap();
        assert!(st.success());
        target.join("debug").join(format!("semoxide-plugin-demo{}", std::env::consts::EXE_SUFFIX))
    })
}

fn py() -> String {
    format!("{ROOT}/plugins/python/semoxide-plugin-py.py")
}

fn env() -> BTreeMap<String, String> {
    let mut e: BTreeMap<_, _> = std::env::vars().collect();
    e.insert("GH_TOKEN".into(), SECRET.into());
    e
}

fn input() -> RunInput {
    RunInput {
        branch: Branch { name: "main".into(), channel: None },
        commits: vec![
            Commit { hash: "a1b2c3d4e5".into(), message: "feat: add thing".into() },
            Commit { hash: "f6e5d4c3b2".into(), message: "fix: bug".into() },
        ],
        last_release: Some(LastRelease { version: "1.2.3".into(), git_tag: "v1.2.3".into(), git_head: "0".into() }),
    }
}

fn demo(mode: &str) -> PluginCommand {
    PluginCommand::new("demo", demo_exe()).args(["--mode", mode]).timeout(Duration::from_secs(10))
}

async fn build(sink: &Arc<MemorySink>, cmds: Vec<PluginCommand>) -> Result<Semoxide, BuildError> {
    let mut b = Semoxide::builder().env(env()).sink(sink.clone()).named("conventional", json!({}));
    for c in cmds {
        b = b.command(c);
    }
    b.build().await
}

fn has(sink: &MemorySink, needle: &str) -> bool {
    sink.lines().iter().any(|l| l.contains(needle))
}

#[tokio::test]
async fn happy_path_builtin_rust_python_and_embedder() {
    struct Emb;
    #[async_trait::async_trait]
    impl Plugin for Emb {
        fn name(&self) -> &str {
            "emb"
        }
        fn steps(&self) -> StepSet {
            [Step::Publish].into()
        }
        async fn publish(&self, _: &Context) -> Result<Option<PublishResult>, PluginError> {
            Ok(Some(PublishResult { name: Some("emb".into()), ..Default::default() }))
        }
    }
    let sink = Arc::new(MemorySink::default());
    let sx = Semoxide::builder()
        .env(env())
        .sink(sink.clone())
        .named("conventional", json!({}))
        .command(demo("ok"))
        .command(PluginCommand::new("py", "python").args([py()]))
        .plugin(Emb)
        .build()
        .await
        .unwrap();
    let Outcome::Released { next, releases } = sx.run(input()).await.unwrap() else { panic!() };
    assert_eq!(next.version, "1.3.0");
    assert!(next.notes.contains("## 1.3.0") && next.notes.contains("### Demo plugin") && next.notes.contains("### From Python"));
    let names: Vec<_> = releases.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(names, ["demo", "emb"]);
    assert!(has(&sink, "python analyzed 2 commits -> minor"));
}

#[tokio::test]
async fn secret_logged_by_plugin_is_masked() {
    let sink = Arc::new(MemorySink::default());
    build(&sink, vec![demo("stderr")]).await.unwrap().run(input()).await.unwrap();
    assert!(has(&sink, "verified GH_TOKEN=[secure]"), "{:?}", sink.lines());
    // stderr is captured as debug logs and masked too
    assert!(has(&sink, "Debug: stderr: warning from demo on stderr, token=[secure]"), "{:?}", sink.lines());
    assert!(!sink.lines().iter().any(|l| l.contains(SECRET)));
}

#[tokio::test]
async fn crash_mid_run_fails_the_step() {
    let sink = Arc::new(MemorySink::default());
    let err = build(&sink, vec![demo("crash")]).await.unwrap().run(input()).await.unwrap_err();
    let (p, s, e) = &err.0[0];
    assert_eq!((p.as_str(), *s), ("demo", Step::Publish));
    assert!(matches!(e, PluginError::Exited(r) if r.contains("exit code: 3")), "{e}");
}

#[tokio::test]
async fn invalid_result_is_rejected_with_semantic_release_code() {
    let sink = Arc::new(MemorySink::default());
    let err = build(&sink, vec![demo("invalid")]).await.unwrap().run(input()).await.unwrap_err();
    let (_, s, e) = &err.0[0];
    assert_eq!(*s, Step::AnalyzeCommits);
    assert!(matches!(e, PluginError::InvalidOutput { code: "EANALYZECOMMITSOUTPUT", .. }), "{e}");
}

#[tokio::test]
async fn unimplemented_step_is_skipped_via_handshake() {
    let sink = Arc::new(MemorySink::default());
    let p = ProcessPlugin::spawn(&demo("ok"), &env(), Path::new(ROOT), &Logger::new(sink.clone(), Arc::new(Masker::from_env(&env()))))
        .await
        .unwrap();
    // demo declares verifyConditions/generateNotes/publish + an unknown future step
    assert_eq!(p.steps(), StepSet::from([Step::VerifyConditions, Step::GenerateNotes, Step::Publish]));
    // calling it anyway maps JSON-RPC -32601 to NotImplemented
    let ctx = Context {
        cwd: ROOT.into(),
        env: BTreeMap::new(),
        branch: input().branch,
        commits: vec![],
        last_release: None,
        next_release: None,
        options: json!({}),
        logger: Logger::new(sink.clone(), Arc::new(Masker::from_env(&BTreeMap::new()))),
    };
    assert!(matches!(p.analyze_commits(&ctx).await, Err(PluginError::NotImplemented)));
    p.shutdown().await;
}

#[tokio::test]
async fn protocol_version_mismatch_fails_at_build() {
    let sink = Arc::new(MemorySink::default());
    let Err(BuildError::Spawn(name, PluginError::Protocol(m))) = build(&sink, vec![demo("badproto")]).await else { panic!() };
    assert_eq!(name, "demo");
    assert!(m.contains("protocol 99"), "{m}");
}

#[tokio::test]
async fn plugin_error_with_code_maps_to_release_error() {
    let sink = Arc::new(MemorySink::default());
    let mut e = env();
    e.remove("GH_TOKEN");
    let sx = Semoxide::builder().env(e).sink(sink.clone()).command(demo("ok")).build().await.unwrap();
    let err = sx.run(input()).await.unwrap_err();
    assert!(matches!(&err.0[0].2, PluginError::Release { code, .. } if code == "ENOGHTOKEN"), "{err}");
}

#[tokio::test]
async fn garbage_on_stdout_is_logged_not_fatal() {
    let sink = Arc::new(MemorySink::default());
    let r = build(&sink, vec![demo("garbage")]).await.unwrap().run(input()).await;
    assert!(r.is_ok(), "{r:?}");
    assert!(has(&sink, "Warn: non-protocol stdout ignored: npm notice"));
    assert!(has(&sink, "Warn: non-protocol stdout ignored: {\"some\""));
}

#[tokio::test]
async fn child_of_plugin_inheriting_stdout_is_tolerated() {
    let sink = Arc::new(MemorySink::default());
    let r = build(&sink, vec![demo("noisy-child")]).await.unwrap().run(input()).await;
    assert!(r.is_ok(), "{r:?}");
    assert!(has(&sink, "child output on stdout"), "{:?}", sink.lines());
}

#[tokio::test]
async fn step_timeout_kills_plugin() {
    let sink = Arc::new(MemorySink::default());
    let sx = build(&sink, vec![demo("slow").timeout(Duration::from_millis(500))]).await.unwrap();
    let t = Instant::now();
    let err = sx.run(input()).await.unwrap_err();
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
    assert!(matches!(err.0[0].2, PluginError::Timeout(_)), "{err}");
    // a second call on the killed plugin fails fast
    let p = &sx.plugins()[1];
    assert_eq!(p.name(), "demo");
    let ctx = Context {
        cwd: ROOT.into(),
        env: BTreeMap::new(),
        branch: input().branch,
        commits: vec![],
        last_release: None,
        next_release: None,
        options: json!({}),
        logger: Logger::new(sink.clone(), Arc::new(Masker::from_env(&BTreeMap::new()))),
    };
    assert!(matches!(p.verify_conditions(&ctx).await, Err(PluginError::Exited(_))));
}

#[test]
fn resolution_order_builtin_path_pinned() {
    let exe = demo_exe();
    let path = OsString::from(exe.parent().unwrap());
    let pathext = Some(".EXE;.CMD");
    assert_eq!(resolve::resolve("conventional", &["conventional"], Some(&path), pathext, None), Ok(Resolved::Builtin("conventional".into())));
    assert_eq!(resolve::resolve("demo", &[], Some(&path), pathext, None), Ok(Resolved::Executable(exe.to_path_buf())));
    assert_eq!(resolve::resolve("nope", &[], Some(&path), pathext, None), Err(ResolveError::NotFound("nope".into())));

    let good = Pinned { file: exe.to_path_buf(), sha256: resolve::sha256_file(exe).unwrap() };
    assert_eq!(resolve::resolve("pinned", &[], None, pathext, Some(&good)), Ok(Resolved::Executable(exe.to_path_buf())));
    let bad = Pinned { sha256: "00".repeat(32), ..good };
    assert!(matches!(resolve::resolve("pinned", &[], None, pathext, Some(&bad)), Err(ResolveError::ChecksumMismatch { .. })));
}

#[cfg(windows)]
#[tokio::test]
async fn python_plugin_found_on_path_through_cmd_shim() {
    let sink = Arc::new(MemorySink::default());
    let sx = Semoxide::builder()
        .env(env())
        .sink(sink.clone())
        .extra_path(PathBuf::from(format!("{ROOT}/plugins/shims")))
        .named("py", json!({}))
        .build()
        .await
        .unwrap();
    assert_eq!(sx.plugins()[0].steps(), StepSet::from([Step::AnalyzeCommits, Step::GenerateNotes]));
    assert!(matches!(sx.run(input()).await.unwrap(), Outcome::Released { .. }));
}
