// Fixture for rules.yaml: `semgrep --test --config .semgrep/rules.yaml .semgrep/rules.rs`.
// `ruleid:` marks the next line as a required finding, `ok:` as a required non-finding.
// Not compiled; it only has to parse as Rust.

use std::process::Command;

// ruleid: no-inline-test-module
#[cfg(test)]
mod inline_tests {
    fn t() {}
}

// ok: no-pub-struct-field
pub struct Good {
    name: String,
}
pub struct Bad {
    // ruleid: no-pub-struct-field
    pub name: String,
    inner: u8,
}

// ok: no-pub-tuple-field
pub struct Tag(String);
// ruleid: no-pub-tuple-field
pub struct Tag2(pub String);

impl Good {
    // ruleid: no-get-prefix
    pub fn get_name(&self) -> &str {
        &self.name
    }
    // ok: no-get-prefix
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Foreign for Good {
    // nosemgrep: no-get-prefix
    fn get_thing(&self) -> u8 {
        1
    }
}

fn spawn(p: &str, e: &Env) {
    // ruleid: command-needs-env-clear
    let a = Command::new(p).arg("x").output();
    // ok: command-needs-env-clear
    let b = Command::new(p).env_clear().envs(e.iter()).output();
    // ok: command-needs-env-clear
    let mut c = Command::new(p);
    c.env_clear();
    c.envs(e.iter());
}

fn logs(tok: &Secret, url: &Url) {
    // ruleid: no-exposed-secret-in-macro
    tracing::info!(token = tok.expose_secret(), "x");
    // ruleid: no-exposed-secret-in-macro
    let s = format!("Bearer {}", tok.expose_secret());
    // ok: no-exposed-secret-in-macro
    let n = tok.expose_secret().len();
    // ruleid: url-in-log-needs-url-for-log
    info!(%url, "fetch");
    // ok: url-in-log-needs-url-for-log
    info!(url = %url_for_log(&url), "fetch");
    // ruleid: url-in-log-needs-url-for-log
    debug!("pushing to {}", url);
}

fn regexes() {
    // ruleid: regex-unicode-digit
    let a = regex::Regex::new(r"^v(\d+)$");
    // ok: regex-unicode-digit
    let b = Regex::new("^[0-9]+$");
    // ok: regex-unicode-digit
    let c = fancy_regex::Regex::new(r"(?-u:\d)+");
}

// ruleid: tokio-test-start-paused
#[tokio::test]
async fn real_clock() {}

// ok: tokio-test-start-paused
#[tokio::test(start_paused = true)]
async fn paused_clock() {}
