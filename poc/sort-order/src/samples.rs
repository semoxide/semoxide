pub const SAMPLES: &[&str] = &[
    "add x", "Bump y", "fix z", "Zebra", "apple", "Äpfel", "éclair", "eclair", "Eclair",
    "ünicode", "uber", "Über", "ß straße", "ss", "10 items", "2 items", "_private", "-dash",
    "Ångström", "angstrom", "naïve", "naive", "résumé", "resume", "co-op", "coop", "Coop",
    "日本", "中文", "ёлка", "елка", "Ёж", "a", "A", "b", "B",
];
pub fn bench_input(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("{} {}", SAMPLES[i % SAMPLES.len()], (i * 7919) % 1000)).collect()
}
