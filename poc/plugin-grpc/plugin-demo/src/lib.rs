//! Demo plugin. The same `DemoPlugin` is served as a binary (`main.rs`) and
//! linked in-process by the host tests (embedder path).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use semoxide_plugin_sdk::pb::{LogLevel, Step};
use semoxide_plugin_sdk::{Manifest, Plugin, PluginEnv, Status, StepContext, async_trait};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// Behave.
    #[default]
    Ok,
    /// `publish` never returns.
    Hang,
    /// `publish` starts a long-lived grandchild, then never returns.
    GrandchildHang,
    /// `publish` starts a long-lived grandchild, then exits with code 3.
    Crash,
    /// `publish` pushes a branch instead of the release tag.
    BadPush,
}

/// Demo plugin config (`[plugins.demo]`).
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    #[serde(default)]
    pub mode: Mode,
    /// Prefix for generated notes.
    #[serde(default)]
    pub notes_prefix: Option<String>,
}

#[derive(Default)]
pub struct DemoPlugin {
    state: Mutex<(Config, PluginEnv)>,
}

pub const SECRET: &str = "DEMO_TOKEN";

impl DemoPlugin {
    fn cfg(&self) -> Config {
        self.state.lock().unwrap().0.clone()
    }
    fn env(&self) -> PluginEnv {
        self.state.lock().unwrap().1.clone()
    }

    fn spawn_grandchild(&self) -> std::io::Result<u32> {
        let exe = std::env::current_exe()?;
        let child = std::process::Command::new(exe)
            .args(["--sleep", "600"])
            .stdin(std::process::Stdio::null())
            .spawn()?;
        Ok(child.id())
    }
}

/// Runs a child that writes 200 lines to stdout (plus the plugin's own noise).
async fn noisy(env: &PluginEnv) -> Result<(), Status> {
    for i in 0..50 {
        println!("plugin stdout noise {i} {{\"not\":\"protocol\"}}");
    }
    let mut cmd = if cfg!(windows) {
        let mut c = env.command("cmd");
        c.args(["/C", "for /L %i in (1,1,200) do @echo child noise %i"]);
        c
    } else {
        let mut c = env.command("sh");
        c.args(["-c", "i=1; while [ $i -le 200 ]; do echo child noise $i; i=$((i+1)); done"]);
        c
    };
    // Child inherits the plugin's stdout, which the host captures as logs.
    let st = cmd.status().await.map_err(|e| Status::internal(format!("noisy child: {e}")))?;
    if !st.success() {
        return Err(Status::internal(format!("noisy child: {st}")));
    }
    Ok(())
}

#[async_trait]
impl Plugin for DemoPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            name: "demo".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            steps: vec![
                Step::VerifyConditions,
                Step::AnalyzeCommits,
                Step::VerifyRelease,
                Step::GenerateNotes,
                Step::Prepare,
                Step::Publish,
                Step::Success,
            ],
            secret_env: vec![SECRET.into()],
        }
    }

    fn config_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(Config)).unwrap()
    }

    async fn configure(&self, config: Value, env: PluginEnv) -> Result<(), Status> {
        let cfg: Config = serde_json::from_value(config).map_err(|e| Status::invalid_argument(e.to_string()))?;
        *self.state.lock().unwrap() = (cfg, env);
        Ok(())
    }

    async fn run_step(&self, ctx: StepContext) -> Result<Value, Status> {
        let env = self.env();
        let cfg = self.cfg();
        match ctx.step {
            Step::VerifyConditions => {
                let token = env.get(SECRET).map(str::to_owned);
                if let Some(t) = &token {
                    // Both must come out masked on the host.
                    ctx.host.log(LogLevel::Info, format!("using token {t}")).await?;
                    eprintln!("stderr also mentions {t}");
                }
                let mut process_env: Vec<String> = std::env::vars().map(|(k, _)| k).collect();
                process_env.sort();
                Ok(json!({
                    "token_present": token.is_some(),
                    "process_env": process_env,
                    "deadline_ms": ctx.deadline.map(|d| d.as_millis() as u64),
                }))
            }
            // No-op: used to measure the per-call round trip.
            Step::AnalyzeCommits => Ok(json!("minor")),
            // Plugin -> host round trip, measured inside the plugin.
            Step::VerifyRelease => {
                let n = ctx.context.get("log_calls").and_then(Value::as_u64).unwrap_or(1);
                let mut samples = Vec::with_capacity(n as usize);
                for i in 0..n {
                    let t = Instant::now();
                    ctx.host.log(LogLevel::Debug, format!("ping {i}")).await?;
                    samples.push(t.elapsed().as_micros() as u64);
                }
                samples.sort_unstable();
                Ok(json!({ "median_us": samples.get(samples.len() / 2) }))
            }
            Step::GenerateNotes => {
                if ctx.context.get("noisy").and_then(Value::as_bool).unwrap_or(false) {
                    noisy(&env).await?;
                }
                let prefix = cfg.notes_prefix.unwrap_or_else(|| "Notes".into());
                Ok(json!(format!("{prefix} for {}", ctx.context["next_version"].as_str().unwrap_or("?"))))
            }
            Step::Prepare => {
                ctx.host.git_add(vec!["CHANGELOG.md".into()]).await?;
                let sha = ctx.host.git_commit("chore(release): demo".into()).await?;
                Ok(json!({ "sha": sha }))
            }
            Step::Publish => {
                let tag = ctx.context["git_tag"].as_str().unwrap_or("v0.0.0").to_owned();
                match cfg.mode {
                    Mode::Ok => {
                        ctx.host.git_push(format!("refs/tags/{tag}")).await?;
                        Ok(json!({ "name": "demo", "url": format!("https://example.invalid/{tag}") }))
                    }
                    Mode::BadPush => {
                        ctx.host.git_push("refs/heads/main".into()).await?;
                        Ok(Value::Null)
                    }
                    Mode::Hang => {
                        tokio::time::sleep(Duration::from_secs(3600)).await;
                        Ok(Value::Null)
                    }
                    Mode::GrandchildHang | Mode::Crash => {
                        let pid = self.spawn_grandchild().map_err(|e| Status::internal(e.to_string()))?;
                        ctx.host.log(LogLevel::Info, format!("grandchild pid {pid}")).await?;
                        if cfg.mode == Mode::Crash {
                            std::process::exit(3);
                        }
                        tokio::time::sleep(Duration::from_secs(3600)).await;
                        Ok(Value::Null)
                    }
                }
            }
            // Env proof: run the requested programs (e.g. npm, python) with
            // the plugin env and report their `--version`.
            Step::Success => {
                let progs: Vec<String> = serde_json::from_value(ctx.context["programs"].clone()).unwrap_or_default();
                let mut out = serde_json::Map::new();
                for p in progs {
                    let r = env.command(&p).arg("--version").output().await;
                    let v = match r {
                        Ok(o) => json!({
                            "ok": o.status.success(),
                            "out": String::from_utf8_lossy(&o.stdout).trim().to_owned()
                                + String::from_utf8_lossy(&o.stderr).trim(),
                            "resolved": env.resolve(&p).map(|p| p.display().to_string()),
                        }),
                        Err(e) => json!({ "ok": false, "out": e.to_string() }),
                    };
                    out.insert(p, v);
                }
                Ok(Value::Object(out))
            }
            other => Err(Status::unimplemented(format!("{other:?}"))),
        }
    }
}
