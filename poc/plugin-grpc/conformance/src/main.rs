//! `semoxide-plugin-conformance [--config <json>] <plugin-binary> [plugin args...]`
//!
//! Language-agnostic checks against any plugin binary. Exit 1 on failure.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use semoxide_plugin_sdk::launcher::{self, CallError, LaunchOptions};
use semoxide_plugin_sdk::pb::{LogLevel, Step};
use semoxide_plugin_sdk::plugin::system_env;
use semoxide_plugin_sdk::{Host, Status, async_trait};
use tonic::Code;

/// Accept-all host services.
struct NullHost;

#[async_trait]
impl Host for NullHost {
    async fn log(&self, _: LogLevel, _: String) -> Result<(), Status> {
        Ok(())
    }
    async fn git_add(&self, _: Vec<String>) -> Result<(), Status> {
        Ok(())
    }
    async fn git_commit(&self, _: String) -> Result<String, Status> {
        Ok("0".repeat(40))
    }
    async fn git_push(&self, _: String) -> Result<(), Status> {
        Ok(())
    }
}

struct Report {
    failed: bool,
}

impl Report {
    fn check(&mut self, name: &str, r: Result<String, String>) {
        match r {
            Ok(d) => println!("PASS  {name}{}", if d.is_empty() { String::new() } else { format!(" ({d})") }),
            Err(e) => {
                self.failed = true;
                println!("FAIL  {name}: {e}");
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut config = "{}".to_owned();
    if args.first().map(String::as_str) == Some("--config") {
        args.remove(0);
        config = args.remove(0);
    }
    if args.is_empty() {
        eprintln!("usage: semoxide-plugin-conformance [--config <json>] <plugin-binary> [args...]");
        std::process::exit(2);
    }
    let program = args.remove(0);
    let mut rep = Report { failed: false };
    let env: BTreeMap<String, String> = system_env(&std::env::vars().collect());

    let t = Instant::now();
    let launched = launcher::launch(LaunchOptions {
        program: program.into(),
        args,
        env,
        connect_timeout: Duration::from_secs(10),
        output: Arc::new(|_, _| {}),
        host: Arc::new(NullHost),
    })
    .await;
    let mut p = match launched {
        Ok(p) => {
            rep.check("launch + connect + handshake", Ok(format!("{:?}", t.elapsed())));
            p
        }
        Err(e) => {
            rep.check("launch + connect + handshake", Err(e.to_string()));
            std::process::exit(1);
        }
    };
    let hs = p.handshake.clone();
    rep.check(
        "protocol major matches",
        Ok(format!("{}.{}", hs.protocol.unwrap_or_default().major, hs.protocol.unwrap_or_default().minor)),
    );
    rep.check(
        "declares at least one known step",
        if p.steps.is_empty() { Err("no known steps".into()) } else { Ok(format!("{:?}", p.steps)) },
    );
    rep.check(
        "plugin name set",
        if hs.plugin_name.is_empty() { Err("empty".into()) } else { Ok(hs.plugin_name.clone()) },
    );

    let schema = p.describe().await;
    rep.check(
        "describe returns a JSON Schema object",
        match schema {
            Ok(s) => match serde_json::from_str::<serde_json::Value>(&s) {
                Ok(v) if v.get("$schema").is_some() || v.get("type").is_some() => Ok(String::new()),
                Ok(_) => Err("JSON but no $schema/type".into()),
                Err(e) => Err(e.to_string()),
            },
            Err(e) => Err(e.to_string()),
        },
    );
    rep.check("configure accepts the given config", p.configure(config, BTreeMap::new()).await.map(|_| String::new()).map_err(|e| e.to_string()));

    let deadline = Duration::from_secs(10);
    rep.check(
        "invalid step value -> INVALID_ARGUMENT",
        match p.run_step_raw(0, "{}".into(), deadline).await {
            Err(CallError::Status(s)) if s.code() == Code::InvalidArgument => Ok(String::new()),
            other => Err(format!("{other:?}")),
        },
    );
    let all = (1..=10).filter_map(|i| Step::try_from(i).ok());
    let undeclared = all.clone().find(|s| !p.steps.contains(s));
    rep.check(
        "undeclared step -> UNIMPLEMENTED",
        match undeclared {
            None => Ok("skipped: all steps declared".into()),
            Some(s) => match p.run_step(s, "{}".into(), deadline).await {
                Err(CallError::Status(st)) if st.code() == Code::Unimplemented => Ok(format!("{s:?}")),
                other => Err(format!("{other:?}")),
            },
        },
    );

    let t = Instant::now();
    let status = p.shutdown(Duration::from_secs(2)).await;
    rep.check(
        "exits on Shutdown within 2 s",
        match status {
            Some(st) => Ok(format!("{st} in {:?}", t.elapsed())),
            None => Err("had to be killed".into()),
        },
    );

    if rep.failed {
        std::process::exit(1);
    }
}
