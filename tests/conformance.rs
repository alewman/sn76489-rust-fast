//! The reference's ten example manifests, replayed against copies of its reference
//! traces (`tests/data/conformance/`, from sn76489-python's `examples/conformance/`
//! at the pinned commit) without Python. `scripts/rung2.sh` makes the same check
//! through the reference's own diff.

use std::fs;
use std::path::Path;

use serde_json::json;
use sn76489_rust::conformance::{
    check_chunked, load_manifest, manifest_from_value, trace_manifest,
};
use sn76489_rust::{ideal_levels, GAME_GEAR_PSG, SEGA_VDP_PSG, SN76489, YM7101_PSG};

#[test]
fn every_example_trace_is_the_reference_trace() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/conformance");
    let mut checked = 0;
    let mut paths: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    for path in paths {
        let manifest = load_manifest(path.to_str().unwrap()).unwrap();
        let mut trace = Vec::new();
        trace_manifest(&manifest, &mut trace).unwrap();
        let reference = fs::read_to_string(path.with_extension("trace")).unwrap();
        let ours = String::from_utf8(trace).unwrap();
        if let Some((line, (a, b))) = reference
            .lines()
            .zip(ours.lines())
            .enumerate()
            .find(|(_, (a, b))| a != b)
        {
            panic!(
                "{}: line {}: reference {a:?}, ours {b:?}",
                manifest.name,
                line + 1
            );
        }
        assert_eq!(
            reference.lines().count(),
            ours.lines().count(),
            "{}",
            manifest.name
        );
        checked += 1;
    }
    assert_eq!(checked, 10);
}

#[test]
fn bad_manifests_are_refused_as_the_reference_refuses_them() {
    let good = json!({"version": 1, "name": "x", "variant": "ym7101", "clocks": 100, "events": []});
    assert!(manifest_from_value(&good).is_ok());
    let cases = [
        json!([]),
        json!({"version": 2, "name": "x", "variant": "ym7101", "clocks": 100}),
        json!({"version": 1, "name": "x", "variant": "sn76496", "clocks": 100}),
        json!({"version": 1, "name": "x", "variant": "ym7101", "clocks": 100, "extra": 1}),
        json!({"version": 1, "name": "x", "variant": "ym7101", "clocks": 100,
               "events": [{"t": 5, "kind": "read"}]}),
        json!({"version": 1, "name": "x", "variant": "ym7101", "clocks": 100,
               "events": [{"t": 5, "kind": "write", "data": 256}]}),
        json!({"version": 1, "name": "x", "variant": "ym7101", "clocks": 100,
               "events": [{"t": 5, "kind": "write", "port": 1}]}),
        json!({"version": 1, "name": "x", "variant": "ym7101", "clocks": 100,
               "events": [{"t": 50, "kind": "write"}, {"t": 5, "kind": "write"}]}),
        json!({"version": 1, "name": "x", "variant": "ym7101", "clocks": 100,
               "events": [{"t": 500, "kind": "write"}]}),
    ];
    for case in cases {
        assert!(manifest_from_value(&case).is_err(), "{case}");
    }
}

#[test]
fn the_reference_minimal_host_sounds() {
    // Tone 1 at full volume, period 254 (about 440 Hz from 3,579,545 Hz), a tenth of
    // a second: one sample per 16 clocks, the channel toggling every 254 samples.
    let mut chip = SN76489::new(SEGA_VDP_PSG);
    for byte in [0x80 | 0x0E, 0x0F, 0x90] {
        chip.write(byte);
        chip.run(32);
    }
    chip.run(357_954);
    let samples = chip.take_samples();
    assert_eq!(samples.len(), (96 + 357_954) / 16);
    let loud = samples.iter().filter(|&&(left, _)| left > 0.9).count();
    let quiet = samples.iter().filter(|&&(left, _)| left < 0.1).count();
    assert!(loud > 10_000 && quiet > 10_000, "{loud} {quiet}");
}

#[test]
fn the_curves_are_the_contracts() {
    let ideal = ideal_levels();
    assert_eq!(ideal[0], 1.0);
    assert_eq!(ideal[15], 0.0);
    assert!((ideal[1] - 10f64.powf(-0.1)).abs() == 0.0);
    assert_eq!(YM7101_PSG.levels.table()[4], 0.382);
}

#[test]
fn stereo_is_the_game_gears_alone() {
    for (variant, routed) in [(GAME_GEAR_PSG, true), (SEGA_VDP_PSG, false)] {
        let mut chip = SN76489::new(variant);
        chip.write(0x90); // tone 1 at full volume, period 0: held high where it holds
        chip.run(16);
        chip.write_stereo(0x10); // tone 1 left only
        chip.run(64);
        let (left, right) = *chip.take_samples().last().unwrap();
        assert!(left > 0.9, "{}", variant.name);
        assert_eq!(right == 0.0, routed, "{}", variant.name);
    }
}

#[test]
fn long_runs_leave_what_single_clocks_leave() {
    // Rung 6 on the examples, three seeds each: the fork's fast paths only run in
    // run() calls longer than one clock, which the trace never makes.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/conformance");
    let mut checked = 0;
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let manifest = load_manifest(path.to_str().unwrap()).unwrap();
        for seed in 1..=3 {
            if let Err(error) = check_chunked(&manifest, seed) {
                panic!("seed {seed}: {error}");
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 10);
}
