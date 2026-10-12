//! `sn76489-bench [--seconds S] [--repeat N] [--chunk C] <manifest.json>...`: the
//! time each manifest takes, best of `--repeat`, and their total.
//!
//! Each manifest runs in its own variant, for its first `--seconds` of emulated time,
//! driven as a board drives the chip: `run()` up to each write and otherwise in
//! pieces of at most `--chunk` input clocks (a board syncs the PSG on every sound
//! access and at least once a scanline: 228 clocks on the Mega Drive), then the
//! samples taken as levels. `check` hashes every workload's samples, so an A/B can
//! confirm both sides made the same sound. Real time is 3,579,545 input clocks a
//! second (NTSC). The machine is shared, so best-of recovers the undisturbed rate.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;

use sn76489_rust::conformance::{load_manifest, variant, Kind, Manifest};
use sn76489_rust::SN76489;

const REAL_TIME: f64 = 3_579_545.0;

fn run_to(chip: &mut SN76489, until: u64, chunk: u64) {
    while chip.clock < until {
        chip.run((until - chip.clock).min(chunk));
    }
}

/// One timed run: the seconds it took and a hash of its samples.
fn run(manifest: &Manifest, clocks: u64, chunk: u64) -> (f64, u64) {
    let mut chip = SN76489::new(variant(&manifest.variant).expect("checked by the loader"));
    let mut samples = Vec::new();
    let started = Instant::now();
    for event in &manifest.events {
        if event.t >= clocks {
            break;
        }
        run_to(&mut chip, event.t, chunk);
        match event.kind {
            Kind::Write => chip.write(event.data),
            Kind::Stereo => chip.write_stereo(event.data),
            Kind::Test => chip.write_test(event.data),
        }
    }
    run_to(&mut chip, clocks, chunk);
    samples.extend(chip.take_samples());
    let elapsed = started.elapsed().as_secs_f64();
    let mut hasher = DefaultHasher::new();
    for (left, right) in black_box(&samples) {
        left.to_bits().hash(&mut hasher);
        right.to_bits().hash(&mut hasher);
    }
    (elapsed, hasher.finish())
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut take = |name: &str| -> Option<String> {
        let i = args.iter().position(|a| a == name)?;
        let value = args.get(i + 1).cloned();
        args.drain(i..(i + 2).min(args.len()));
        value
    };
    let seconds: f64 = take("--seconds")
        .and_then(|v| v.parse().ok())
        .unwrap_or(10.0);
    let repeat: usize = take("--repeat").and_then(|v| v.parse().ok()).unwrap_or(5);
    let chunk: u64 = take("--chunk")
        .and_then(|v| v.parse().ok())
        .unwrap_or(228)
        .max(1);
    if args.is_empty() {
        eprintln!("usage: sn76489-bench [--seconds S] [--repeat N] [--chunk C] <manifest.json>...");
        return ExitCode::from(2);
    }
    let mut total = 0.0;
    let mut check = DefaultHasher::new();
    for path in &args {
        let manifest = match load_manifest(path) {
            Ok(manifest) => manifest,
            Err(error) => {
                eprintln!("error: {path}: {error}");
                return ExitCode::from(2);
            }
        };
        let clocks = ((REAL_TIME * seconds) as u64).min(manifest.clocks);
        let mut best = f64::INFINITY;
        let mut hash = 0;
        for _ in 0..repeat {
            let (elapsed, h) = run(&manifest, clocks, chunk);
            best = best.min(elapsed);
            hash = h;
        }
        hash.hash(&mut check);
        total += best;
        let rate = clocks as f64 / best;
        println!(
            "  {:54} {:>10} clocks  {:8.1}x real time  ({best:.4} s)  {hash:016x}",
            manifest.name,
            clocks,
            rate / REAL_TIME
        );
    }
    println!("check {:016x}", check.finish());
    println!("total {total:.4} s");
    ExitCode::SUCCESS
}
