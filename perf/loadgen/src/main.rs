//! Small MQTT load generator / counter used by the perf experiments.
//!
//!   mqtt-load pub --port 28830 --topic perf/a --rate 5000 --duration 20 --size 64 --kind json
//!   mqtt-load sub --port 28830 --filter 'perf/#' --duration 60
//!
//! `pub` paces messages at `--rate` msg/s (0 = as fast as possible). Every
//! payload carries a global sequence number and the send time in epoch ms:
//!   json: {"seq":12,"ts":1730000000123,"pad":"xxxx"}
//!   text: seq=12 ts=1730000000123 xxxx
//! so the UI side can compute the display lag and the processed sequence.
//! Progress is printed once per second as JSON lines on stdout.

use rumqttc::{AsyncClient, Event, MqttOptions, Outgoing, Packet, QoS};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

fn now_s() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64()
}

struct Args(HashMap<String, String>);

impl Args {
    fn parse(items: &[String]) -> Self {
        let mut map = HashMap::new();
        let mut i = 0;
        while i < items.len() {
            let key = items[i].trim_start_matches("--").to_string();
            let value = items.get(i + 1).cloned().unwrap_or_default();
            map.insert(key, value);
            i += 2;
        }
        Args(map)
    }
    fn s(&self, k: &str, d: &str) -> String {
        self.0.get(k).cloned().unwrap_or_else(|| d.to_string())
    }
    fn n(&self, k: &str, d: f64) -> f64 {
        self.0.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
    }
}

fn qos(q: u8) -> QoS {
    match q {
        1 => QoS::AtLeastOnce,
        2 => QoS::ExactlyOnce,
        _ => QoS::AtMostOnce,
    }
}

fn make_payload(kind: &str, seq: u64, size: usize, pad: &str) -> Vec<u8> {
    let ts = now_ms();
    match kind {
        "text" => {
            let head = format!("seq={seq} ts={ts} ");
            let n = size.saturating_sub(head.len());
            let mut s = head.into_bytes();
            s.extend_from_slice(&pad.as_bytes()[..n.min(pad.len())]);
            s
        }
        _ => {
            let head = format!("{{\"seq\":{seq},\"ts\":{ts},\"pad\":\"");
            let n = size.saturating_sub(head.len() + 2);
            let mut s = head.into_bytes();
            s.extend_from_slice(&pad.as_bytes()[..n.min(pad.len())]);
            s.extend_from_slice(b"\"}");
            s
        }
    }
}

fn options(id: String, host: &str, port: u16) -> MqttOptions {
    let mut o = MqttOptions::new(id, host, port);
    o.set_keep_alive(Duration::from_secs(30));
    o.set_max_packet_size(200 * 1024 * 1024, 200 * 1024 * 1024);
    o.set_inflight(1000);
    o.set_clean_session(true);
    o
}

async fn run_pub(a: Args) {
    let host = a.s("host", "127.0.0.1");
    let port = a.n("port", 1883.0) as u16;
    let topic = a.s("topic", "perf/load");
    let topics = a.n("topics", 1.0).max(1.0) as u64;
    let rate = a.n("rate", 1000.0);
    let duration = a.n("duration", 0.0);
    let count = a.n("count", 0.0) as u64;
    let size = a.n("size", 64.0) as usize;
    let kind = a.s("kind", "json");
    let q = a.n("qos", 0.0) as u8;
    let conns = a.n("conns", 1.0).max(1.0) as u64;
    let seq_start = a.n("seq-start", 0.0) as u64;
    let id = a.s("id", &format!("perf-load-{}", std::process::id()));
    let pad: Arc<String> = Arc::new("x".repeat(size));

    let seq = Arc::new(AtomicU64::new(seq_start));
    let sent = Arc::new(AtomicU64::new(0));
    let errors = Arc::new(AtomicU64::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let start = Instant::now();
    let t_first = now_ms();

    let mut handles = Vec::new();
    for c in 0..conns {
        let (client, mut eventloop) = AsyncClient::new(options(format!("{id}-{c}"), &host, port), 10_000);
        let errors_loop = errors.clone();
        let loop_task = tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(Event::Outgoing(Outgoing::Disconnect)) => break,
                    Ok(_) => {}
                    Err(e) => {
                        errors_loop.fetch_add(1, Ordering::Relaxed);
                        eprintln!("{{\"conn_error\":\"{e}\"}}");
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
        });
        let (seq, sent, done, topic, kind, pad) =
            (seq.clone(), sent.clone(), done.clone(), topic.clone(), kind.clone(), pad.clone());
        let per_conn_rate = rate / conns as f64;
        handles.push(tokio::spawn(async move {
            let mut local: u64 = 0;
            let mut tick = tokio::time::interval(Duration::from_millis(1));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                if done.load(Ordering::Relaxed) {
                    break;
                }
                let due = if per_conn_rate > 0.0 {
                    tick.tick().await;
                    (start.elapsed().as_secs_f64() * per_conn_rate) as u64
                } else {
                    local + 256
                };
                while local < due {
                    let s = seq.fetch_add(1, Ordering::Relaxed);
                    if count > 0 && s >= seq_start + count {
                        done.store(true, Ordering::Relaxed);
                        break;
                    }
                    let t = if topics > 1 { format!("{topic}/{}", s % topics) } else { topic.clone() };
                    let payload = make_payload(&kind, s, size, &pad);
                    if client.publish(t, qos(q), false, payload).await.is_err() {
                        break;
                    }
                    local += 1;
                    sent.fetch_add(1, Ordering::Relaxed);
                }
            }
            let _ = client.disconnect().await;
            let _ = tokio::time::timeout(Duration::from_secs(30), loop_task).await;
        }));
    }

    // Progress reporter.
    let (sent_r, done_r) = (sent.clone(), done.clone());
    let reporter = tokio::spawn(async move {
        let mut last = 0;
        let mut iv = tokio::time::interval(Duration::from_secs(1));
        iv.tick().await;
        loop {
            iv.tick().await;
            let now = sent_r.load(Ordering::Relaxed);
            println!("{{\"t\":{:.3},\"sent\":{},\"rate\":{}}}", now_s(), now, now - last);
            last = now;
            if done_r.load(Ordering::Relaxed) {
                break;
            }
        }
    });

    if duration > 0.0 {
        let done_d = done.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs_f64(duration)).await;
            done_d.store(true, Ordering::Relaxed);
        });
    }
    for h in handles {
        let _ = h.await;
    }
    done.store(true, Ordering::Relaxed);
    reporter.abort();
    let total = sent.load(Ordering::Relaxed);
    let elapsed = start.elapsed().as_secs_f64();
    println!(
        "{{\"done\":true,\"sent\":{},\"elapsed\":{:.3},\"rate\":{:.1},\"t_first\":{},\"t_last\":{},\"seq_start\":{},\"seq_last\":{},\"errors\":{}}}",
        total,
        elapsed,
        total as f64 / elapsed,
        t_first,
        now_ms(),
        seq_start,
        seq_start + total.saturating_sub(1),
        errors.load(Ordering::Relaxed)
    );
}

fn extract_ts(payload: &[u8]) -> Option<u64> {
    let s = std::str::from_utf8(&payload[..payload.len().min(80)]).ok()?;
    let i = s.find("ts")? + 2;
    let digits: String = s[i..].chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn pct(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * p).round() as usize]
}

async fn run_sub(a: Args) {
    let host = a.s("host", "127.0.0.1");
    let port = a.n("port", 1883.0) as u16;
    let filter = a.s("filter", "#");
    let duration = a.n("duration", 0.0);
    let q = a.n("qos", 0.0) as u8;
    let id = a.s("id", &format!("perf-sub-{}", std::process::id()));
    let (client, mut eventloop) = AsyncClient::new(options(id, &host, port), 1000);
    client.subscribe(filter, qos(q)).await.unwrap();
    let start = Instant::now();
    let deadline = if duration > 0.0 { Some(start + Duration::from_secs_f64(duration)) } else { None };
    let mut per_topic: HashMap<String, u64> = HashMap::new();
    let mut arrivals: Vec<f64> = Vec::new();
    let mut lat: Vec<f64> = Vec::new();
    let (mut total, mut last_report, mut last_count) = (0u64, Instant::now(), 0u64);
    let (mut first_ms, mut last_ms) = (0u64, 0u64);
    loop {
        let ev = match deadline {
            Some(d) => match tokio::time::timeout_at(d.into(), eventloop.poll()).await {
                Ok(ev) => ev,
                Err(_) => break,
            },
            None => eventloop.poll().await,
        };
        match ev {
            Ok(Event::Incoming(Packet::Publish(p))) => {
                let now = now_ms();
                if total == 0 {
                    first_ms = now;
                }
                last_ms = now;
                total += 1;
                *per_topic.entry(p.topic.clone()).or_default() += 1;
                if arrivals.len() < 5_000_000 {
                    arrivals.push(start.elapsed().as_secs_f64());
                    if let Some(ts) = extract_ts(&p.payload) {
                        lat.push(now.saturating_sub(ts) as f64);
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("{{\"conn_error\":\"{e}\"}}");
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
        if last_report.elapsed() >= Duration::from_secs(1) {
            println!("{{\"t\":{:.3},\"count\":{},\"rate\":{}}}", now_s(), total, total - last_count);
            last_count = total;
            last_report = Instant::now();
        }
    }
    let mut gaps: Vec<f64> = arrivals.windows(2).map(|w| (w[1] - w[0]) * 1000.0).collect();
    let span = if total > 1 { (last_ms - first_ms) as f64 / 1000.0 } else { 0.0 };
    let topics: Vec<String> = per_topic.iter().map(|(t, c)| format!("\"{t}\":{c}")).collect();
    println!(
        "{{\"done\":true,\"count\":{},\"span_s\":{:.3},\"rate\":{:.1},\"first_ms\":{},\"last_ms\":{},\"gap_ms_p50\":{:.2},\"gap_ms_p99\":{:.2},\"gap_ms_max\":{:.2},\"lat_ms_p50\":{:.1},\"lat_ms_p99\":{:.1},\"topics\":{{{}}}}}",
        total,
        span,
        if span > 0.0 { (total - 1) as f64 / span } else { 0.0 },
        first_ms,
        last_ms,
        pct(&mut gaps, 0.5),
        pct(&mut gaps, 0.99),
        gaps.iter().cloned().fold(0.0, f64::max),
        pct(&mut lat, 0.5),
        pct(&mut lat, 0.99),
        topics.join(",")
    );
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let mode = argv.get(1).cloned().unwrap_or_default();
    let args = Args::parse(&argv[2.min(argv.len())..]);
    match mode.as_str() {
        "pub" => run_pub(args).await,
        "sub" => run_sub(args).await,
        _ => {
            eprintln!("usage: mqtt-load pub|sub [--key value ...]");
            std::process::exit(2);
        }
    }
}
