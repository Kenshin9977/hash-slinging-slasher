//! The capability ladder: one rung per thing a driver could get wrong, lowest first.
//!
//! # What this is for
//!
//! Somebody with a Radeon runs `gpuinfo` once and sends what it prints. That is the whole
//! interaction, and it has to be enough — every extra round trip costs a day and some goodwill,
//! and the person helping did not sign up to be a debugger.
//!
//! So a failure has to identify itself. "The peel crashes" is the start of an investigation;
//! "rung 6 (bytes read backwards) gives the wrong answer, rung 5 (the same bytes forwards) is
//! fine" is the end of one. That exact pair is what an afternoon of bisecting by hand established
//! about Mesa, and it is what one run of this would have said immediately.
//!
//! # Why each rung is its own process
//!
//! A driver that faults takes the process with it — that is not hypothetical, it is what Intel's
//! packaged compiler does on hardware newer than itself. If the ladder ran in one process, a
//! fault on rung 6 would destroy the results of rungs 7 to 10 along with it, and the report would
//! be a fault with no ladder in it at all.
//!
//! So the parent runs each rung as a child of its own. A child that dies is recorded as having
//! died, and the next rung starts regardless. The cost is a process and a program build per rung,
//! a few seconds altogether, paid once by somebody who is doing us a favour.

use crate::adapters::opencl::Device;

/// The source of the micro-kernels, built separately from the real one.
///
/// Separate on purpose: a driver that cannot build `meet.cl` may well build this, and knowing
/// which of the two it choked on is itself a result.
pub const SOURCE: &str = include_str!("kernels/ladder.cl");

/// How many values each rung is given. Small: this is a correctness ladder, not a benchmark, and
/// it has to finish quickly on an integrated GPU and inside a simulator.
pub const WIDTH: usize = 256;

/// The bytes every rung that reads bytes is given.
pub const BYTES: usize = 32;

/// The scalar handed to each rung, and the number of bytes the byte rungs fold.
pub const ARG: u64 = 17;

/// FNV-1a's multiplier, as the kernels use it.
const PRIME: u64 = 0x0000_0100_0000_01B3;

pub struct Rung {
    /// The kernel to run.
    pub kernel: &'static str,
    /// What a person reading the report should understand this to mean.
    pub what: &'static str,
    /// What it tells you when this one fails and the one before it did not.
    pub so_what: &'static str,
}

/// Lowest first, so that the first failure is the most specific thing that is wrong.
pub const RUNGS: &[Rung] = &[
    Rung {
        kernel: "rung_copy",
        what: "read a buffer and write another",
        so_what: "nothing below this is meaningful; the device is not running kernels at all",
    },
    Rung {
        kernel: "rung_mul_arg",
        what: "multiply 64-bit numbers, one of them a kernel argument",
        so_what:
            "either the multiply is wrong or the argument is not arriving; rung 3 separates them",
    },
    Rung {
        kernel: "rung_mul_literal",
        what: "the same multiply, by a constant",
        so_what: "the 64-bit multiply itself is wrong, which no correct answer can survive",
    },
    Rung {
        kernel: "rung_split_multiply",
        what: "the shift-and-add that replaces that multiply",
        so_what: "a 64-bit shift is being done at 32 bits somewhere",
    },
    Rung {
        kernel: "rung_bytes_forward",
        what: "read single bytes at a run-time index",
        so_what: "byte addressing is wrong, which breaks every stem and every ending",
    },
    Rung {
        kernel: "rung_bytes_backward",
        what: "the same bytes, walked backwards",
        so_what: "the loop that counts down is mishandled -- this is the rung Mesa 25.2 fails",
    },
    Rung {
        kernel: "rung_unrolled_window",
        what: "thirty-two bytes folded from registers, fully unrolled",
        so_what: "the unrolled fold is miscompiled; the forward sweep cannot be trusted",
    },
    Rung {
        kernel: "rung_binary_search",
        what: "binary search through a sorted table",
        so_what:
            "membership is wrong, so the search would either miss every name or claim all of them",
    },
    Rung {
        kernel: "rung_atomic",
        what: "atomic increment of a shared counter",
        so_what: "hits would be lost or double-counted, quietly",
    },
    Rung {
        kernel: "rung_grid_stride",
        what: "cover more work than the launch has threads",
        so_what: "candidates would be skipped or tried twice with no sign of it",
    },
];

/// The values every rung is given.
pub fn input() -> Vec<u64> {
    // Sorted and spread, because rung 8 searches this same array and needs it ordered. Distinct
    // in their high bits as well as their low, so a device that only gets half a word right is
    // caught rather than flattered.
    (0..WIDTH as u64)
        .map(|n| n.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 1)
        .collect::<Vec<u64>>()
        .tap_sorted()
}

/// The bytes every rung is given.
pub fn bytes() -> Vec<u8> {
    (0..BYTES as u8).map(|n| b'a'.wrapping_add(n)).collect()
}

/// What the rung should produce, worked out here in plain Rust.
///
/// This is the definition. If it and the device disagree, the device is wrong -- which is only
/// true because every line below is the same arithmetic the CPU adapter does for real, and that
/// arithmetic is checked against `slasher::feed` by the test suite.
pub fn expected(rung: usize, input: &[u64], bytes: &[u8]) -> Vec<u64> {
    let n = input.len();
    let len = ARG as usize;

    match RUNGS[rung].kernel {
        "rung_copy" => input.to_vec(),

        "rung_mul_arg" => input.iter().map(|v| v.wrapping_mul(ARG)).collect(),

        "rung_mul_literal" => input.iter().map(|v| v.wrapping_mul(PRIME)).collect(),

        "rung_split_multiply" => input
            .iter()
            .map(|v| (v << 40).wrapping_add(v.wrapping_mul(435)))
            .collect(),

        "rung_bytes_forward" => input
            .iter()
            .map(|v| {
                bytes[..len]
                    .iter()
                    .fold(*v, |h, b| (h ^ u64::from(*b)).wrapping_mul(PRIME))
            })
            .collect(),

        "rung_bytes_backward" => input
            .iter()
            .map(|v| {
                bytes[..len]
                    .iter()
                    .rev()
                    .fold(*v, |h, b| (h ^ u64::from(*b)).wrapping_mul(PRIME))
            })
            .collect(),

        "rung_unrolled_window" => input
            .iter()
            .map(|v| {
                bytes[..len.min(32)]
                    .iter()
                    .fold(*v, |h, b| (h ^ u64::from(*b)).wrapping_mul(PRIME))
            })
            .collect(),

        "rung_binary_search" => (0..n)
            .map(|i| {
                // Odd indices look for a value one greater, which is not in the table.
                u64::from(i % 2 == 0)
            })
            .collect(),

        // Only the first word is written, and every work item under `n` adds one to it.
        "rung_atomic" => {
            let mut want = vec![0_u64; n];
            want[0] = n as u64;
            want
        }

        "rung_grid_stride" => (0..n as u64).collect(),

        other => unreachable!("no expectation written for {other}"),
    }
}

/// Runs one rung on a device and says whether it agreed.
pub fn run(device: &Device, rung: usize) -> Result<(), String> {
    let input = input();
    let bytes = bytes();

    let got = device
        .run_rung(RUNGS[rung].kernel, &input, &bytes, ARG)
        .map_err(|why| why.to_string())?;

    let want = expected(rung, &input, &bytes);

    if got == want {
        return Ok(());
    }

    // The first disagreement and how many there are, not a dump. Somebody is going to paste this
    // into a message, and three lines that name the problem beat three hundred that contain it.
    let wrong = got.iter().zip(&want).filter(|(a, b)| a != b).count();

    let at = got.iter().zip(&want).position(|(a, b)| a != b).unwrap_or(0);

    Err(format!(
        "{wrong} of {} values differ; the first is at index {at}, \
         where the device said {:016x} and the answer is {:016x}",
        want.len(),
        got[at],
        want[at],
    ))
}

/// A small helper so `input` reads as one expression.
trait TapSorted {
    fn tap_sorted(self) -> Self;
}

impl TapSorted for Vec<u64> {
    fn tap_sorted(mut self) -> Self {
        self.sort_unstable();
        self.dedup();
        self
    }
}
