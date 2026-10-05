//! semoxide-poc demo | bench [N] | wasm <component.wasm>

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::json;
use semoxide_poc_host::plugin::*;
use semoxide_poc_host::process::{PluginCommand, ProcessPlugin};
use semoxide_poc_host::{Outcome, RunInput, Semoxide};

const PY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../plugins/python/semoxide-plugin-py.py");
const SHIM: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../plugins/shims/semoxide-plugin-py.cmd");

/// An embedder's own plugin, registered through the builder.
struct EmbedderPublish;

#[async_trait]
impl Plugin for EmbedderPublish {
    fn name(&self) -> &str {
        "embedder"
    }
    fn steps(&self) -> StepSet {
        [Step::Publish].into()
    }
    async fn publish(&self, ctx: &Context) -> Result<Option<PublishResult>, PluginError> {
        let v = &ctx.next_release.as_ref().unwrap().version;
        ctx.logger.info(&format!("embedder publishing {v}"));
        Ok(Some(PublishResult { name: Some("embedder store".into()), url: None, channel: None }))
    }
}

fn exe_dir() -> PathBuf {
    std::env::current_exe().unwrap().parent().unwrap().to_path_buf()
}

fn env_with_secret() -> BTreeMap<String, String> {
    let mut env: BTreeMap<_, _> = std::env::vars().collect();
    env.insert("GH_TOKEN".into(), "ghp_FAKEsecret123".into());
    env
}

fn sample_input(n: usize) -> RunInput {
    let mut commits = vec![
        Commit { hash: "a1b2c3d4e5".into(), message: "feat: add thing".into() },
        Commit { hash: "f6e5d4c3b2".into(), message: "fix: edge case".into() },
    ];
    for i in 0..n.saturating_sub(2) {
        commits.push(Commit { hash: format!("{i:010}"), message: format!("chore: filler commit {i}\n\nbody text") });
    }
    RunInput {
        branch: Branch { name: "main".into(), channel: None },
        commits,
        last_release: Some(LastRelease { version: "1.2.3".into(), git_tag: "v1.2.3".into(), git_head: "0000000".into() }),
    }
}

async fn demo() {
    let sx = Semoxide::builder()
        .env(env_with_secret())
        .sink(Arc::new(StderrSink { debug: true }))
        .extra_path(exe_dir())
        .named("conventional", json!({}))
        .named("demo", json!({"registry": "x"}))
        .command(PluginCommand::new("python", "python").args([PY]))
        .plugin(EmbedderPublish)
        .build()
        .await
        .expect("build");
    for p in sx.plugins() {
        println!("plugin {:<12} steps {:?}", p.name(), p.steps());
    }
    match sx.run(sample_input(2)).await {
        Ok(Outcome::Released { next, releases }) => {
            println!("released {} ({:?})\n--- notes ---\n{}\n--- releases ---", next.version, next.kind, next.notes);
            for (p, r) in releases {
                println!("{p}: {r:?}");
            }
        }
        Ok(Outcome::NoRelease) => println!("no release"),
        Err(e) => println!("run failed: {e}"),
    }
}

fn stats(mut v: Vec<Duration>) -> String {
    v.sort();
    let us = |d: Duration| d.as_secs_f64() * 1e3;
    format!("median {:.3} ms, p90 {:.3} ms, min {:.3} ms (n={})", us(v[v.len() / 2]), us(v[v.len() * 9 / 10]), us(v[0]), v.len())
}

async fn bench(n: usize) {
    let env: BTreeMap<_, _> = std::env::vars().collect();
    let cwd = std::env::current_dir().unwrap();
    let logger = Logger::new(Arc::new(MemorySink::default()), Arc::new(Masker::from_env(&env)));
    let demo = exe_dir().join("semoxide-plugin-demo.exe");
    let cmds = [
        ("rust demo (.exe)", PluginCommand::new("demo", &demo)),
        ("python (python.exe script)", PluginCommand::new("py", "python").args([PY])),
        ("python via .cmd shim", PluginCommand::new("py", SHIM)),
    ];
    let ctx = Context {
        cwd: cwd.clone(),
        env: BTreeMap::new(),
        branch: Branch { name: "main".into(), channel: None },
        commits: sample_input(100).commits,
        last_release: None,
        next_release: None,
        options: json!({}),
        logger: logger.clone(),
    };
    println!("ctx with 100 commits = {} bytes JSON", serde_json::to_string(&ctx).unwrap().len());
    for (label, cmd) in &cmds {
        let mut spawn = vec![];
        for _ in 0..n {
            let t = Instant::now();
            let p = ProcessPlugin::spawn(cmd, &env, &cwd, &logger).await.expect("spawn");
            spawn.push(t.elapsed());
            p.shutdown().await;
        }
        let p = ProcessPlugin::spawn(cmd, &env, &cwd, &logger).await.unwrap();
        let (mut empty, mut full) = (vec![], vec![]);
        for _ in 0..n * 10 {
            let t = Instant::now();
            p.request("bench", &json!({})).await.unwrap();
            empty.push(t.elapsed());
            let t = Instant::now();
            p.request("bench", &json!({ "context": ctx })).await.unwrap();
            full.push(t.elapsed());
        }
        p.shutdown().await;
        println!("{label}\n  spawn+handshake: {}\n  call (empty):    {}\n  call (100-commit ctx): {}", stats(spawn), stats(empty), stats(full));
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("bench") => bench(args.get(2).and_then(|n| n.parse().ok()).unwrap_or(30)).await,
        #[cfg(feature = "wasm")]
        Some("wasm") => semoxide_poc_host::wasm::run(&args[2]).expect("wasm"),
        _ => demo().await,
    }
}
