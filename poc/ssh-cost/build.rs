fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fds = protox::compile(["stub.proto"], ["proto"])?;
    tonic_prost_build::configure().build_server(true).build_client(true).compile_fds(fds)?;
    println!("cargo:rerun-if-changed=proto");
    Ok(())
}
