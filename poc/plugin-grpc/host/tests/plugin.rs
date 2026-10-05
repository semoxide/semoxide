use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use semoxide_host::{HostError, HostServices, LoadedPlugin, SpawnSpec, current_env};
use semoxide_plugin_sdk::launcher::{CallError, LaunchError};
use semoxide_plugin_sdk::pb::{self, Step};
use semoxide_plugin_sdk::proctree::is_alive;
use serde_json::{Value, json};
use tonic::Code;

const TOKEN: &str = "s3cr3t-demo-token-value";
const TAG: &str = "v1.1.0";

/// The demo binary sits next to the test's deps dir. A workspace `cargo test`
/// builds it; otherwise build it once.
fn demo_bin() -> PathBuf {
    let dir = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
    let bin = dir.join(format!("semoxide-plugin-demo{}", std::env::consts::EXE_SUFFIX));
    if !bin.exists() {
        let st = std::process::Command::new(env!("CARGO"))
            .args(["build", "-p", "semoxide-plugin-demo"])
            .status()
            .unwrap();
        assert!(st.success());
    }
    bin
}

fn host_env() -> BTreeMap<String, String> {
    let mut env = current_env();
    env.insert("DEMO_TOKEN".into(), TOKEN.into());
    env.insert("UNDECLARED_SECRET".into(), "nope".into());
    env
}

async fn spawn(config: Value, args: &[&str]) -> (LoadedPlugin, Arc<HostServices>) {
    let services = HostServices::new(TAG);
    let p = LoadedPlugin::spawn(
        SpawnSpec { program: demo_bin(), args: args.iter().map(|s| s.to_string()).collect(), config },
        &host_env(),
        services.clone(),
    )
    .await
    .expect("spawn");
    (p, services)
}

fn ctx() -> Value {
    json!({ "next_version": "1.1.0", "git_tag": TAG })
}

const D: Duration = Duration::from_secs(20);

fn grandchild_pid(s: &HostServices) -> u32 {
    s.logs()
        .iter()
        .find_map(|l| l.message.strip_prefix("grandchild pid ").map(|p| p.parse().unwrap()))
        .expect("grandchild pid logged")
}

async fn wait_dead(pid: u32) -> bool {
    for _ in 0..50 {
        if !is_alive(pid) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

#[tokio::test]
async fn full_flow_env_secrets_masking_host_services() {
    let (mut p, s) = spawn(json!({}), &[]).await;
    assert_eq!(p.name, "demo");
    assert_eq!(p.secret_env, vec!["DEMO_TOKEN"]);

    let schema = p.describe().await.unwrap();
    assert!(schema["properties"]["mode"].is_object(), "{schema}");

    let out = p.run_step(Step::VerifyConditions, ctx(), Duration::from_secs(30)).await.unwrap();
    assert_eq!(out["token_present"], true);
    // gRPC deadline reached the plugin.
    let ms = out["deadline_ms"].as_u64().unwrap();
    assert!(ms > 25_000 && ms <= 30_000, "{ms}");
    let penv: Vec<String> = serde_json::from_value(out["process_env"].clone()).unwrap();
    assert!(!penv.iter().any(|k| k == "UNDECLARED_SECRET"), "{penv:?}");
    // Secrets are delivered over the channel, not in the process env.
    assert!(!penv.iter().any(|k| k == "DEMO_TOKEN"), "{penv:?}");
    if cfg!(windows) {
        for k in ["SystemRoot", "PATHEXT"] {
            assert!(penv.iter().any(|e| e.eq_ignore_ascii_case(k)), "{k} missing: {penv:?}");
        }
    }

    // Plugin -> host calls while the host waits on the step call.
    let out = p.run_step(Step::Prepare, ctx(), D).await.unwrap();
    assert_eq!(out["sha"].as_str().unwrap().len(), 40);
    assert_eq!(s.commits(), vec!["chore(release): demo"]);
    let out = p.run_step(Step::Publish, ctx(), D).await.unwrap();
    assert_eq!(out["name"], "demo");
    assert_eq!(s.pushed(), vec![format!("refs/tags/{TAG}")]);

    let st = p.shutdown().await.expect("clean exit");
    assert!(st.success());

    let logs = s.logs();
    assert!(logs.iter().any(|l| l.source == "plugin" && l.message == "using token [secure]"), "{logs:#?}");
    assert!(logs.iter().any(|l| l.level == "stderr" && l.message == "stderr also mentions [secure]"), "{logs:#?}");
    assert!(!logs.iter().any(|l| l.message.contains(TOKEN)));
}

#[tokio::test]
async fn noisy_child_stdout_does_not_break_protocol() {
    let (mut p, s) = spawn(json!({ "notes-prefix": "Release" }), &[]).await;
    let mut c = ctx();
    c["noisy"] = json!(true);
    let out = p.run_step(Step::GenerateNotes, c, D).await.unwrap();
    assert_eq!(out, json!("Release for 1.1.0"));
    p.shutdown().await;
    let logs = s.logs();
    let stdout: Vec<_> = logs.iter().filter(|l| l.level == "stdout").map(|l| l.message.as_str()).collect();
    assert!(stdout.contains(&"child noise 200"), "{stdout:?}");
    assert!(stdout.iter().any(|l| l.starts_with("plugin stdout noise 49")), "{stdout:?}");
    assert_eq!(stdout.iter().filter(|l| l.starts_with("child noise")).count(), 200);
}

#[tokio::test]
async fn timeout_kills_process_tree() {
    let (mut p, s) = spawn(json!({ "mode": "grandchild-hang" }), &[]).await;
    let plugin_pid = p.pid().unwrap();
    let t = Instant::now();
    let err = p.run_step(Step::Publish, ctx(), Duration::from_millis(1500)).await.unwrap_err();
    let took = t.elapsed();
    assert!(matches!(err, HostError::Call(CallError::Timeout(_))), "{err:?}");
    assert!(took < Duration::from_secs(5), "{took:?}");
    let gc = grandchild_pid(&s);
    assert!(wait_dead(gc).await, "grandchild {gc} survived");
    assert!(wait_dead(plugin_pid).await, "plugin survived");
    // Plugin is dead now: later calls fail fast.
    let err = p.run_step(Step::AnalyzeCommits, ctx(), D).await.unwrap_err();
    assert!(matches!(err, HostError::Call(CallError::Dead)), "{err:?}");
}

#[tokio::test]
async fn crash_kills_process_tree() {
    let (mut p, s) = spawn(json!({ "mode": "crash" }), &[]).await;
    let err = p.run_step(Step::Publish, ctx(), D).await.unwrap_err();
    match &err {
        HostError::Call(CallError::Crashed { exit, .. }) => assert!(exit.contains('3'), "{exit}"),
        other => panic!("{other:?}"),
    }
    let gc = grandchild_pid(&s);
    assert!(wait_dead(gc).await, "grandchild {gc} survived");
}

#[tokio::test]
async fn host_rule_rejects_bad_push_and_plugin_survives() {
    let (mut p, s) = spawn(json!({ "mode": "bad-push" }), &[]).await;
    let err = p.run_step(Step::Publish, ctx(), D).await.unwrap_err();
    match err {
        HostError::Call(CallError::Status(st)) => assert_eq!(st.code(), Code::PermissionDenied, "{st:?}"),
        other => panic!("{other:?}"),
    }
    assert!(s.pushed().is_empty());
    assert_eq!(p.run_step(Step::AnalyzeCommits, ctx(), D).await.unwrap(), json!("minor"));
    p.shutdown().await.unwrap();
}

#[tokio::test]
async fn protocol_major_mismatch_is_rejected() {
    let services = HostServices::new(TAG);
    let err = LoadedPlugin::spawn(
        SpawnSpec { program: demo_bin(), args: vec!["--fake-protocol".into(), "2.0.0".into()], config: json!({}) },
        &host_env(),
        services,
    )
    .await
    .err()
    .expect("must fail");
    assert!(matches!(err, HostError::Launch(LaunchError::Protocol { .. })), "{err:?}");
}

#[tokio::test]
async fn newer_minor_and_unknown_step_are_accepted() {
    let (mut p, _) = spawn(json!({}), &["--fake-protocol", "1.7.3", "--fake-step", "42"]).await;
    assert!(p.steps.contains(&Step::Publish));
    assert_eq!(p.steps.len(), 7, "unknown step 42 ignored: {:?}", p.steps);
    assert_eq!(p.run_step(Step::AnalyzeCommits, ctx(), D).await.unwrap(), json!("minor"));
    p.shutdown().await.unwrap();
}

#[test]
fn unknown_protobuf_fields_are_ignored() {
    use prost::Message;
    let msg = pb::HandshakeResponse {
        protocol: Some(pb::ProtocolVersion { major: 1, minor: 2, patch: 0 }),
        plugin_name: "x".into(),
        steps: vec![6, 42],
        ..Default::default()
    };
    let mut bytes = msg.encode_to_vec();
    // A field from a future version: #99, length-delimited "hello".
    prost::encoding::string::encode(99, &"hello".to_string(), &mut bytes);
    let back = pb::HandshakeResponse::decode(bytes.as_slice()).unwrap();
    assert_eq!(back.plugin_name, "x");
    assert_eq!(back.steps, vec![6, 42]);
}

#[tokio::test]
async fn children_run_with_filtered_env() {
    // Windows: npm is npm.cmd (PATHEXT), python needs SystemRoot.
    let wanted: &[&str] = if cfg!(windows) { &["npm", "python"] } else { &["python3", "git"] };
    let env = semoxide_plugin_sdk::PluginEnv { vars: host_env() };
    let present: Vec<&str> = wanted.iter().copied().filter(|p| env.resolve(p).is_some()).collect();
    if present.len() < wanted.len() {
        eprintln!("SKIPPED programs not on PATH: {:?}", wanted.iter().filter(|p| !present.contains(p)).collect::<Vec<_>>());
    }
    let (mut p, _) = spawn(json!({}), &[]).await;
    let out = p.run_step(Step::Success, json!({ "programs": present }), Duration::from_secs(60)).await.unwrap();
    for prog in &present {
        assert_eq!(out[prog]["ok"], true, "{prog}: {}", out[prog]);
        eprintln!("{prog}: {}", out[prog]);
    }
    p.shutdown().await.unwrap();
}

#[tokio::test]
async fn in_process_same_trait() {
    let services = HostServices::new(TAG);
    let mut p = LoadedPlugin::in_process(
        semoxide_plugin_demo::DemoPlugin::default(),
        json!({ "notes-prefix": "Embedded" }),
        &host_env(),
        services.clone(),
    )
    .await
    .unwrap();
    assert!(p.pid().is_none());
    assert!(p.describe().await.unwrap()["properties"]["mode"].is_object());
    let out = p.run_step(Step::VerifyConditions, ctx(), D).await.unwrap();
    assert_eq!(out["token_present"], true);
    assert_eq!(p.run_step(Step::GenerateNotes, ctx(), D).await.unwrap(), json!("Embedded for 1.1.0"));
    p.run_step(Step::Prepare, ctx(), D).await.unwrap();
    p.run_step(Step::Publish, ctx(), D).await.unwrap();
    assert_eq!(services.pushed(), vec![format!("refs/tags/{TAG}")]);
    assert!(services.logs().iter().any(|l| l.message == "using token [secure]"));
}

#[tokio::test]
async fn in_process_timeout() {
    let services = HostServices::new(TAG);
    let mut p = LoadedPlugin::in_process(
        semoxide_plugin_demo::DemoPlugin::default(),
        json!({ "mode": "hang" }),
        &host_env(),
        services,
    )
    .await
    .unwrap();
    let err = p.run_step(Step::Publish, ctx(), Duration::from_millis(200)).await.unwrap_err();
    assert!(matches!(err, HostError::InProcessTimeout(_)), "{err:?}");
}
