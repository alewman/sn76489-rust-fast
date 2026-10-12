#!/usr/bin/env python
"""A gate that cannot fail proves nothing: break this core one rule at a time and
check that the ladder catches every break.

    python scripts/seeded_faults.py [--jobs 8]

Each fault is one textual change to ``src/chip.rs``, applied to a copy of the
repository under ``external/faults/<name>/`` (the working tree is never touched) and
judged first by ``cargo test --release --test conformance``: the ten examples against
the reference's traces, and run in long pieces against one clock at a time. A fault the examples miss is then run through rung 1 (the
tables and ``take_samples()``), rung 3, rung 5 and rung 6 (``external/manifests/synthetic``
and ``random``, written by ``scripts/rung3.sh`` and ``scripts/rung5.sh``; the
reference must be fetched), and what each finds is reported. sn76489-python's
``scripts/mutation.py`` records the examples' own gap: the curves and the stereo
sides are outside the trace.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

#: name -> (text in src/chip.rs, replacement). Each text must occur exactly once.
FAULTS = {
    "write-never-early": ("        if !SETTLED && self.write_flag_l {\n            self.update_registers();\n        }\n        self.rot", "        self.rot"),
    "counter-compares-greater": ("if counter >= period {", "if counter > period {"),
    "noise-periods-doubled": ("[0x10, 0x20, 0x40]", "[0x20, 0x40, 0x80]"),
    "noise-follows-tone-2": ("            (self.sign >> 1) & 1\n", "            (self.sign >> 2) & 1\n"),
    "lfsr-output-top-bit": ("lfsr_out: variant.lfsr_bits - 2,", "lfsr_out: variant.lfsr_bits - 1,"),
    "zero-detect-whole-register": ("|| noise & self.lfsr_low == 0", "|| noise == 0"),
    "volume-data-byte-ignored": ("            self.volume[(latch >> 1) as usize] = data & 0x0F;", "            if low {\n                self.volume[(latch >> 1) as usize] = data & 0x0F;\n            }"),
    "noise-write-no-restart": ("self.noise_trig = 1;", "self.noise_trig = 0;"),
    "low-period-two-held": ("if freq[0] <= 1 {", "if freq[0] <= 2 {"),
    "test-bit-ignored": ("            sign |= 15;\n", "            sign |= 0;\n"),
    "power-on-not-run": ("        self.run(16);\n        self.ic = 0;", "        self.ic = 0;"),
    "stereo-sides-swapped": ("if stereo & (0x10 << channel) != 0 {\n                        left", "if stereo & (0x01 << channel) != 0 {\n                        left"),
    "ym7101-curve-one-entry": ("0.382, 0.29", "0.383, 0.29"),
    "ideal-curve-1-5-db": ("10f64.powf(-0.1 * step as f64)", "10f64.powf(-0.075 * step as f64)"),
    # The fork's own paths, which only long run() calls take: the examples' chunked
    # test and rung 6 must catch them.
    "fast-settled-ignores-latched-write": ("&& !self.write_flag && !self.write_flag_l", "&& !self.write_flag"),
    "fast-settled-ignores-ic-latch2": ("self.ic | self.ic_latch1 | self.ic_latch2 == 0", "self.ic | self.ic_latch1 == 0"),
    # Settled groups start at clock & 15 = 2, 6, 10 or 14, so the sample falls
    # after phase 1 of the 14 group; this takes it a group early. (Taking it after
    # that group's phase 2 is an equivalent mutant: in steady state the noise never
    # shifts there.)
    "fast-sample-a-group-early": ("if clock & 15 == 14 {", "if clock & 15 == 10 {"),
    "fast-round-pairs-tone-1-with-counter-2": ("            0x11 => 1,\n", "            0x11 => 2,\n"),
    # Dropping run()'s `self.clock & 3 == 2` is an equivalent mutant from power-on:
    # it guards only a host that sets p1 and p2 itself.
}


def judge(name: str, old: str, new: str) -> tuple[str, bool, str]:
    copy = ROOT / "external" / "faults" / name
    copy.mkdir(parents=True, exist_ok=True)
    for item in ("Cargo.toml", "Cargo.lock", "src", "tests"):
        source = ROOT / item
        if source.is_dir():
            shutil.copytree(source, copy / item, dirs_exist_ok=True)
        else:
            shutil.copy2(source, copy / item)
    chip = copy / "src" / "chip.rs"
    text = (ROOT / "src" / "chip.rs").read_text()
    if text.count(old) != 1:
        return name, False, f"fault text found {text.count(old)} times"
    chip.write_text(text.replace(old, new))
    result = subprocess.run(
        ["nice", "cargo", "test", "--release", "-q", "--test", "conformance"],
        cwd=copy,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return name, False, escalate(copy)
    detail = next((line for line in result.stdout.splitlines() + result.stderr.splitlines() if "line " in line and ":" in line), "")
    return name, True, detail.strip()


def escalate(copy: Path) -> str:
    """Rungs 1, 3 and 5 for a fault the examples missed: what each finds."""
    subprocess.run(["nice", "cargo", "build", "--release", "-q"], cwd=copy, check=True)
    bins = copy / "target" / "release"
    found = []
    rung1 = subprocess.run(
        [str(ROOT / "scripts" / "rung1.sh")],
        env={**os.environ, "BIN_DIR": str(bins)},
        capture_output=True,
        text=True,
    )
    differ = [line.split(":")[0] for line in rung1.stdout.splitlines() if "DIFFERENT" in line]
    found.append(f"rung 1: {len(differ)} differ ({', '.join(differ)[:60]})" if differ else "rung 1: none")
    for rung, folder in (("rung 3", "synthetic"), ("rung 5", "random")):
        manifests = sorted((ROOT / "external" / "manifests" / folder).glob("*.json"))
        if not manifests:
            found.append(f"{rung}: no manifests")
            continue
        result = subprocess.run(
            [str(ROOT / "scripts" / "diff_all.sh"), *map(str, manifests)],
            env={**os.environ, "TRACE_BIN": str(bins / "sn76489-trace"), "JOBS": "4"},
            capture_output=True,
            text=True,
        )
        total = result.stdout.strip().splitlines()[-1] if result.stdout.strip() else result.stderr.strip()
        found.append(f"{rung}: {total.replace('TOTAL: ', '')}")
    rung6 = subprocess.run(
        [str(ROOT / "scripts" / "rung6.sh")],
        env={**os.environ, "CHUNKS_BIN": str(bins / "sn76489-chunks"), "JOBS": "4"},
        capture_output=True,
        text=True,
    )
    found.append("rung 6: " + (rung6.stdout.strip().splitlines() or ["no output"])[-1].replace("TOTAL: ", ""))
    return "; ".join(found)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--jobs", type=int, default=4)
    args = parser.parse_args()
    with ThreadPoolExecutor(args.jobs) as pool:
        results = list(pool.map(lambda item: judge(item[0], *item[1]), FAULTS.items()))
    caught = 0
    for name, failed, detail in results:
        caught += failed
        print(f"{'caught ' if failed else 'examples missed'} {name}: {detail[:150]}")
    print(f"{caught} of {len(FAULTS)} faults caught by the examples")
    return 0


if __name__ == "__main__":
    sys.exit(main())
