//! The conformance kit's Rust half: read a manifest, run it, write the trace.
//!
//! A transcription of sn76489-python's `src/sn76489_python/conformance.py`
//! (`trace_manifest`, `manifest_from_dict`); the formats are its
//! `docs/conformance.md`. The diff stays the reference's: a trace written here is
//! judged by `python -m sn76489_python.conformance diff manifest.json -`.

use std::io::{self, Write};

use serde_json::{Map, Value};

use crate::chip::{
    PSGVariant, CLOCKS_PER_SAMPLE, GAME_GEAR_PSG, SEGA_VDP_PSG, SN76489, SN76489AN, YM7101_PSG,
};

pub const MANIFEST_SCHEMA_VERSION: i64 = 1;
/// The first line of every trace, so a trace of another version is refused, not misread.
pub const TRACE_HEADER: &str = "# sn76489-trace 1";

/// The manifest's variant names.
pub fn variant(name: &str) -> Option<PSGVariant> {
    match name {
        "sn76489an" => Some(SN76489AN),
        "sega-vdp" => Some(SEGA_VDP_PSG),
        "game-gear" => Some(GAME_GEAR_PSG),
        "ym7101" => Some(YM7101_PSG),
        _ => None,
    }
}

/// One thing the host does to the chip, before input clock `t`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `write(data)`, the data port.
    Write,
    /// `write_stereo(data)`, the Game Gear's port $06.
    Stereo,
    /// `write_test(data)`, the three test bits.
    Test,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub t: u64,
    pub kind: Kind,
    pub data: u8,
}

/// A run: the variant, the events in clock order, and how many clocks to run.
#[derive(Clone, Debug)]
pub struct Manifest {
    pub name: String,
    pub variant: String,
    pub events: Vec<Event>,
    pub clocks: u64,
}

fn int(value: &Value, what: &str) -> Result<i64, String> {
    value
        .as_i64()
        .ok_or_else(|| format!("{what} is not an integer"))
}

fn event_from(index: usize, value: &Value) -> Result<Event, String> {
    let item: &Map<String, Value> = value
        .as_object()
        .ok_or_else(|| format!("event {index} is not an object"))?;
    let mut extra: Vec<&String> = item
        .keys()
        .filter(|k| !["t", "kind", "data"].contains(&k.as_str()))
        .collect();
    if !extra.is_empty() {
        extra.sort();
        return Err(format!("event {index}: unknown keys {extra:?}"));
    }
    let t = int(
        item.get("t")
            .ok_or_else(|| format!("event {index} has no t"))?,
        &format!("event {index} t"),
    )?;
    let data = item
        .get("data")
        .map_or(Ok(0), |v| int(v, &format!("event {index} data")))?;
    let kind = match item.get("kind").and_then(Value::as_str) {
        Some("write") => Kind::Write,
        Some("stereo") => Kind::Stereo,
        Some("test") => Kind::Test,
        other => return Err(format!("unknown event kind {other:?}")),
    };
    if t < 0 || !(0..=0xFF).contains(&data) {
        return Err(format!("event {index} out of range"));
    }
    Ok(Event {
        t: t as u64,
        kind,
        data: data as u8,
    })
}

/// A manifest from its JSON form (sn76489-python `docs/conformance.md`), checked as
/// strictly as the reference's `manifest_from_dict`.
pub fn manifest_from_value(value: &Value) -> Result<Manifest, String> {
    let object = value.as_object().ok_or("a manifest is a JSON object")?;
    let allowed = ["version", "name", "variant", "events", "clocks"];
    let mut unknown: Vec<&String> = object
        .keys()
        .filter(|k| !allowed.contains(&k.as_str()))
        .collect();
    if !unknown.is_empty() {
        unknown.sort();
        return Err(format!("unknown manifest keys: {unknown:?}"));
    }
    if object.get("version").and_then(Value::as_i64) != Some(MANIFEST_SCHEMA_VERSION) {
        return Err(format!(
            "unsupported manifest version {:?}",
            object.get("version")
        ));
    }
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .ok_or("manifest has no name")?;
    let variant_name = object
        .get("variant")
        .and_then(Value::as_str)
        .ok_or("manifest has no variant")?;
    if variant(variant_name).is_none() {
        return Err(format!(
            "unknown variant {variant_name:?} (sn76489an, sega-vdp, game-gear, ym7101)"
        ));
    }
    let clocks = int(
        object.get("clocks").ok_or("manifest has no clocks")?,
        "clocks",
    )?;
    if clocks < 0 {
        return Err("clocks is negative".into());
    }
    let mut events = Vec::new();
    if let Some(list) = object.get("events") {
        let list = list.as_array().ok_or("events is not a list")?;
        for (index, item) in list.iter().enumerate() {
            events.push(event_from(index, item)?);
        }
    }
    if events.windows(2).any(|w| w[1].t < w[0].t) {
        return Err("events must be in clock order".into());
    }
    if events.last().is_some_and(|e| e.t > clocks as u64) {
        return Err("an event falls after the end of the run".into());
    }
    Ok(Manifest {
        name: name.to_string(),
        variant: variant_name.to_string(),
        events,
        clocks: clocks as u64,
    })
}

pub fn load_manifest(path: &str) -> Result<Manifest, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let value: Value = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    manifest_from_value(&value)
}

/// Run `manifest`'s events on a fresh chip, calling `each` after every input clock.
/// The reference's `trace_manifest` loop, shared with the samples check.
pub fn replay(
    manifest: &Manifest,
    mut each: impl FnMut(&mut SN76489) -> io::Result<()>,
) -> io::Result<SN76489> {
    let mut chip = SN76489::new(variant(&manifest.variant).expect("checked when loaded"));
    let mut advance = |chip: &mut SN76489, t: u64| -> io::Result<()> {
        while chip.clock < t {
            chip.run(1);
            each(chip)?;
        }
        Ok(())
    };
    for event in &manifest.events {
        advance(&mut chip, event.t)?;
        match event.kind {
            Kind::Write => chip.write(event.data),
            Kind::Stereo => chip.write_stereo(event.data),
            Kind::Test => chip.write_test(event.data),
        }
    }
    advance(&mut chip, manifest.clocks)?;
    Ok(chip)
}

/// Run `manifest` and write its trace: `A clock a1 a2 a3 an stereo` after input
/// clock `clock` whenever the four attenuations being driven or the stereo enables
/// differ from the previous line's, and once before the first clock (the reference's
/// `trace_manifest`).
pub fn trace_manifest(manifest: &Manifest, out: &mut impl Write) -> io::Result<()> {
    let first = SN76489::new(variant(&manifest.variant).expect("checked when loaded"));
    writeln!(out, "{TRACE_HEADER}")?;
    writeln!(out, "# {} {}", manifest.name, manifest.variant)?;
    let [a, b, c, d] = first.attenuations();
    let mut last = ([a, b, c, d], first.stereo);
    writeln!(out, "A 0 {a} {b} {c} {d} {}", first.stereo)?;
    let chip = replay(manifest, |chip| {
        let now = (chip.attenuations(), chip.stereo);
        if now != last {
            last = now;
            let [a, b, c, d] = now.0;
            writeln!(out, "A {} {a} {b} {c} {d} {}", chip.clock, now.1)?;
        }
        Ok(())
    })?;
    let writes = manifest
        .events
        .iter()
        .filter(|e| e.kind == Kind::Write)
        .count();
    writeln!(
        out,
        "# end {} clocks {} samples {writes} writes",
        chip.clock,
        chip.clock / CLOCKS_PER_SAMPLE
    )
}

/// Rung 6: run `manifest` twice, once a clock at a time as the trace does and once in
/// pieces of seeded random length (one clock to thousands, and always to each event,
/// as a board runs the chip), and check that the second leaves the first's state at
/// every boundary and the same samples, to the bit. The ladder's other rungs judge
/// the first against the reference, so this carries their result over to long runs.
/// Returns the boundaries checked.
pub fn check_chunked(manifest: &Manifest, seed: u64) -> Result<u64, String> {
    let variant = variant(&manifest.variant).expect("checked when loaded");
    let mut single = SN76489::new(variant);
    let mut chunked = SN76489::new(variant);
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut length = move || {
        // xorshift64*: pieces short, middling and long in equal measure
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let r = state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32;
        1 + match r % 3 {
            0 => r / 3 % 20,
            1 => r / 3 % 400,
            _ => r / 3 % 20_000,
        }
    };
    let mut boundaries = 0;
    let mut checked = 0;
    let mut advance = |single: &mut SN76489, chunked: &mut SN76489, checked: &mut usize, t: u64| {
        while chunked.clock < t {
            chunked.run(length().min(t - chunked.clock));
            while single.clock < chunked.clock {
                single.run(1);
            }
            if !chunked.same_state(single) {
                return Err(format!(
                    "{}: state differs at clock {}",
                    manifest.name, chunked.clock
                ));
            }
            if chunked.levels[*checked..] != single.levels[*checked..] {
                return Err(format!(
                    "{}: samples differ by clock {}",
                    manifest.name, chunked.clock
                ));
            }
            *checked = chunked.levels.len();
            boundaries += 1;
        }
        Ok(())
    };
    for event in &manifest.events {
        advance(&mut single, &mut chunked, &mut checked, event.t)?;
        for chip in [&mut single, &mut chunked] {
            match event.kind {
                Kind::Write => chip.write(event.data),
                Kind::Stereo => chip.write_stereo(event.data),
                Kind::Test => chip.write_test(event.data),
            }
        }
        checked = checked.min(chunked.levels.len());
    }
    advance(&mut single, &mut chunked, &mut checked, manifest.clocks)?;
    let bits = |samples: Vec<(f64, f64)>| -> Vec<(u64, u64)> {
        samples
            .iter()
            .map(|(l, r)| (l.to_bits(), r.to_bits()))
            .collect()
    };
    if bits(chunked.take_samples()) != bits(single.take_samples()) {
        return Err(format!("{}: take_samples() differs", manifest.name));
    }
    Ok(boundaries)
}
