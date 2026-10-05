#[path = "../samples.rs"] mod samples;
use icu_collator::{Collator, options::CollatorOptions};
fn main() {
    let t0 = std::time::Instant::now();
    let c = Collator::try_new(icu_locale_core::locale!("en").into(), CollatorOptions::default()).unwrap();
    let init = t0.elapsed().as_secs_f64() * 1000.0;
    let mut s: Vec<&str> = samples::SAMPLES.to_vec(); s.sort_by(|x, y| c.compare(x, y));
    println!("ICU\t{}", s.join(" | "));
    println!("ICU_INIT_ms\t{:.3}", init);
    let mut v = samples::bench_input(100_000);
    let t = std::time::Instant::now(); v.sort_by(|x, y| c.compare(x, y));
    println!("TIME_ICU_100k_ms\t{:.2}", t.elapsed().as_secs_f64() * 1000.0);
}
