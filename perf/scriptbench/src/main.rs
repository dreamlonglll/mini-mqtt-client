//! Per-message cost of the after_receive script path (fresh Boa Context per run).

// The script module reports errors through the app's `tr!` macro and texts.
rust_i18n::i18n!("../../locales", fallback = "en");

macro_rules! tr {
    ($($args:tt)*) => {
        String::from(rust_i18n::t!($($args)*))
    };
}

#[allow(dead_code)]
#[path = "../../../src/backend/env.rs"]
mod env;

#[allow(dead_code)]
#[path = "../../../src/backend/script/mod.rs"]
mod script;

use std::time::Instant;

const TAG: &str = "async function process(payload, topic) {\n  try { const o = JSON.parse(payload); o.handled_by = \"script\"; return o; }\n  catch (e) { return payload; }\n}";
const PASS: &str = "function process(payload, topic) { return payload; }";
const ENV: &str = "function process(payload, topic) { return env.replace('{{dev}}:' + env.get('k5') + ':') + payload; }";
const CRYPTO: &str = "function process(payload, topic) { return crypto.sha256(payload) + crypto.crc32(payload); }";
const GUNZIP: &str = "function process(payload, topic) { return crypto.bytesToString(zlib.gunzip(payloadBytes)); }";

fn bench(name: &str, code: &str, payload: &str, n: usize, vars: &[(String, String)]) {
    bench_bytes(name, code, payload.as_bytes(), n, vars);
}

/// Received messages reach the chain as bytes, as in process_incoming.
fn bench_bytes(name: &str, code: &str, payload: &[u8], n: usize, vars: &[(String, String)]) {
    let scripts = vec![(name.to_string(), code.to_string())];
    let run = || {
        let input = script::Payload::Bytes(payload.to_vec());
        script::run_chain(&scripts, input, script::TextForm::Utf8, "perf/a", vars).unwrap()
    };
    run();
    let t = Instant::now();
    for _ in 0..n {
        let _ = run();
    }
    let per = t.elapsed().as_secs_f64() / n as f64;
    println!(
        "{name:<28} payload {:>8} B  {:>8.3} ms/run  ({:>7.0} runs/s on one thread)",
        payload.len(),
        per * 1e3,
        1.0 / per
    );
}

fn main() {
    let small = r#"{"seq":123,"ts":1793626981339,"pad":"xxxxxxxxxxxxxxxxxxxxxxxx"}"#;
    let big = format!(r#"{{"seq":1,"ts":2,"pad":"{}"}}"#, "x".repeat(100_000));
    let none: Vec<(String, String)> = Vec::new();
    let vars: Vec<(String, String)> = (0..20)
        .map(|i| (format!("k{i}"), format!("v{i}")))
        .chain([("dev".to_string(), "D1".to_string())])
        .collect();
    bench("tag-json (e2e seed script)", TAG, small, 2000, &none);
    bench("pass-through", PASS, small, 2000, &none);
    bench("env.replace/get (21 vars)", ENV, small, 2000, &vars);
    bench("crypto sha256+crc32", CRYPTO, small, 2000, &none);
    bench("tag-json 100KB", TAG, &big, 100, &none);
    let gz = script::zlib::compress(big.as_bytes(), script::zlib::Format::Gzip, 6).unwrap();
    bench_bytes("zlib.gunzip 100KB", GUNZIP, &gz, 100, &none);
    // Context creation alone
    let t = Instant::now();
    for _ in 0..2000 {
        let _ctx = boa_engine::Context::default();
    }
    println!(
        "{:<28} {:>8.3} ms each",
        "Context::default()",
        t.elapsed().as_secs_f64() / 2000.0 * 1e3
    );
}
