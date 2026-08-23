//! Checking, every batch, that the device still agrees with the processor.
//!
//! # Why a fixture is not enough
//!
//! `name_them` rebuilds and rehashes every hit on the processor, so no device can introduce a
//! *wrong* name. What it cannot do is notice a *missing* one: a hit the device never reported is a
//! name that is simply not there, and a pass that quietly finds fewer is indistinguishable from a
//! pass over ground that had less in it. That is the failure this whole project fears most, and
//! the GPU adapter shipped with only three defences against it, all of them off to one side:
//!
//! - `gpuinfo` is opt-in, run once, on synthetic stems and a filter sized by the fixture.
//! - `tests/backend.rs` runs on whoever wrote the code, not on whoever runs it.
//! - CI cannot see a contributor's driver at all.
//!
//! A pass has real stem lengths and a filter sized by real ids. A bug that only appears at that
//! shape passes all three and then loses names on the night it matters.
//!
//! So a sample of every batch is re-swept here, and the two answers must match exactly.
//!
//! # Why the sample is scattered
//!
//! Several windows spread across the whole stem list rather than one block at the front. The worst
//! bug of this kind is per-launch rather than uniform -- a launch-relative index used where an
//! absolute one was meant gives a first launch that is entirely right and every later one wrong --
//! and a contiguous sample taken from the front would see nothing at all.
//!
//! # Why it is fatal
//!
//! A disagreement is not an out-of-memory or a timeout, and must not be treated like one. It means
//! what the kernel computes and what the engine computes have come apart; every later batch would
//! be just as wrong and just as quiet. So the device is put away for the rest of the run and the
//! pass finishes on the processor, slowly and correctly.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::ports::{Backend, Hit, SweepRequest};

/// How the sample is chosen, from the environment.
///
/// - unset, or `sample`: several windows per batch, a few milliseconds
/// - `all`: every stem, every batch. Gives up the speedup and is the run to make once on a card
///   nobody has witnessed, or after touching the kernel. Far more convincing than a fixture.
/// - `off`: for somebody who has measured their own device and wants the last few percent
pub const VERIFY: &str = "SLASHER_GPU_VERIFY";

/// How many windows the sample is spread over.
const WINDOWS: usize = 8;

/// How many stems each window holds.
const WINDOW: usize = 512;

/// What a check concluded.
pub enum Verdict {
    /// The two agreed on everything sampled.
    Agreed,
    /// They did not. Carries what to tell somebody, already worded.
    Disagreed(String),
    /// Nothing was compared, because the caller asked for none.
    Skipped,
}

/// Whether verification is wanted, and how much of it.
enum Wanted {
    Nothing,
    Sample,
    Everything,
}

fn wanted() -> Wanted {
    match std::env::var(VERIFY)
        .unwrap_or_default()
        .to_lowercase()
        .trim()
    {
        "off" | "0" | "no" | "false" => Wanted::Nothing,
        "all" | "every" | "full" => Wanted::Everything,
        _ => Wanted::Sample,
    }
}

/// Re-sweeps part of a batch on the processor and compares, exactly.
pub fn check(request: &SweepRequest<'_>, from_device: &[Hit], cpu: &dyn Backend) -> Verdict {
    let total = request.stems.len();

    let sampled: Vec<u32> = match wanted() {
        Wanted::Nothing => return Verdict::Skipped,
        Wanted::Everything => (0..total as u32).collect(),
        Wanted::Sample => scatter(total),
    };

    if sampled.is_empty() {
        return Verdict::Skipped;
    }

    let subset = request.stems.subset(&sampled);

    let theirs = {
        let mut kept: Vec<Hit> = from_device
            .iter()
            .copied()
            .filter(|hit| sampled.binary_search(&stem_of(*hit)).is_ok())
            .collect();

        // The device appends hits as its groups finish, which is not deterministic and does not
        // need to be. Ordering both sides the same way is what makes an exact comparison possible;
        // ordering them into *agreement* would be how a real disagreement gets hidden, so the key
        // is the order the processor emits in and nothing looser.
        kept.sort_by_key(|hit| emitted(*hit));
        kept
    };

    let ours = {
        let ask = SweepRequest {
            stems: &subset,
            openings: request.openings,
            bare: request.bare,
            peeled: request.peeled,
        };

        let Ok(found) = cpu.sweep(&ask) else {
            return Verdict::Skipped;
        };

        // The subset renumbered its stems from zero, so put the batch's own indices back before
        // anything is compared.
        let mut ours: Vec<Hit> = found
            .into_iter()
            .map(|hit| {
                let inside = stem_of(hit) as usize;
                (u64::from(sampled[inside]) << 32) | (hit & 0xFFFF_FFFF)
            })
            .collect();

        ours.sort_by_key(|hit| emitted(*hit));
        ours
    };

    if theirs == ours {
        return Verdict::Agreed;
    }

    let missed = ours.iter().filter(|hit| !theirs.contains(hit)).count();
    let invented = theirs.iter().filter(|hit| !ours.contains(hit)).count();

    Verdict::Disagreed(format!(
        "the device and the processor disagree on {} of {total} stems: {missed} name(s) the \
         processor found that the device did not, and {invented} the device reported that the \
         processor does not. This is not a device declining work -- it means the kernel and the \
         engine have come apart, so every later batch would be wrong the same way and just as \
         quietly. The device is not used again in this run; the pass continues on the processor. \
         Please run `gpuinfo` and send what it writes.",
        sampled.len(),
    ))
}

/// Windows spread evenly across the batch, as sorted stem indices.
fn scatter(total: usize) -> Vec<u32> {
    if total <= WINDOWS * WINDOW {
        return (0..total as u32).collect();
    }

    let mut picked = Vec::with_capacity(WINDOWS * WINDOW);

    for window in 0..WINDOWS {
        // Spread so the last window ends at the end of the batch rather than short of it: the
        // final launch is exactly where a launch-relative index goes wrong.
        let start = (total - WINDOW) * window / (WINDOWS - 1);

        picked.extend((start..start + WINDOW).map(|stem| stem as u32));
    }

    picked.sort_unstable();
    picked.dedup();
    picked
}

fn stem_of(hit: Hit) -> u32 {
    (hit >> 32) as u32
}

/// The order the processor emits hits in.
///
/// Not a plain ascending sort on the mark. `BARE` is `0xFFFF_FFFF`, the *largest* opening index
/// there can be, and the processor tries the bare stem **first** -- so sorting on the mark would
/// put the bare hit last for its stem and every comparison would fail on batches that had one.
fn emitted(hit: Hit) -> (u32, u32) {
    let opening = (hit & 0xFFFF_FFFF) as u32;

    (
        stem_of(hit),
        if opening == u32::MAX { 0 } else { opening + 1 },
    )
}

/// Whether a device has been put away for the rest of this run.
///
/// A run-long latch and not a counter: once the two have come apart there is nothing to recover
/// to, and trying again would only produce another quiet batch.
#[derive(Default)]
pub struct Trust(AtomicBool);

impl Trust {
    pub fn broken(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn break_it(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::cpu::Cpu;
    use crate::ports::{PeeledSet, StemBatch};
    use crate::{feed, hash64, Filter};

    /// A batch with enough planted hits, spread widely, that dropping one is a real disagreement
    /// rather than an empty comparison.
    fn a_batch() -> (Vec<String>, Vec<u64>, Vec<u64>) {
        let openings: Vec<u64> = (0..8)
            .map(|n| hash64(&format!("weapons/tier{n}/")))
            .collect();
        let stems: Vec<String> = (0..20_000).map(|n| format!("part_{n:05}_body")).collect();

        let mut wanted: Vec<u64> = stems
            .iter()
            .enumerate()
            .filter(|(index, _)| index % 7 == 0)
            .map(|(index, stem)| feed(openings[index % openings.len()], stem.as_bytes()))
            .collect();

        wanted.sort_unstable();
        wanted.dedup();

        (stems, openings, wanted)
    }

    /// The whole point: a device that quietly loses a hit is caught.
    ///
    /// This is the failure nothing downstream can see. `name_them` rehashes on the processor so a
    /// *wrong* name is impossible, and a *missing* one looks exactly like ground that had less in
    /// it. If this test ever stops failing for a truncated answer, the adapter has gone back to
    /// trusting a device it cannot check.
    #[test]
    fn a_device_that_loses_a_hit_is_caught() {
        let (stems, openings, wanted) = a_batch();
        let packed = StemBatch::pack(&stems, true);
        let filter = Filter::sized(wanted.iter(), wanted.len());
        let (coarse, fine, coarse_bits, fine_bits) = filter.parts();

        let request = SweepRequest {
            stems: &packed,
            openings: &openings,
            bare: true,
            peeled: PeeledSet {
                hashes: &wanted,
                coarse,
                fine,
                coarse_bits,
                fine_bits,
            },
        };

        let cpu = Cpu::new();
        let honest = cpu.sweep(&request).expect("the processor never declines");

        assert!(
            honest.len() > 100,
            "the fixture has to have enough hits for a dropped one to be findable"
        );

        assert!(
            matches!(check(&request, &honest, &cpu), Verdict::Agreed),
            "an honest answer must pass, or every run would stop on its first batch"
        );

        // One hit removed, from the far end -- where a launch-relative index goes wrong and where
        // a sample taken from the front would never look.
        let mut truncated = honest.clone();
        truncated.pop();

        assert!(
            matches!(check(&request, &truncated, &cpu), Verdict::Disagreed(_)),
            "a device that dropped a hit was believed"
        );

        // And one invented, which cannot produce a wrong name but does mean the two have come
        // apart, and the next thing they disagree about might not be harmless.
        let mut invented = honest.clone();
        invented.push((1_u64 << 32) | 3);

        assert!(
            matches!(check(&request, &invented, &cpu), Verdict::Disagreed(_)),
            "a device that invented a hit was believed"
        );
    }

    /// The bare hit is the largest opening index and the *first* thing the processor emits for its
    /// stem. Sorting on the mark would put it last and every batch containing one would look like
    /// a disagreement -- which would take the device away from everybody, permanently, on the
    /// first batch.
    #[test]
    fn the_bare_hit_sorts_where_the_processor_emits_it() {
        let stem = 5_u64 << 32;
        let bare = stem | u64::from(u32::MAX);
        let first_opening = stem;

        assert_eq!(emitted(bare), (5, 0));
        assert_eq!(emitted(first_opening), (5, 1));
        assert!(emitted(bare) < emitted(first_opening));
    }

    /// The sample has to reach the end of the batch. A per-launch bug leaves the first launch
    /// perfectly correct, so a window set that stops short is a window set that sees nothing.
    #[test]
    fn the_sample_reaches_both_ends() {
        let picked = scatter(1_000_000);

        assert_eq!(picked.first(), Some(&0));
        assert_eq!(picked.last(), Some(&999_999));
        assert!(
            picked.len() >= WINDOW,
            "a sample of one window is not a sample"
        );
    }
}
