//! semoxide plugin SDK (PoC): message types, local-socket transport,
//! `Plugin` trait, `serve()`, and (feature `host`) the host-side launcher.

pub use semoxide_plugin_proto::v1 as pb;
pub use semoxide_plugin_proto::PROTOCOL;

pub mod plugin;
pub mod proctree;
pub mod serve;
pub mod transport;

#[cfg(feature = "host")]
pub mod launcher;

pub use async_trait::async_trait;
pub use plugin::{Host, Manifest, Plugin, PluginEnv, StepContext};
pub use serve::{ServeOptions, serve, serve_with};
pub use tonic::Status;

pub fn version(v: (u32, u32, u32)) -> pb::ProtocolVersion {
    pb::ProtocolVersion { major: v.0, minor: v.1, patch: v.2 }
}
