#[path = "../samples.rs"] mod samples;
use std::cmp::Ordering;
fn cmp_b(a: &str, b: &str) -> Ordering {
    a.chars().flat_map(char::to_lowercase).cmp(b.chars().flat_map(char::to_lowercase)).then_with(|| a.cmp(b))
}
fn main() {
    let mut bytes: Vec<&str> = samples::SAMPLES.to_vec(); bytes.sort();
    let mut b: Vec<&str> = samples::SAMPLES.to_vec(); b.sort_by(|x, y| cmp_b(x, y));
    println!("BYTES\t{}", bytes.join(" | "));
    println!("B\t{}", b.join(" | "));
    let mut v = samples::bench_input(100_000);
    let t = std::time::Instant::now(); v.sort_by(|x, y| cmp_b(x, y));
    println!("TIME_B_100k_ms\t{:.2}", t.elapsed().as_secs_f64() * 1000.0);
}
