fn main() -> Result<(), Box<dyn std::error::Error>> {
    // protox: pure-Rust protobuf compiler, so no `protoc` is needed (Docker/CI).
    let fds = protox::compile(["semoxide/plugin/v1/plugin.proto"], ["proto"])?;
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_fds(fds)?;
    println!("cargo:rerun-if-changed=proto");
    Ok(())
}
