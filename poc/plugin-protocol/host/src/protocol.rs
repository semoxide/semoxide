//! Wire types: JSON-RPC 2.0, one JSON object per line on stdio.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Protocol versions this host speaks. A plugin answering anything else is rejected.
pub const SUPPORTED_PROTOCOLS: &[u32] = &[1];
pub const PROTOCOL_VERSION: u32 = 1;

/// JSON-RPC error codes used by the protocol.
pub mod codes {
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// A plugin-reported release error (semantic-release `SemanticReleaseError`); `data.code` holds the `E...` code.
    pub const PLUGIN_ERROR: i64 = -32000;
    /// Plugin does not speak the requested protocol version.
    pub const UNSUPPORTED_PROTOCOL: i64 = -32001;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Step {
    VerifyConditions,
    AnalyzeCommits,
    GenerateNotes,
    Publish,
}

impl Step {
    pub const ALL: [Step; 4] = [
        Step::VerifyConditions,
        Step::AnalyzeCommits,
        Step::GenerateNotes,
        Step::Publish,
    ];
    pub fn method(self) -> &'static str {
        match self {
            Step::VerifyConditions => "verifyConditions",
            Step::AnalyzeCommits => "analyzeCommits",
            Step::GenerateNotes => "generateNotes",
            Step::Publish => "publish",
        }
    }
    pub fn from_method(s: &str) -> Option<Step> {
        Step::ALL.into_iter().find(|st| st.method() == s)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams<'a> {
    pub protocol: u32,
    pub host_version: &'a str,
    pub plugin_config: &'a Value,
}

#[derive(Debug, Deserialize)]
pub struct InitializeResult {
    pub protocol: u32,
    pub name: String,
    /// Strings, not `Step`, so a plugin declaring a step this host doesn't know is not an error.
    pub steps: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct LogParams {
    pub level: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

/// Any line the plugin writes. Classified by which fields are present.
#[derive(Debug, Deserialize)]
pub struct Incoming {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub params: Option<Value>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<RpcError>,
}

pub fn request(id: u64, method: &str, params: &impl Serialize) -> String {
    serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
}

pub fn notification(method: &str, params: &impl Serialize) -> String {
    serde_json::json!({"jsonrpc": "2.0", "method": method, "params": params}).to_string()
}
