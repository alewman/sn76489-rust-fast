//! `sn76489-chunks [--seed N] <manifest.json>...`: rung 6. Each manifest is run a
//! clock at a time and in seeded random pieces, and the two must agree at every
//! boundary (`conformance::check_chunked`). Prints one line per manifest and a total;
//! exits 1 if any differ.

use std::process::ExitCode;

use sn76489_rust::conformance::{check_chunked, load_manifest};

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut seed = 1;
    if let Some(i) = args.iter().position(|a| a == "--seed") {
        seed = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(1);
        args.drain(i..(i + 2).min(args.len()));
    }
    if args.is_empty() {
        eprintln!("usage: sn76489-chunks [--seed N] <manifest.json>...");
        return ExitCode::from(2);
    }
    let mut same = 0;
    for path in &args {
        let manifest = match load_manifest(path) {
            Ok(manifest) => manifest,
            Err(error) => {
                eprintln!("error: {error}");
                return ExitCode::from(2);
            }
        };
        match check_chunked(&manifest, seed) {
            Ok(boundaries) => {
                same += 1;
                println!("{}: {boundaries} boundaries agree", manifest.name);
            }
            Err(error) => println!("{error}"),
        }
    }
    println!("TOTAL: {same} of {} manifests agree", args.len());
    if same == args.len() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
