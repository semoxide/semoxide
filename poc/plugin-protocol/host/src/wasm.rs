//! Timeboxed check: load the demo plugin as a WASI 0.2 component with wasmtime.

use std::time::Instant;

use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime::{Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

wasmtime::component::bindgen!({ path: "../wasm-plugin/wit", world: "plugin" });

struct State {
    wasi: WasiCtx,
    table: ResourceTable,
}

impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView { ctx: &mut self.wasi, table: &mut self.table }
    }
}

impl semoxide::plugin::host::Host for State {
    fn log(&mut self, level: String, message: String) {
        eprintln!("[semoxide] [wasm] {level}: {message}");
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

pub fn run(path: &str) -> wasmtime::Result<()> {
    let t = Instant::now();
    let engine = Engine::default();
    let engine_ms = ms(t);

    let t = Instant::now();
    let component = Component::from_file(&engine, path)?;
    let compile_ms = ms(t);

    // Simulate a precompile cache (what a real host would store next to the download).
    let bytes = component.serialize()?;
    let t = Instant::now();
    let component = unsafe { Component::deserialize(&engine, &bytes)? };
    let deser_ms = ms(t);

    let mut linker: Linker<State> = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
    Plugin::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s)?;

    let t = Instant::now();
    let wasi = WasiCtxBuilder::new().env("GH_TOKEN", "ghp_FAKEsecret123").build();
    let mut store = Store::new(&engine, State { wasi, table: ResourceTable::new() });
    let plugin = Plugin::instantiate(&mut store, &component, &linker)?;
    let steps = plugin.call_initialize(&mut store, 1, "{}")?;
    let inst_ms = ms(t);
    println!("steps: {steps:?}");

    let ctx = r#"{"nextRelease":{"version":"1.3.0"}}"#;
    for step in ["verifyConditions", "generateNotes", "publish"] {
        println!("{step} -> {:?}", plugin.call_call(&mut store, step, ctx)?);
    }
    let mut calls = vec![];
    for _ in 0..1000 {
        let t = Instant::now();
        plugin.call_call(&mut store, "bench", "{}")?.ok();
        calls.push(t.elapsed());
    }
    calls.sort();
    println!(
        "engine {engine_ms:.2} ms | compile (cold, no cache) {compile_ms:.1} ms | deserialize precompiled {deser_ms:.2} ms | instantiate+initialize {inst_ms:.2} ms | call median {:.4} ms",
        calls[500].as_secs_f64() * 1e3
    );
    Ok(())
}
