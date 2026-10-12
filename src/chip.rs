//! The SN76489 programmable sound generator, run the way the die runs it.
//!
//! A transcription of sn76489-python's `src/sn76489_python/chip.py`, function for
//! function and in the same order, with its names and its citations of Nuked-PSG
//! (`ympsg.c`, pinned at d15a168; GPL-2.0, cited through the reference and never read
//! into this repository). The reference's module header explains the die's machine;
//! the citations below are kept so each line can be checked against it.
//!
//! Where Rust forced a difference:
//!
//! * **Integers.** Every register is a `u32` and the clock a `u64`; nothing in the
//!   engine is negative or wider than 16 bits.
//! * **`levels`** is a [`Levels`] value, not a tuple: [`Levels::Ideal`] is computed
//!   by the reference's formula (`10 ** (-0.1 * step)`, Rust's `powf`) when asked
//!   for, since a `const` cannot call `powf`. [`Levels::Table`] is a host's own curve,
//!   as a host passes its own tuple to the reference.
//! * **The finished samples** are a `Vec<[u8; 4]>` of attenuations and a
//!   `Vec<(f64, f64)>` of mixed levels, summed in the reference's order so the
//!   floating-point results are the same bits.

/// Input clocks per output sample: four channels, four input clocks a phase pair.
pub const CLOCKS_PER_SAMPLE: u64 = 16;

/// The YM7101's curve: Nuked-PSG's values (`ympsg.c` line 7, `ympsg_vol`), which
/// are not 2 dB a step.
pub const YM7101_LEVELS: [f64; 16] = [
    1.0, 0.772, 0.622, 0.485, 0.382, 0.29, 0.229, 0.174, 0.132, 0.096, 0.072, 0.051, 0.034, 0.019,
    0.009, 0.0,
];

/// The ideal attenuation curve: 2 dB a step, 15 silent (the TI data manual,
/// "attenuator"; Maxim's wiki, "Volume/attenuation").
pub fn ideal_levels() -> [f64; 16] {
    let mut levels = [0.0; 16];
    for (step, level) in levels.iter_mut().enumerate().take(15) {
        *level = 10f64.powf(-0.1 * step as f64);
    }
    levels
}

/// Counter reload values for noise shift rates 0-2; rate 3 follows tone channel 3
/// (ympsg.c lines 152-166, where the rate ORs 16, 32 or 64 into the period).
pub const NOISE_PERIODS: [u32; 3] = [0x10, 0x20, 0x40];

/// The output level of each attenuation, 15 being silence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Levels {
    /// [`ideal_levels`]: 2 dB a step.
    Ideal,
    /// [`YM7101_LEVELS`]: the die's.
    Ym7101,
    /// A host's own curve.
    Table([f64; 16]),
}

impl Levels {
    pub fn table(&self) -> [f64; 16] {
        match self {
            Levels::Ideal => ideal_levels(),
            Levels::Ym7101 => YM7101_LEVELS,
            Levels::Table(table) => *table,
        }
    }
}

/// What differs between the SN76489 parts in Sega's machines.
///
/// `lfsr_bits` and `white_noise_taps` (in the wiki's right-shifting notation):
/// 16 bits tapped at bits 0 and 3 ($0009) on the Sega-integrated parts, 15 bits
/// tapped at bits 0 and 1 ($0003) on the discrete TI part (Maxim's wiki, "The Linear
/// Feedback Shift Register"; Nuked-PSG's 16-bit register taps bits 15 and 12, the
/// same pair mirrored).
///
/// `stereo`: the Game Gear's port $06, per-channel left and right enables (Sega's
/// Game Gear manual).
///
/// `levels`: the output level of each attenuation, 15 being silence. The analogue
/// curve is the variant's: the die-measured one for the YM7101, the ideal 2 dB
/// otherwise, since nothing in hand measures the Master System's.
///
/// `low_periods_hold_high`: tone periods 0 and 1 hold the channel's output high.
/// Reported for the Sega Master System part, which games use for sample playback
/// (Maxim's wiki, "Tone channels"); the YM7101's die instead toggles the output once
/// a rotation, as Nuked-PSG does. Where the sources disagree each variant keeps the
/// behaviour its own evidence gives (the reference's docs/claims.md).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PSGVariant {
    pub name: &'static str,
    pub lfsr_bits: u32,
    pub white_noise_taps: u32,
    pub stereo: bool,
    pub levels: Levels,
    pub low_periods_hold_high: bool,
}

impl PSGVariant {
    /// The white-noise taps mirrored for the left-shifting register.
    pub fn left_taps(&self) -> u32 {
        let top = self.lfsr_bits - 1;
        (0..self.lfsr_bits)
            .filter(|bit| self.white_noise_taps >> bit & 1 != 0)
            .map(|bit| 1 << (top - bit))
            .sum()
    }
}

/// The discrete TI SN76489AN of the SG-1000, SC-3000 and Othello Multivision.
pub const SN76489AN: PSGVariant = PSGVariant {
    name: "SN76489AN",
    lfsr_bits: 15,
    white_noise_taps: 0x0003,
    stereo: false,
    levels: Levels::Ideal,
    low_periods_hold_high: true,
};
/// The PSG inside the Master System's 315-5124 and 315-5246 VDPs.
pub const SEGA_VDP_PSG: PSGVariant = PSGVariant {
    name: "Sega VDP",
    lfsr_bits: 16,
    white_noise_taps: 0x0009,
    stereo: false,
    levels: Levels::Ideal,
    low_periods_hold_high: true,
};
/// The Game Gear's PSG: the Sega one, with per-channel stereo enables.
pub const GAME_GEAR_PSG: PSGVariant = PSGVariant {
    name: "Game Gear",
    stereo: true,
    ..SEGA_VDP_PSG
};
/// The PSG inside the Mega Drive's YM7101 VDP: Nuked-PSG's die.
pub const YM7101_PSG: PSGVariant = PSGVariant {
    name: "YM7101",
    lfsr_bits: 16,
    white_noise_taps: 0x0009,
    stereo: false,
    levels: Levels::Ym7101,
    low_periods_hold_high: false,
};

pub const VARIANTS: [PSGVariant; 4] = [SN76489AN, SEGA_VDP_PSG, GAME_GEAR_PSG, YM7101_PSG];

/// The PSG as the die has it: registers, four counters, and the rotation.
///
/// `volume`: the four attenuations (tone 1-3, noise). `freq`: the three 10-bit tone
/// periods. `noise_data`: the 3-bit noise register (bit 2 white, bits 1-0 the shift
/// rate). `stereo`: the Game Gear's port $06 value (bits 7-4 left enables for noise
/// and tones 3-1, bits 3-0 right), $FF on the other variants.
///
/// [`SN76489::attenuations`] is the chip's digital output, what each channel is
/// driving this instant, and is what the referee is compared against.
/// [`SN76489::take_samples`] turns the finished samples into levels.
#[derive(Clone, Debug)]
pub struct SN76489 {
    pub variant: PSGVariant,
    lfsr_mask: u32,
    lfsr_low: u32,
    lfsr_taps: u32,
    lfsr_out: u32,
    level_table: [f64; 16],
    pub volume: [u32; 4],
    pub freq: [u32; 3],
    pub noise_data: u32,
    pub stereo: u32,
    pub latch: u32,
    pub test: u32,
    pub counter: [u32; 4],
    pub counter_of: u32,
    /// Bit 3 tone 1, bit 2 tone 2, bit 1 tone 3, bit 0 noise (ympsg.c line 219).
    pub sign: u32,
    pub noise: u32,
    pub noise_of: u32,
    pub noise_trig: u32,
    pub noise_trig_l: u32,
    pub noise_sign: u32,
    /// The one-hot period selector and the counter index it runs ahead of.
    pub chan_sel: u32,
    pub rot: u32,
    /// The /4 prescaler and the two /IC shift registers.
    pub p1: u32,
    pub p2: u32,
    pub ic_latch1: u32,
    pub ic_latch2: u32,
    pub ic: u32,
    /// The write pipeline (ympsg.c's `data`, `write_flag` and `write_flag_l`): the
    /// data register, which a write replaces at once; a write not yet taken in by a
    /// clock; a byte taken in and still to reach its register.
    data: u32,
    write_flag: bool,
    write_flag_l: bool,
    pub clock: u64,
    /// One entry per finished sample: the four attenuations being driven.
    pub levels: Vec<[u8; 4]>,
    /// Samples already turned into levels by a stereo change ([`SN76489::write_stereo`]).
    mixed: Vec<(f64, f64)>,
}

impl SN76489 {
    pub fn new(variant: PSGVariant) -> Self {
        let lfsr_mask = (1 << variant.lfsr_bits) - 1;
        let mut chip = SN76489 {
            variant,
            lfsr_mask,
            lfsr_low: lfsr_mask >> 1,
            lfsr_taps: variant.left_taps(),
            lfsr_out: variant.lfsr_bits - 2,
            level_table: variant.levels.table(),
            volume: [0xF; 4],
            freq: [0; 3],
            noise_data: 0,
            stereo: 0xFF,
            latch: 0,
            test: 0,
            counter: [0; 4],
            counter_of: 0,
            sign: 0,
            noise: 0,
            noise_of: 0,
            noise_trig: 0,
            noise_trig_l: 0,
            noise_sign: 0,
            chan_sel: 0,
            rot: 0,
            p1: 0,
            p2: 0,
            ic_latch1: 0,
            ic_latch2: 0,
            ic: 1,
            data: 0,
            write_flag: false,
            write_flag_l: false,
            clock: 0,
            levels: Vec::new(),
            mixed: Vec::new(),
        };
        chip.reset();
        chip
    }

    /// Power-on: /IC held for 16 input clocks, as a console's reset does.
    ///
    /// Nuked-PSG's `YMPSG_Init` (ympsg.c lines 256-266) holds /IC over 16 clocks and
    /// then releases it, which zeroes the registers through `reg_reset`, silences the
    /// channels through `data_mask`, and leaves the sequencer part way round:
    /// counters at 1, the overflow register full, the one-hot selector at bit 3.
    /// Running the same sequence here is shorter than asserting the resulting state
    /// and cannot drift from it.
    pub fn reset(&mut self) {
        self.volume = [0xF; 4];
        self.freq = [0; 3];
        self.noise_data = 0;
        self.stereo = 0xFF;
        self.latch = 0;
        self.test = 0;
        self.counter = [0; 4];
        self.counter_of = 0;
        self.sign = 0;
        self.noise = 0;
        self.noise_of = 0;
        self.noise_trig = 0;
        self.noise_trig_l = 0;
        self.noise_sign = 0;
        self.chan_sel = 0;
        self.rot = 0;
        self.p1 = 0;
        self.p2 = 0;
        self.ic_latch1 = 0;
        self.ic_latch2 = 0;
        self.ic = 1;
        self.data = 0;
        self.write_flag = false;
        self.write_flag_l = false;
        self.clock = 0;
        self.levels = Vec::new();
        self.mixed = Vec::new();

        self.run(16);
        self.ic = 0;
        // /IC drives reg_reset and data_mask, which zero the periods and the noise
        // register and force every attenuation to 15 (ympsg.c YMPSG_UpdateRegisters,
        // lines 24-107). Asserted here rather than run, because a write during /IC --
        // the only thing the loop would do differently -- cannot happen on a console:
        // the board holds the PSG in reset before the CPUs run.
        self.volume = [0xF; 4];
        self.freq = [0; 3];
        self.noise_data = 0;
        self.clock = 0;
        self.levels.clear();
    }

    /// Whether `other` is in the same state, every field but the finished samples
    /// (`levels`, which a caller compares itself, and those already mixed by a stereo
    /// change, which `take_samples` returns). Rung 6's check that a long `run()`
    /// leaves what clocks one at a time leave; the destructuring makes a new field a
    /// compile error here until it is compared.
    pub fn same_state(&self, other: &SN76489) -> bool {
        let SN76489 {
            variant,
            lfsr_mask,
            lfsr_low,
            lfsr_taps,
            lfsr_out,
            level_table,
            volume,
            freq,
            noise_data,
            stereo,
            latch,
            test,
            counter,
            counter_of,
            sign,
            noise,
            noise_of,
            noise_trig,
            noise_trig_l,
            noise_sign,
            chan_sel,
            rot,
            p1,
            p2,
            ic_latch1,
            ic_latch2,
            ic,
            data,
            write_flag,
            write_flag_l,
            clock,
            levels: _,
            mixed: _,
        } = self;
        *variant == other.variant
            && *lfsr_mask == other.lfsr_mask
            && *lfsr_low == other.lfsr_low
            && *lfsr_taps == other.lfsr_taps
            && *lfsr_out == other.lfsr_out
            && level_table.map(f64::to_bits) == other.level_table.map(f64::to_bits)
            && *volume == other.volume
            && *freq == other.freq
            && *noise_data == other.noise_data
            && *stereo == other.stereo
            && *latch == other.latch
            && *test == other.test
            && *counter == other.counter
            && *counter_of == other.counter_of
            && *sign == other.sign
            && *noise == other.noise
            && *noise_of == other.noise_of
            && *noise_trig == other.noise_trig
            && *noise_trig_l == other.noise_trig_l
            && *noise_sign == other.noise_sign
            && *chan_sel == other.chan_sel
            && *rot == other.rot
            && *p1 == other.p1
            && *p2 == other.p2
            && *ic_latch1 == other.ic_latch1
            && *ic_latch2 == other.ic_latch2
            && *ic == other.ic
            && *data == other.data
            && *write_flag == other.write_flag
            && *write_flag_l == other.write_flag_l
            && *clock == other.clock
    }

    // -- the ports ------------------------------------------------------------------

    /// The data port. Reading it locks up a Mega Drive (CMD `gen-hw.txt` 1) and is
    /// not wired on the Master System, so there is no read.
    ///
    /// The byte reaches its register on the next input clock: run the chip up to the
    /// moment of the write first. It replaces the data register at once, so a write on
    /// the clock straight after another hands the first one's register update its own
    /// byte, as the die does (`YMPSG_Write`, ympsg.c lines 238-242). No console can
    /// write that fast; the conformance kit's `back-to-back` example does.
    pub fn write(&mut self, value: u8) {
        self.data = value as u32;
        self.write_flag = true;
    }

    /// The Game Gear's port $06: per-channel left and right enables. Ignored on the
    /// other variants. Samples finished before the write keep the old routing.
    pub fn write_stereo(&mut self, value: u8) {
        if self.variant.stereo {
            let levels = std::mem::take(&mut self.levels);
            let mixed = self.mix(&levels);
            self.mixed.extend(mixed);
            self.stereo = value as u32;
        }
    }

    /// The three test bits, which Nuked-PSG takes as `YMPSG_Test` (ympsg.c line
    /// 336). Bit 0 forces every channel's sign high; the other two select one channel
    /// for an analogue test output, which this model has no counterpart for and
    /// ignores. On the Mega Drive they are the VDP debug register's bits 11-9.
    pub fn write_test(&mut self, value: u8) {
        self.test = value as u32 & 7;
    }

    // -- time -----------------------------------------------------------------------

    /// Advance `clocks` input clocks, appending finished samples.
    ///
    /// Each clock is [`SN76489::tick`], except that once the chip is settled (see
    /// [`SN76489::groups`]) it runs four clocks at a time: the state and the samples
    /// at the end are what the clocks one at a time would have left, which rung 6
    /// checks at every boundary of every manifest.
    pub fn run(&mut self, clocks: u64) {
        let end = self.clock + clocks;
        while self.clock < end {
            if end - self.clock >= 4
                && self.p1 == 0
                && self.p2 == 0
                && self.clock & 3 == 2
                && self.settled()
            {
                self.groups((end - self.clock) / 4);
            } else {
                self.tick();
            }
        }
    }

    /// One input clock.
    ///
    /// The body is `YMPSG_Clock` (ympsg.c lines 273-306): the /4 prescaler, the /IC
    /// shift register that resets it, then the registers, the channel latch and one
    /// of the two internal phases.
    ///
    /// The register update happens **twice**: once at the head of the clock after the
    /// write, and once inside the second internal phase, which the write's own clock
    /// may be (ympsg.c calls `YMPSG_UpdateRegisters` at line 287 and again at line
    /// 209). So a write that lands on that phase reaches the register a clock earlier
    /// than one that does not.
    fn tick(&mut self) {
        let p2_latch = self.p2;
        self.p2 = (self.p2 + self.p1) & 1;
        let mut p1 = self.p1 ^ 1;
        if self.ic_latch1 == 2 {
            p1 = 0;
            self.p2 = 0;
        }
        self.p1 = p1;
        self.ic_latch1 = ((self.ic_latch1 << 1) | self.ic) & 3;
        if self.write_flag_l {
            self.update_registers();
            self.write_flag_l = false;
        }
        if self.write_flag {
            // The channel latch is taken on the clock of the write itself.
            if self.data & 0x80 != 0 {
                self.latch = (self.data >> 4) & 7;
            }
            self.write_flag = false;
            self.write_flag_l = true;
        }
        if p1 != 0 {
            if p2_latch != 0 {
                self.phase2::<false>();
            } else {
                self.phase1::<false>();
            }
        }
        self.clock += 1;
        if self.clock % CLOCKS_PER_SAMPLE == 0 {
            let now = self.attenuations();
            self.levels.push(now);
        }
    }

    /// Nothing in flight that [`SN76489::groups`] leaves out: /IC released and
    /// out of both its shift registers, and no write waiting to be taken in.
    fn settled(&self) -> bool {
        self.ic | self.ic_latch1 | self.ic_latch2 == 0 && !self.write_flag && !self.write_flag_l
    }

    /// `count` groups of four clocks from where the prescaler is about to run the
    /// first internal phase at `clock & 3` = 2, on a settled chip. With /IC
    /// released the prescaler runs free, so of each four clocks the first runs
    /// `phase1` and the third `phase2`, and with no write in flight
    /// [`SN76489::tick`]'s register steps do nothing; what is left is the two phases
    /// and the sample, which falls two clocks into the group starting at `clock & 15`
    /// = 14, after its `phase1`. Nothing in a group can unsettle the chip: only a
    /// write or [`SN76489::reset`] does.
    ///
    /// Power-on leaves the prescaler at that alignment and nothing but /IC moves it;
    /// a host that sets `p1` and `p2` itself runs clock by clock instead.
    fn groups(&mut self, count: u64) {
        let mut clock = self.clock;
        for _ in 0..count {
            self.phase1::<true>();
            if clock & 15 == 14 {
                let now = self.attenuations();
                self.levels.push(now);
            }
            self.phase2::<true>();
            clock += 4;
        }
        self.clock = clock;
    }

    /// The latched byte into the register the channel latch names (ympsg.c
    /// `YMPSG_UpdateRegisters`, lines 24-107).
    ///
    /// A byte with bit 7 set carries the low four data bits; one with bit 7 clear
    /// carries the high six bits of a tone register. Bit 7 does not select the
    /// register: the latch does, so a byte with bit 7 clear written while a volume or
    /// noise register is latched is written there whole ("The data byte is NOT
    /// ignored", Maxim's wiki: Micro Machines' noise is wrong otherwise).
    fn update_registers(&mut self) {
        let data = self.data;
        let low = data & 0x80 != 0;
        let latch = self.latch;
        if latch & 1 != 0 {
            // 1, 3, 5, 7: the attenuations of tone 1-3 and noise
            self.volume[(latch >> 1) as usize] = data & 0x0F;
        } else if latch == 6 {
            self.noise_data = data & 0x07;
            self.noise_trig = 1;
        } else if low {
            let channel = (latch >> 1) as usize;
            self.freq[channel] = (self.freq[channel] & 0x3F0) | (data & 0x0F);
        } else {
            let channel = (latch >> 1) as usize;
            self.freq[channel] = (self.freq[channel] & 0x00F) | ((data << 4) & 0x3F0);
        }
    }

    /// The first internal phase: the noise register, then one counter's step.
    ///
    /// `ympsg.c` `YMPSG_ClockInternal1`, lines 109-199. `chan_sel` is a one-hot shift
    /// register naming whose *period* this step compares against; `rot` names the
    /// counter. The four expiries collected in `counter_of` flip the four signs
    /// together on the step that serves tone 1.
    ///
    /// `SETTLED`: called from [`SN76489::groups`], where `ic_latch2` is known to be
    /// zero, so the compiler drops the /IC paths.
    #[inline(always)]
    fn phase1<const SETTLED: bool>(&mut self) {
        let ic_latch2 = if SETTLED { 0 } else { self.ic_latch2 };
        let noise_data = self.noise_data;
        // The noise generator follows tone 3's sign in shift rate 3, its own counter's
        // otherwise, and shifts on that signal's rising edge.
        let noise_of = if noise_data & 3 == 3 {
            (self.sign >> 1) & 1
        } else {
            self.sign & 1
        };
        if self.noise_trig_l != 0 || ic_latch2 & 1 != 0 {
            self.noise = 0;
        } else if noise_of != 0 && self.noise_of == 0 {
            self.shift();
        }
        self.noise_of = noise_of;

        let chan_sel = if ic_latch2 & 2 != 0 { 0 } else { self.chan_sel };
        let mut period = 0;
        if chan_sel & 1 != 0 {
            period = self.freq[0];
        }
        if chan_sel & 2 != 0 {
            period |= self.freq[1];
        }
        if chan_sel & 4 != 0 {
            period |= self.freq[2];
        }
        if chan_sel & 8 != 0 {
            let rate = noise_data & 3;
            if rate != 3 {
                period |= NOISE_PERIODS[rate as usize];
            }
        }
        if chan_sel & 1 != 0 {
            self.sign ^= self.counter_of & 15;
        }
        if ic_latch2 & 2 != 0 {
            self.sign = 0;
        }
        self.counter_of = (self.counter_of << 1) & 0xFF;
        let rot = self.rot as usize;
        let mut counter = self.counter[rot];
        if counter >= period {
            self.counter_of |= 1;
            counter = 0;
        }
        if ic_latch2 & 2 != 0 {
            counter = 0;
        }
        self.counter[rot] = (counter + 1) & 0x3FF;
        // The one-hot rotation: a 1 is fed in when the three tone places are clear.
        let feed = if ic_latch2 & 1 != 0 || self.chan_sel & 7 != 0 {
            0
        } else {
            1
        };
        self.chan_sel = ((self.chan_sel << 1) | feed) & 0xFF;
        self.ic_latch2 = ((ic_latch2 << 1) | self.ic) & 0xFF;
        self.noise_trig_l = self.noise_trig;
    }

    /// One step of the noise register (ympsg.c lines 126-134): white noise feeds back
    /// the parity of the tapped bits, periodic noise nothing, and an all-zero register
    /// below the top bit takes a 1 either way, which is what makes periodic noise a
    /// single 1 circulating the register's width.
    fn shift(&mut self) {
        let noise = self.noise;
        let feedback =
            (noise & self.lfsr_taps).count_ones() & 1 != 0 && (self.noise_data >> 2) & 1 != 0;
        let bit = if feedback || noise & self.lfsr_low == 0 {
            1
        } else {
            0
        };
        self.noise = ((noise << 1) | bit) & self.lfsr_mask;
    }

    /// The second internal phase: the counter index steps and the LFSR's output bit
    /// is latched (`ympsg.c` `YMPSG_ClockInternal2`, lines 201-214).
    ///
    /// `SETTLED`: called from [`SN76489::groups`], where no write is in flight.
    #[inline(always)]
    fn phase2<const SETTLED: bool>(&mut self) {
        if self.noise_trig_l != 0 {
            self.noise_trig = 0;
        }
        if !SETTLED && self.write_flag_l {
            self.update_registers();
        }
        self.rot = (self.rot + 1) & 3;
        self.noise_sign = (self.noise >> self.lfsr_out) & 1;
    }

    // -- output ---------------------------------------------------------------------

    /// What each channel is driving now: its attenuation while its sign bit is high,
    /// silence (15) otherwise (ympsg.c `YMPSG_UpdateSample`, lines 216-236). The noise
    /// channel uses the latched LFSR bit, the tone channels their signs.
    pub fn attenuations(&self) -> [u8; 4] {
        let mut sign = (self.sign & 14) | self.noise_sign;
        if self.test & 1 != 0 {
            sign |= 15;
        }
        let volume = self.volume;
        if self.variant.low_periods_hold_high {
            let freq = self.freq;
            if freq[0] <= 1 {
                sign |= 8;
            }
            if freq[1] <= 1 {
                sign |= 4;
            }
            if freq[2] <= 1 {
                sign |= 2;
            }
        }
        let driven = |bit: u32, channel: usize| {
            if sign & bit != 0 {
                volume[channel] as u8
            } else {
                15
            }
        };
        [driven(8, 0), driven(4, 1), driven(2, 2), driven(1, 3)]
    }

    fn mix(&self, levels: &[[u8; 4]]) -> Vec<(f64, f64)> {
        let table = &self.level_table;
        let stereo = self.stereo;
        if stereo == 0xFF {
            return levels
                .iter()
                .map(|&[a, b, c, d]| {
                    let level = table[a as usize]
                        + table[b as usize]
                        + table[c as usize]
                        + table[d as usize];
                    (level, level)
                })
                .collect();
        }
        // Bits 7-4 left, 3-0 right; within each, bit 0 tone 1 ... bit 3 noise.
        levels
            .iter()
            .map(|attenuations| {
                let (mut left, mut right) = (0.0, 0.0);
                for (channel, &attenuation) in attenuations.iter().enumerate() {
                    let level = table[attenuation as usize];
                    if stereo & (0x10 << channel) != 0 {
                        left += level;
                    }
                    if stereo & (0x01 << channel) != 0 {
                        right += level;
                    }
                }
                (left, right)
            })
            .collect()
    }

    /// Return and clear the finished samples as `(left, right)` levels, each 0.0-4.0
    /// (four channels of 0.0-1.0) by the variant's curve. Without stereo the two sides
    /// are equal. Resampling and the analogue path belong to the board.
    pub fn take_samples(&mut self) -> Vec<(f64, f64)> {
        let mut mixed = std::mem::take(&mut self.mixed);
        let levels = std::mem::take(&mut self.levels);
        mixed.extend(self.mix(&levels));
        mixed
    }

    /// Return and clear the finished samples as the chip's own digital state, the
    /// four attenuations, which is what the Nuked-PSG comparison is made on. Stereo
    /// is not applied.
    pub fn take_attenuations(&mut self) -> Vec<[u8; 4]> {
        self.mixed.clear();
        std::mem::take(&mut self.levels)
    }
}
