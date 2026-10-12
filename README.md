# sn76489-rust-fast

A fast fork of [sn76489-rust](https://github.com/alewman/sn76489-rust), the Rust
transcription of [sn76489-python](https://github.com/alewman/sn76489-python), the
SN76489 PSG in its four Sega variants. It stays provably equivalent to the reference
at every input clock: the same ladder, every rung, on every commit that touches
`src/`, plus a rung of its own for the long `run()` calls a board makes. Each
optimization is one commit with its measurement. A change that does not pay is
reverted and recorded as not kept.

The crate keeps the name `sn76489_rust` and sn76489-rust's API, so a board written
for sn76489-rust takes this one with
`sn76489-rust = { package = "sn76489-rust-fast", ... }` and no other change.

## The gate

`scripts/ladder.sh`, about a minute: `cargo test`, then rungs 1-5 against the
pinned reference (below, "Certification"), then rung 6.

The reference's trace runs the chip **one clock at a time**, so rungs 1-5 judge only
`run(1)`. The fork's fast paths run only inside longer calls. **Rung 6**
(`scripts/rung6.sh`, `conformance::check_chunked`) runs every manifest of rungs 2-5
twice: clock by clock, and in seeded random pieces of one clock to 20,000 and up to
each event, as a board runs the chip. It requires the same state at every boundary
(`same_state`, which destructures the chip, so a new field that is not compared is a
compile error) and the same samples, to the bit. Rungs 1-5 carry over to long runs
through it: 175 of 175 manifests, 544,266 boundaries. `cargo test` runs it on the
ten vendored examples with three seeds, so CI covers the fast paths too.

`scripts/seeded_faults.py` breaks the core one rule at a time. All 18 faults are
caught: sn76489-rust's 14, and 4 in the fork's paths. Three more fork faults turned
out to be equivalent mutants; the script notes why rather than counting them.

## Speed

`scripts/bench.sh` drives `run()` as a board does: in pieces of at most a Mega Drive
scanline (228 input clocks) and up to each write, the first ten emulated seconds of
four workloads. These are the reference's random walk on the YM7101 and, from the
ROM-derived board streams (local only), Gunstar Heroes (YM7101), Sonic the Hedgehog
(Master System) and Columns (Game Gear, stereo). It prints a check over every
sample. `scripts/ab.sh` builds a base commit and the working tree under three code
layouts and runs them alternately on one core of the shared i9-13900K, best of 7. It
refuses a result whose checks differ. The ratio is the result.

| Commit | Change | Ratio (min over layouts) |
| --- | --- | ---: |
| `66de73b` | the fork point: sn76489-rust's transcription, with the harness | -- |
| `5124c76` | a settled chip (/IC out of both shift registers, no write in flight) runs four clocks at a time: `phase1::<true>` and `phase2::<true>`, the same code with the /IC and write paths compiled out | x1.776 |
| `6d3bab0` | whole 16-clock rounds with `chan_sel` and its counter known, since in steady state they are the same at the same point of every round | x1.063 |
| | **at `6d3bab0` against `66de73b`** | **x1.910** |

At `6d3bab0`, best of 10 on pinned core 31, 2026-10-12: the random walk 151x real
time (68x at the fork point), the three cartridges 109-110x (57x).

**Measured and not kept:**
- Passing over quiet stretches whole: x0.69 on Gunstar. A tone at period 0, the
  power-on value, expires every round, so a stretch is almost never quiet, and the
  check cost more than it saved.
- Fat LTO with one codegen unit: x0.982.
- `#[inline(always)]` on `shift` and `attenuations`: x0.991.
- Reserving the samples' room before the rounds: x1.004.

**In a Mega Drive.** megadrive-rust-fast (at `e18bdd1`) was built against this crate
(`6d3bab0`) in a separate worktree and timed against its own build, 600 frames,
alternately on one core, best of 5:

| Workload | Board with sn76489-rust | With this fork | Ratio |
| --- | ---: | ---: | ---: |
| Sonic the Hedgehog | 1.615 s | 1.533 s | x1.053 |
| Streets of Rage 2 | 1.340 s | 1.261 s | x1.063 |
| Gunstar Heroes | 1.658 s | 1.573 s | x1.054 |
| Overdrive 2 | 1.299 s | 1.132 s | x1.148 |
| **Total** | 5.912 s | 5.499 s | **x1.075** |

Both boards made the same sound and picture (the checksums match). The board's own
gate, all 37 lockstep cases at full length against upstream megadrive-rust, is 37 of
37 identical with this PSG. What is left of the PSG is about 4% of a board frame, so
further gains here move the board by a percent or two.

The library builds for `wasm32-unknown-unknown` (stable toolchain).

## sn76489-rust, which this forks

A Rust **SN76489** PSG core transcribed from
[sn76489-python](https://github.com/alewman/sn76489-python) and proven equivalent to
it at every input clock: the four attenuations being driven and the Game Gear's
stereo register. sn76489-python is in turn equal to **Nuked-PSG**, a transcription of
the YM7101's die, at every input clock of the same 19 streams (633,262,572 clocks,
0 different), so the two results compose.

Four variants, as in the reference: the discrete TI **SN76489AN** (SG-1000, SC-3000),
the **Sega VDP** PSG (Master System), the **Game Gear**'s with stereo, and the
**YM7101**'s (Mega Drive).

`src/chip.rs` is `chip.py` function for function, in the same order, with the same
names and the same citations of the die model. `src/conformance.rs` is the
reference's manifest reader and trace writer. Nothing is redesigned: where Rust
forced a difference, the module header says what it is. Speed is the job of a
`-fast` fork, if one is ever needed (see "Speed").

## Certification

Pinned oracles:

| What | Version |
| --- | --- |
| sn76489-python (reference core and conformance kit) | `9ed3b85a333191caa662edfcaf7a7b3167b95476` |
| Manifest and trace formats | manifest version 1, `# sn76489-trace 1` |

The ladder is the reference's own (its `docs/conformance.md`, "Certification ladder
for a port"), with rung 1 widened and a rung 5 added. Every rung was reproduced with
the script named (`scripts/`) on 2026-10-11: Linux x86_64, rustc 1.93.1, the
reference on PyPy 7.3.20.

| Rung | What | Result | Script |
| --- | --- | --- | --- |
| 1 | The data the trace does not see. Every variant's table (noise register width and taps, stereo, the low-period rule, the sixteen levels) printed by both cores, floats as Python's `repr`; and `take_samples()` over every example, the levels curves and the stereo routing summed in the reference's order | 4 of 4 variants identical; 10 of 10 examples' samples identical to the bit | `rung1.sh` |
| 2 | Every example manifest in the reference's `examples/conformance/` | 10 of 10 identical | `rung2.sh` |
| 3 | The six synthetic streams of the reference's `scripts/run_psg_compare.py` (two random walks, a dense one with writes 1-4 clocks apart, every tone period, every noise setting, every attenuation), in all four variants | 24 of 24 identical, 2,990,093 clocks each variant | `rung3.sh` |
| 4 | The writes the boards' PSGs received running cartridges, each in its machine's variant: five Mega Drive (Sonic the Hedgehog, Streets of Rage 2, Gunstar Heroes, Castlevania Bloodlines, Phantasy Star IV), five Master System (Sonic the Hedgehog, Alex Kidd in Miracle World, Phantasy Star, After Burner, Micro Machines), two Game Gear (Columns, Shinobi II), one SG-1000 (Flicky) | 13 of 13 identical, 630,272,479 clocks (35 s) | `rung4.sh` |
| 5 | 128 seeded random manifests, 32 in each variant: voices programmed to sound, then 47,353 writes from one clock to thousands apart, noise restarts, 2,003 Game Gear stereo writes (ignored by the other variants), 2,764 test-bit events | 128 of 128 identical, 132,223,397 clocks | `rung5.sh` |

Rung 3's YM7101 manifests and rung 4 together are every clock of the reference's
Nuked-PSG record (633,262,572), as a trace identical to the reference's here is the
run the reference compared with the die model. Each stream's manifest writes when the
chip has run exactly the clock the stream names, which is where the reference's
`compare` writes.

**A gate that cannot fail proves nothing.** `scripts/seeded_faults.py` breaks this
core in fourteen places, one at a time, in a copy of the repository:

- the write a clock late;
- the counter's compare;
- the noise periods;
- noise rate 3 following tone 2;
- the LFSR's output bit;
- the zero-detect's width;
- a volume data byte ignored;
- a noise write that does not restart the register;
- period 2 held high;
- the test bit ignored;
- power-on without the /IC sequence;
- the stereo sides swapped;
- one entry of the YM7101 curve;
- a 1.5 dB ideal curve.

The ten examples catch eleven. The other three are data outside the trace, as the
reference's own `scripts/mutation.py` records. Rung 1 catches all three:

| Fault | Rung 1 finds | Rungs 3 and 5 |
| --- | --- | --- |
| stereo sides swapped | `stereo-game-gear`'s samples | identical, as they must be |
| one YM7101 curve entry | the tables and the YM7101 examples' samples | identical |
| a 1.5 dB ideal curve | the tables and the other variants' samples | identical |

That is why rung 1 compares `take_samples()` as well as the tables.

CI (`.github/workflows/ci.yml`) runs `cargo fmt --check`, `cargo clippy -D
warnings` and `cargo test` on main and pull requests. `cargo test` replays the ten
example manifests against copies of the reference traces, without Python. Rungs 1-5
need the reference checked out (`scripts/fetch_sn76489python.sh`), and rung 4 the
ROM-derived board streams, so they run locally.

### Reproducing

```text
scripts/fetch_sn76489python.sh    # the reference at the pin into external/, with a venv
scripts/rung1.sh
scripts/rung2.sh
scripts/rung3.sh                  # writes the manifests into external/manifests/synthetic
scripts/rung4.sh [streams dir]    # needs the reference's exported board streams, below
scripts/rung5.sh [count] [seed]   # 128 from seed 1 by default
python3 scripts/seeded_faults.py  # after rungs 3 and 5, which write the manifests it reuses
scripts/bench.sh
```

`PYTHON=/path/to/pypy3.11 scripts/fetch_sn76489python.sh` puts the reference on
PyPy, which writes its traces several times faster; the records here used it. Each of rungs 2-5 writes this core's trace and
pipes it to the reference's own
`python -m sn76489_python.conformance diff manifest.json -`, 16 at a time
(`scripts/diff_all.sh`). Rung 4's streams come from ROMs, so neither repository
commits them, nor the manifests made from them (`external/`, ignored). The reference
exports them with `scripts/export_board_streams.py`, run with each board's
interpreter.

## Using the core

```rust
use sn76489_rust::{SN76489, YM7101_PSG};

let mut chip = SN76489::new(YM7101_PSG); // the Mega Drive's
for byte in [0x8E, 0x0F, 0x90] {         // tone 1, period 254, full volume
    chip.write(byte);
    chip.run(32);                        // the byte lands on the next clock
}
chip.run(3_579_545 / 10);                // a tenth of a second of input clocks
let samples = chip.take_samples();       // (left, right), 0.0-4.0, at 223,722 Hz
```

The contract is the reference's `docs/contract.md`, in Rust form:

- `run(clocks)` advances that many **input clocks** (the Z80's clock on a Master
  System or Mega Drive); `clock` counts them from power-on. The host brings the chip
  to the moment of each write with `run()` first.
- `write(value)` is the data port, `write_stereo(value)` the Game Gear's port $06
  (ignored by the other variants), and `write_test(bits)` the three test bits (on the
  Mega Drive, the VDP debug register's bits 11-9). There is no read.
- `attenuations()` is the digital state now. `take_attenuations()` and
  `take_samples()` drain one queue of finished samples, one per 16 clocks, as
  attenuations or as levels by the variant's curve.
- `SN76489::new()` and `reset()` run the die's power-on (/IC for 16 clocks).

A variant is a value (`PSGVariant`), so a host can build its own, with its own curve
as `Levels::Table`. The engine's state is public so a debugger can look at it. It is
not a contract.

## Binaries

- `sn76489-trace <manifest> [--out FILE]`: run a conformance manifest and write its
  trace (rungs 2-5).
- `sn76489-tables`: every variant's data, as rung 1 compares it.
- `sn76489-samples <manifest>`: `take_samples()` after a manifest, as rung 1
  compares it.
- `sn76489-bench <manifest>...`: each manifest in its own variant through `run()`,
  as a board drives it, best of `--repeat`, with a check over the samples
  (`scripts/bench.sh`).
- `sn76489-chunks <manifest>...`: rung 6, long runs against single clocks.

## sn76489-rust's speed

`scripts/bench.sh` runs the reference's benchmark: the first two seconds of the seed-1
random walk (40,000 latches) through `run()`, as a board drives the chip, best of 20.
Measured on one pinned core of an i9-13900K that other jobs were sharing (load average
25 on 32 threads), 2026-10-11:

| | YM7101 | Game Gear |
| --- | ---: | ---: |
| sn76489-rust | 420 M clocks/s, **117× real time** | 428 M clocks/s, 120× |
| sn76489-python, PyPy 7.3.20 (its record) | 30.0 M clocks/s, 8.4× | 32.5 M clocks/s, 9.1× |
| sn76489-python, CPython 3.14.4 (its record) | 3.05 M clocks/s, 0.85× | 3.30 M clocks/s, 0.92× |

Real time is 3,579,545 input clocks a second (NTSC), so this core is 14 times the
reference on PyPy. In a whole Mega Drive the PSG is under 1% of one core, against
about 6% for ym2612-rust-fast's FM chip, so a `-fast` fork is not needed.
(That was sn76489-rust's estimate. megadrive-rust-fast's profile later put the PSG at
7% of a board frame, 14% in Overdrive 2, and this fork exists because of it. Its
own benchmark, above, replaced this one.)
The library also builds for `wasm32-unknown-unknown` (stable toolchain).

## Where the brief is

[docs/handoff-brief.md](docs/handoff-brief.md) is the request this repository was
built from and the plan read from it, and [docs/build-record.md](docs/build-record.md)
is what happened.

## License

MIT, like the reference. Nuked-PSG (GPL-2.0) is cited through the reference and never
read into this repository.
