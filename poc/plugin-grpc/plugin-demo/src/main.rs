//! `semoxide-plugin-demo --socket <addr> [--fake-protocol X.Y.Z] [--fake-step N]`
//! `semoxide-plugin-demo --sleep <secs>` (grandchild used by the kill tests)

use semoxide_plugin_sdk::ServeOptions;
use semoxide_plugin_demo::DemoPlugin;

fn arg(name: &str) -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == name {
            return args.next();
        }
    }
    None
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(secs) = arg("--sleep") {
        std::thread::sleep(std::time::Duration::from_secs(secs.parse()?));
        return Ok(());
    }
    let mut opts = ServeOptions::default();
    if let Some(v) = arg("--fake-protocol") {
        let p: Vec<u32> = v.split('.').map(|x| x.parse()).collect::<Result<_, _>>()?;
        opts.advertise_protocol = Some((p[0], p[1], p[2]));
    }
    if let Some(s) = arg("--fake-step") {
        opts.extra_raw_steps.push(s.parse()?);
    }
    semoxide_plugin_sdk::serve_with(DemoPlugin::default(), opts).await
}
