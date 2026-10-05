wit_bindgen::generate!({ path: "wit", world: "plugin" });

use semoxide::plugin::host::log;
use serde_json::{Value, json};

struct Demo;

impl Guest for Demo {
    fn initialize(protocol: u32, _config: String) -> Result<Vec<String>, String> {
        if protocol != 1 {
            return Err(format!("unsupported protocol {protocol}"));
        }
        Ok(vec!["verifyConditions".into(), "generateNotes".into(), "publish".into()])
    }

    fn call(step: String, context: String) -> Result<String, String> {
        let ctx: Value = serde_json::from_str(&context).map_err(|e| e.to_string())?;
        match step.as_str() {
            "verifyConditions" => {
                // Only env vars the host explicitly passed are visible.
                let tok = std::env::var("GH_TOKEN");
                log("info", &format!("GH_TOKEN visible: {}", tok.is_ok()));
                let home = std::fs::read_dir(".").map(|d| d.count());
                log("info", &format!("read_dir(\".\"): {home:?}"));
                Ok("null".into())
            }
            "generateNotes" => {
                let v = ctx["nextRelease"]["version"].as_str().unwrap_or("?");
                Ok(json!(format!("### Wasm plugin\n- built {v}")).to_string())
            }
            "publish" => {
                // Can a WASI component run `npm publish`?
                let r = std::process::Command::new("npm").arg("--version").output();
                log("info", &format!("spawn npm: {:?}", r.map(|o| o.status)));
                Ok("false".into())
            }
            "bench" => Ok("null".into()),
            other => Err(format!("not implemented: {other}")),
        }
    }
}

export!(Demo);
