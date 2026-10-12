# Build record

2026-10-11, one session, the day sn76489-python was extracted and both boards adopted
it, and the day after ym2612-rust.

## The transcription

`chip.py` is about 430 lines: a per-clock loop, two internal phases, a register
update and a mixer. It became `src/chip.rs` in one pass, function for function. The
decisions:

- **Integers.** Every register is a `u32` and the clock a `u64`. Nothing in the engine
  is negative, so no floor-division care was needed, unlike ym2612-rust's /IC
  arithmetic.
- **The curves.** `levels` is a tuple of floats in the reference. Here it is a
  `Levels` value: the ideal curve is computed by the reference's formula when asked
  for (`10f64.powf(-0.1 * step)` for Python's `10 ** (-0.1 * step)`, both the C
  library's `pow`), since a `const` cannot call `powf`. A host's own curve is
  `Levels::Table`.
- **The mixer** sums in the reference's order (`a + b + c + d` for mono, `0.0 +` each
  routed channel for stereo), so the floats are the same bits, and rung 1 checks that
  they are.

The first build reproduced all ten example traces and every rung's manifests on the
first run. No divergence between the two cores was found.

## Rung 1, widened

The reference's rung 1 is "the `levels` tables equal contract.md's". The trace sees
only attenuations, so the curves, and the Game Gear's stereo routing inside
`take_samples()`, are data a port copies unchecked; the reference's own mutation run
records that its examples miss a wrong curve and swapped sides. Here rung 1 prints
every variant's table from both cores (floats by Python's `repr`, which Rust's `{:?}`
matches: the shortest string that reads back as the same double) and compares
`take_samples()` over all ten examples to the bit. The seeded faults show it is needed:
the three faults the examples miss are exactly these, and rung 1 alone catches them.

## The ladder

| Rung | Result | Wall |
| --- | --- | --- |
| 1 | 4 variants, 10 examples' samples | seconds |
| 2 | 10 / 10 | seconds |
| 3 | 24 / 24, 2,990,093 clocks each variant | 3 s |
| 4 | 13 / 13, 630,272,479 clocks | 35 s |
| 5 | 128 / 128, 132,223,397 clocks | 16 s |

The reference wrote its traces on PyPy 7.3.20, 16 manifests at a time. Rung 5's first
run reported 126 of 128: two manifests put a test-bit release past the run's end,
which both cores refuse. The generator now extends the run to cover its last event.

## The seeded faults

14 faults, each one change in a copy of the repository (`scripts/seeded_faults.py`).
The examples catch 11; rung 1 catches the other 3 (the stereo sides and the two
curves), which rungs 3 and 5 cannot see by design.

## Tooling notes

- `cargo clippy` and the wasm build here need the rustup stable toolchain through
  `RUSTC`, as in ym2612-rust:
  `CARGO_TARGET_DIR=target/clippy RUSTC=$(rustup which --toolchain stable rustc) rustup run stable cargo clippy --release --all-targets -- -D warnings`
  and the same with `CARGO_TARGET_DIR=target/wasm ... cargo build --release --lib --target wasm32-unknown-unknown`.
  The default toolchain is an old nightly without the wasm target.
- `scripts/fetch_sn76489python.sh` clones the reference over HTTPS by
  default. `SN76489_PYTHON_SOURCE=/data/emu/sn76489-python` clones the local one.

## The fork (sn76489-rust-fast)

2026-10-12, started after megadrive-rust-fast's profile put the PSG at 7% of a Mega
Drive frame (14% in Overdrive 2).

- **Fork point `66de73b`.** sn76489-rust `c5fc09e`, the package renamed, the crate
  name kept. The reference was fetched from the local sn76489-python clone at the
  same pin and installed on PyPy 7.3.20. The ladder was clean before any change.
  `sn76489-bench` was rewritten to take several manifests, each in its own variant,
  run in scanline-sized pieces, with a check over the samples.
- **The trace runs one clock at a time.** `conformance::replay` calls `run(1)` for
  every clock, so no rung of the reference's ladder would ever reach a path that only
  a long `run()` takes. Rung 6 compares long runs with single clocks over every
  manifest, including the full state at every boundary. It is the fork's gate for
  everything it changes. A deliberate off-by-one in the first fast path failed 162 of
  175 manifests.
- **Steady state is rigid.** From power-on, with /IC released, the prescaler's
  four-clock groups start at `clock & 15` = 2, 6, 10, 14. At those points `chan_sel`
  is $88, $11, $22, $44 and `rot` 0, 1, 2, 3, on every variant. Only /IC moves the
  prescaler (writes do not), so this holds for the chip's life. Both fast paths
  check their alignment on entry and otherwise fall back to the slower path.
- **Equivalent mutants.** Because the alignment is fixed, several plausible faults
  change nothing. A sample after a 12 group's phase 2 is never taken. Taking the
  sample before or after the 14 group's phase 2 is the same, since the noise never
  shifts there. A sample a group early or late within 8-14 is also the same, since
  the attenuations change only in the 2 and 6 groups. Each was run, found unseen by
  every rung, and reasoned out before being noted rather than kept. The faults that
  stand put the sample a group early, from 14 to 10, and pair tone 1 with the wrong
  counter.
- **Quiet stretches.** Passing over quiet stretches whole was the first idea, and it
  lost (x0.69 on Gunstar). The power-on tone period is 0, which expires every round,
  so almost no stretch is quiet in a real game.
- **In the board.** In a megadrive-rust-fast worktree (`external/psgswap`, the
  dependency pointed at this checkout), md-bench gained x1.075 (Overdrive 2 x1.148).
  md-trace was 37 of 37 identical to upstream megadrive-rust in the lockstep gate.
- **Tooling.** `cargo clippy` needs the stable toolchain through `RUSTC`, as above.
  The plain `cargo` here is 1.93.1, and its clippy is an old nightly that refuses the
  crate's `rust-version`.
