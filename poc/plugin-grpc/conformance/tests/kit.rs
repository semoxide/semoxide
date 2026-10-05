use std::path::PathBuf;
use std::process::Command;

fn demo_bin() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_BIN_EXE_semoxide-plugin-conformance")).parent().unwrap().to_path_buf();
    let bin = dir.join(format!("semoxide-plugin-demo{}", std::env::consts::EXE_SUFFIX));
    if !bin.exists() {
        assert!(Command::new(env!("CARGO")).args(["build", "-p", "semoxide-plugin-demo"]).status().unwrap().success());
    }
    bin
}

fn kit(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_semoxide-plugin-conformance")).arg(demo_bin()).args(args).output().unwrap();
    (out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned())
}

#[test]
fn demo_plugin_passes() {
    let (ok, out) = kit(&[]);
    println!("{out}");
    assert!(ok, "{out}");
    assert!(!out.contains("FAIL"));
}

#[test]
fn wrong_major_fails() {
    let (ok, out) = kit(&["--fake-protocol", "2.0.0"]);
    assert!(!ok, "{out}");
    assert!(out.contains("FAIL  launch + connect + handshake: incompatible protocol"), "{out}");
}
