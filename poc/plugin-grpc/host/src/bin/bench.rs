//! `bench [N]`: median latencies of the demo plugin (release build recommended).
use std::time::{Duration, Instant};

use semoxide_host::{HostServices, LoadedPlugin, SpawnSpec, current_env};
use semoxide_plugin_sdk::pb::Step;
use serde_json::json;

fn median(mut v: Vec<f64>) -> (f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[(v.len() * 9) / 10])
}

#[tokio::main]
async fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(50);
    let exe = std::env::current_exe().unwrap();
    let demo = exe.with_file_name(format!("semoxide-plugin-demo{}", std::env::consts::EXE_SUFFIX));
    let env = current_env();
    let d = Duration::from_secs(10);
    let (mut spawn, mut hs, mut call, mut cb, mut shut, mut inproc) = (vec![], vec![], vec![], vec![], vec![], vec![]);
    for i in 0..n + 3 {
        let s = HostServices::new("v1.0.0");
        let t = Instant::now();
        let mut p = LoadedPlugin::spawn(SpawnSpec { program: demo.clone(), args: vec![], config: json!({}) }, &env, s)
            .await
            .unwrap();
        let t_spawn = t.elapsed();
        let mut c = vec![];
        for _ in 0..20 {
            let t = Instant::now();
            p.run_step(Step::AnalyzeCommits, json!({}), d).await.unwrap();
            c.push(t.elapsed().as_secs_f64() * 1e3);
        }
        let out = p.run_step(Step::VerifyRelease, json!({ "log_calls": 20 }), d).await.unwrap();
        let t = Instant::now();
        p.shutdown().await;
        if i >= 3 {
            // warm-up excluded
            spawn.push(t_spawn.as_secs_f64() * 1e3);
            hs.push(t_spawn.as_secs_f64() * 1e3);
            call.extend(c);
            cb.push(out["median_us"].as_f64().unwrap() / 1e3);
            shut.push(t.elapsed().as_secs_f64() * 1e3);
        }
    }
    let s = HostServices::new("v1.0.0");
    let mut p = LoadedPlugin::in_process(semoxide_plugin_demo::DemoPlugin::default(), json!({}), &env, s).await.unwrap();
    for _ in 0..1000 {
        let t = Instant::now();
        p.run_step(Step::AnalyzeCommits, json!({}), d).await.unwrap();
        inproc.push(t.elapsed().as_secs_f64() * 1e3);
    }
    let _ = hs;
    println!("os={} n={n}", std::env::consts::OS);
    for (name, v) in [
        ("spawn+connect+handshake+configure", spawn),
        ("step call round trip (no-op)", call),
        ("plugin->host Log round trip", cb),
        ("shutdown (RPC + exit + reap)", shut),
        ("in-process call (no-op)", inproc),
    ] {
        let (m, p90) = median(v);
        println!("{name:36} median {m:8.3} ms   p90 {p90:8.3} ms");
    }
}
