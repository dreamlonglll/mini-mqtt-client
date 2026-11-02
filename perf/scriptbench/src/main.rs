//! Per-message cost of the after_receive script path (fresh Boa Context per run).
#[allow(dead_code)]
#[path = "../../../src/backend/script/mod.rs"]
mod script;

use std::time::Instant;

const TAG: &str = "async function process(payload, topic) {\n  try { const o = JSON.parse(payload); o.handled_by = \"script\"; return o; }\n  catch (e) { return payload; }\n}";
const PASS: &str = "function process(payload, topic) { return payload; }";

fn bench(name: &str, code: &str, payload: &str, n: usize) {
    let scripts = vec![(name.to_string(), code.to_string())];
    let _ = script::run_chain(&scripts, payload, "perf/a");
    let t = Instant::now();
    for _ in 0..n {
        let _ = script::run_chain(&scripts, payload, "perf/a").unwrap();
    }
    let per = t.elapsed().as_secs_f64() / n as f64;
    println!("{name:<28} payload {:>8} B  {:>8.3} ms/run  ({:>7.0} runs/s on one thread)", payload.len(), per * 1e3, 1.0 / per);
}

fn main() {
    let small = r#"{"seq":123,"ts":1793626981339,"pad":"xxxxxxxxxxxxxxxxxxxxxxxx"}"#;
    let big = format!(r#"{{"seq":1,"ts":2,"pad":"{}"}}"#, "x".repeat(100_000));
    bench("tag-json (e2e seed script)", TAG, small, 2000);
    bench("pass-through", PASS, small, 2000);
    bench("tag-json 100KB", TAG, &big, 100);
    // Context creation alone
    let t = Instant::now();
    for _ in 0..2000 {
        let _ctx = boa_engine::Context::default();
    }
    println!("{:<28} {:>8.3} ms each", "Context::default()", t.elapsed().as_secs_f64() / 2000.0 * 1e3);
}
