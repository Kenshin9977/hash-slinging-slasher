//! What this machine's GPU is, and whether it gets the same answers as the CPU.
//!
//! This binary exists because of a constraint that will not go away: the person who wrote the
//! kernel cannot test it on an AMD card or an Intel iGPU, and those are most of the people who
//! will run it. There is no amount of care that substitutes for hardware. What there is instead
//! is a single command a contributor can run in ten seconds, whose output contains every number
//! that decides whether the kernel works on their device -- so that a bug on hardware nobody here
//! owns is one paste away from being diagnosed rather than a conversation.
//!
//! It is also the check that has to pass before a device is trusted with a real pass. The GPU and
//! the CPU are handed the same bytes and their answers are compared exactly. Not approximately,
//! and not by counting: this project's most expensive failure mode is a search that runs happily
//! and matches nothing, and every one of those starts as a small disagreement nobody looked for.
//!
//! ```text
//! cargo run --release --bin gpuinfo
//! cargo run --release --bin gpuinfo -- --bench
//! ```

use std::time::Instant;

use slasher::adapters::cpu::Cpu;
use slasher::adapters::opencl;
use slasher::ports::{Backend, PeelRequest, PeeledSet, StemBatch, SweepRequest};
use slasher::{feed, hash64, Filter, BASIS, ID_MASK};

fn main() {
    // Started by another process to find out whether opening a device here is survivable. Says
    // nothing on success, because the caller is a program and not a person. See `guard.rs`.
    if opencl::guard::is_probe() {
        std::process::exit(probe());
    }

    println!("{}", opencl::report());

    let device = match opencl::Device::open() {
        Ok(Some(device)) => device,
        Ok(None) => {
            println!("Nothing to check: no device was opened.");
            return;
        }
        Err(why) => {
            println!("A device was found and could not be used:\n\n{why}");
            std::process::exit(1);
        }
    };

    println!("checking {} against the CPU\n", Backend::name(&device));

    let mut failures = 0;

    failures += check("the hash itself", known_vectors(&device));
    failures += check("the forward sweep", sweep_agrees(&device));
    failures += check("the backward peel", peel_agrees(&device));
    failures += check(
        "a stem longer than the register window",
        long_stems(&device),
    );
    failures += check("an empty batch", degenerate(&device));

    if std::env::args().any(|argument| argument == "--bench") {
        bench(&device);
    }

    println!();

    if failures == 0 {
        println!("All checks passed. This device can be trusted with a pass.");
    } else {
        println!(
            "{failures} check(s) failed. Please open an issue with everything printed above -- \
             including the device line at the top, which is the part that identifies what is \
             different about your machine."
        );
        std::process::exit(1);
    }
}

/// Open a device, build the kernel, ask it one question, and exit.
///
/// The exit code is the whole message: zero means a caller may open a device of its own, anything
/// else means it should not. A caller that never gets a code at all -- because a driver took this
/// process down building the kernel -- has learned the most useful thing of the three, and has
/// learned it somewhere it can survive.
///
/// Having no device is success. The question asked is "does using a device here break", and on a
/// machine with none the answer is no.
fn probe() -> i32 {
    match opencl::Device::open() {
        Ok(Some(device)) => match known_vectors(&device) {
            Ok(_) => 0,
            Err(why) => {
                eprintln!("{why}");
                2
            }
        },
        Ok(None) => 0,
        Err(why) => {
            eprintln!("{why}");
            2
        }
    }
}

fn check(what: &str, outcome: Result<String, String>) -> u32 {
    match outcome {
        Ok(note) => {
            println!(
                "  ok    {what}{}{note}",
                if note.is_empty() { "" } else { " -- " }
            );
            0
        }
        Err(why) => {
            println!("  FAIL  {what}\n          {why}");
            1
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The checks

/// Names whose hashes are known, run through the device's own arithmetic.
///
/// The device is asked for them the only way it can be: by making each one the single member of
/// a peeled set and seeing whether the sweep finds it. That exercises the fold, both bitmaps and
/// the binary search in one go, which is the point -- a device that folds correctly and probes
/// wrongly fails here exactly as loudly as one that does neither.
fn known_vectors(device: &opencl::Device) -> Result<String, String> {
    let names = [
        "weapon_ar_standard",
        "maps/mp/mp_apocalypse.d3dbsp",
        "ui/uieditor/menus/hud/hud_main.lua",
        "c",
        "abcdefghijklmnopqrstuvwxyz0123456789",
    ];

    for name in names {
        let wanted = hash64(name);
        let stems = StemBatch::pack(&[name], true);
        let table = vec![wanted];
        let filter = Filter::sized(table.iter(), table.len());
        let (coarse, fine, coarse_bits, fine_bits) = filter.parts();

        let request = SweepRequest {
            stems: &stems,
            openings: &[],
            bare: true,
            peeled: PeeledSet {
                hashes: &table,
                coarse,
                fine,
                coarse_bits,
                fine_bits,
            },
        };

        let hits = device.sweep(&request).map_err(|why| why.to_string())?;

        if hits.len() != 1 {
            return Err(format!(
                "{name:?} hashes to {wanted:016x}, and the device reported {} hit(s) instead of 1",
                hits.len(),
            ));
        }
    }

    Ok(format!("{} known names", names.len()))
}

/// The forward sweep, on a batch big enough to have hits by accident rather than by design.
///
/// The peeled set is seeded so that a known fraction of candidates land in it. A test where every
/// candidate hits proves the fold and nothing else; a test where none do proves nothing at all
/// and passes on a device that returns an empty answer for every question -- which is exactly
/// what a broken driver does.
fn sweep_agrees(device: &opencl::Device) -> Result<String, String> {
    let openings: Vec<String> = (0..170).map(|n| format!("weapons/tier{n}/")).collect();
    let stems: Vec<String> = (0..20_000).map(|n| format!("part_{n:05}_body")).collect();

    // Every twenty-fifth candidate is planted, so both sides have several hundred hits to get
    // wrong, and their indices are spread across the whole batch rather than bunched at the front.
    let mut wanted: Vec<u64> = Vec::new();

    for (index, stem) in stems.iter().enumerate() {
        if index % 25 == 0 {
            let opening = index % openings.len();
            wanted.push(feed(hash64(&openings[opening]), stem.as_bytes()));
        }

        if index % 400 == 0 {
            wanted.push(hash64(stem));
        }
    }

    wanted.sort_unstable();
    wanted.dedup();

    let states: Vec<u64> = openings.iter().map(|opening| hash64(opening)).collect();
    let packed = StemBatch::pack(&stems, true);
    let filter = Filter::sized(wanted.iter(), wanted.len());
    let (coarse, fine, coarse_bits, fine_bits) = filter.parts();

    let request = SweepRequest {
        stems: &packed,
        openings: &states,
        bare: true,
        peeled: PeeledSet {
            hashes: &wanted,
            coarse,
            fine,
            coarse_bits,
            fine_bits,
        },
    };

    let mut theirs = device.sweep(&request).map_err(|why| why.to_string())?;
    let mut ours = Cpu::new().sweep(&request).map_err(|why| why.to_string())?;

    // Order is the one thing that legitimately differs: the device appends hits in whatever order
    // its groups finish, which is not deterministic and does not need to be. Everything else must
    // match exactly.
    theirs.sort_unstable();
    ours.sort_unstable();

    if theirs == ours {
        return Ok(format!(
            "{} candidates, {} hits, identical",
            request.forward(),
            ours.len(),
        ));
    }

    Err(disagreement(&ours, &theirs))
}

/// The backward peel, compared element for element in the layout both sides write.
fn peel_agrees(device: &opencl::Device) -> Result<String, String> {
    let endings: Vec<String> = (0..400).map(|n| format!("_variant{n:03}.xmodel")).collect();
    let packed = StemBatch::pack(&endings, true);

    let spellings: Vec<u64> = (0..5_000_u64)
        .flat_map(|n| {
            let id = hash64(&format!("some/asset/{n}")) & ID_MASK;
            [id, id | !ID_MASK]
        })
        .collect();

    let request = PeelRequest {
        spellings: &spellings,
        endings: &packed,
        no_ending: true,
    };

    let theirs = device.peel(&request).map_err(|why| why.to_string())?;
    let ours = Cpu::new().peel(&request).map_err(|why| why.to_string())?;

    if theirs.len() != ours.len() {
        return Err(format!(
            "the device returned {} entries, the CPU {}",
            theirs.len(),
            ours.len()
        ));
    }

    if let Some(at) = theirs.iter().zip(&ours).position(|(a, b)| a != b) {
        return Err(format!(
            "first disagreement at entry {at} of {}: device {:016x}, CPU {:016x}",
            ours.len(),
            theirs[at],
            ours[at],
        ));
    }

    Ok(format!("{} peeled entries, identical", ours.len()))
}

/// Stems past the thirty-two byte window, which take the kernel's slow path.
///
/// Its own check because it is the one branch in the kernel that a realistic batch may never
/// reach, and a bug there would surface as a handful of names quietly missing from a pass rather
/// than as anything that looks wrong.
fn long_stems(device: &opencl::Device) -> Result<String, String> {
    let stems: Vec<String> = (28..96)
        .map(|len| {
            let mut stem = format!("{len:03}_");
            while stem.len() < len {
                stem.push((b'a' + (stem.len() % 26) as u8) as char);
            }
            stem
        })
        .collect();

    let openings = ["", "zombie/", "maps/mp/gametypes/"];
    let states: Vec<u64> = openings.iter().map(|opening| hash64(opening)).collect();

    let mut wanted: Vec<u64> = stems
        .iter()
        .enumerate()
        .filter(|(index, _)| index % 3 == 0)
        .map(|(index, stem)| feed(states[index % states.len()], stem.as_bytes()))
        .collect();

    wanted.sort_unstable();
    wanted.dedup();

    let packed = StemBatch::pack(&stems, true);
    let filter = Filter::sized(wanted.iter(), wanted.len());
    let (coarse, fine, coarse_bits, fine_bits) = filter.parts();

    let request = SweepRequest {
        stems: &packed,
        openings: &states,
        bare: true,
        peeled: PeeledSet {
            hashes: &wanted,
            coarse,
            fine,
            coarse_bits,
            fine_bits,
        },
    };

    let mut theirs = device.sweep(&request).map_err(|why| why.to_string())?;
    let mut ours = Cpu::new().sweep(&request).map_err(|why| why.to_string())?;

    theirs.sort_unstable();
    ours.sort_unstable();

    if theirs == ours {
        Ok(format!(
            "lengths {} to {}, identical",
            packed.lengths[0],
            packed.longest()
        ))
    } else {
        Err(disagreement(&ours, &theirs))
    }
}

/// The shapes a real pass produces at its edges: no stems, no beginnings, nothing wanted.
///
/// A driver that returns something plausible for an empty launch and a driver that faults on one
/// are both common, and neither is worth discovering in hour three of a grind.
fn degenerate(device: &opencl::Device) -> Result<String, String> {
    let empty: [&str; 0] = [];
    let packed = StemBatch::pack(&empty, true);
    let table: Vec<u64> = Vec::new();
    let filter = Filter::sized(table.iter(), 1);
    let (coarse, fine, coarse_bits, fine_bits) = filter.parts();

    let request = SweepRequest {
        stems: &packed,
        openings: &[],
        bare: true,
        peeled: PeeledSet {
            hashes: &table,
            coarse,
            fine,
            coarse_bits,
            fine_bits,
        },
    };

    if !device
        .sweep(&request)
        .map_err(|why| why.to_string())?
        .is_empty()
    {
        return Err("an empty batch produced hits".to_owned());
    }

    // One stem, no beginnings at all, and the bare candidate turned off: nothing to ask.
    let one = StemBatch::pack(&["solo"], true);
    let table = vec![hash64("solo")];
    let filter = Filter::sized(table.iter(), table.len());
    let (coarse, fine, coarse_bits, fine_bits) = filter.parts();

    let request = SweepRequest {
        stems: &one,
        openings: &[],
        bare: false,
        peeled: PeeledSet {
            hashes: &table,
            coarse,
            fine,
            coarse_bits,
            fine_bits,
        },
    };

    if !device
        .sweep(&request)
        .map_err(|why| why.to_string())?
        .is_empty()
    {
        return Err("a batch with nothing to ask produced hits".to_owned());
    }

    Ok(String::new())
}

fn disagreement(ours: &[u64], theirs: &[u64]) -> String {
    let only_cpu: Vec<&u64> = ours
        .iter()
        .filter(|hit| !theirs.contains(hit))
        .take(3)
        .collect();
    let only_gpu: Vec<&u64> = theirs
        .iter()
        .filter(|hit| !ours.contains(hit))
        .take(3)
        .collect();

    format!(
        "the CPU found {} and the device {}.\n          \
         missed by the device: {:?}\n          \
         invented by the device: {:?}\n          \
         (each is a stem index in the high word and a beginning index in the low word)",
        ours.len(),
        theirs.len(),
        only_cpu,
        only_gpu,
    )
}

// ---------------------------------------------------------------------------------------------

/// What the device is worth, against the threads it would replace.
///
/// Both sides are given the same work and timed end to end, transfers included. Timing the kernel
/// alone would flatter the device by hiding the upload, and the upload is real: a peeled batch is
/// hundreds of megabytes and crosses the bus once per batch.
fn bench(device: &opencl::Device) {
    println!("\ntiming, same work both sides, transfers included\n");

    let openings: Vec<u64> = (0..170)
        .map(|n| hash64(&format!("weapons/tier{n}/")))
        .collect();
    let stems: Vec<String> = (0..2_000_000)
        .map(|n| format!("part_{n:07}_body"))
        .collect();
    let packed = StemBatch::pack(&stems, true);

    // Sized like a real peeled batch rather than like a test. It is the peeled set that decides
    // what this kernel costs: twenty million entries is 160 MiB, far past any cache, so every
    // candidate that clears both bitmaps pays for a binary search through device memory. A bench
    // over a small table measures the fold, which is the cheap part and not the question.
    let wanted: Vec<u64> = {
        // Multiplied rather than hashed: these only have to be well spread and sorted, and
        // twenty million `format!` calls would make building the bench slower than running it.
        let mut seeded: Vec<u64> = (0..20_000_000_u64)
            .map(|n| n.wrapping_mul(0x9E37_79B9_7F4A_7C15))
            .collect();
        seeded.sort_unstable();
        seeded.dedup();
        seeded
    };

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

    let candidates = request.forward();

    let started = Instant::now();
    let _ = device.sweep(&request);
    let on_device = started.elapsed().as_secs_f64();

    let started = Instant::now();
    let _ = Cpu::new().sweep(&request);
    let on_cpu = started.elapsed().as_secs_f64();

    println!(
        "  {candidates} candidates ({:.2}B)",
        candidates as f64 / 1e9
    );
    println!(
        "  device  {on_device:>7.2}s   {:>8.2}M/s",
        candidates as f64 / on_device / 1e6
    );
    println!(
        "  CPU     {on_cpu:>7.2}s   {:>8.2}M/s",
        candidates as f64 / on_cpu / 1e6
    );
    println!("  {:.1}x", on_cpu / on_device);

    let _ = BASIS;
}
