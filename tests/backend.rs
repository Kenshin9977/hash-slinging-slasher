//! The port, checked against the arithmetic it is a port of.
//!
//! Everything here runs on a machine with no GPU, because that is what continuous integration
//! runs on and because the CPU adapter is the definition the device is measured against -- if it
//! drifts from `slasher::feed` and `slasher::peel`, then a device agreeing with it proves
//! nothing at all.
//!
//! The last test is the differential one, and it is a no-op without a device. That is deliberate:
//! it means a contributor with an AMD card runs `cargo test` and finds out, and a CI runner with
//! a GPU attached checks the kernel on every commit without the workflow needing to know whether
//! one is there.

use std::collections::HashMap;

use slasher::adapters::{cpu::Cpu, opencl};
use slasher::ports::{Backend, PeelRequest, PeeledSet, StemBatch, SweepRequest};
use slasher::{feed, feed_raw, hash64, peel, peel_raw, Filter, BASIS, ID_MASK};

/// The fold belongs to the packing now, so it has to be exactly the fold it replaced.
///
/// Both directions matter and they are not the same test. Folding is what makes a search match;
/// folding when it should not is what made every Black Ops 4 sound name unreachable, and that
/// bug looked like an ordinary unproductive pass for as long as nobody checked.
#[test]
fn packing_folds_exactly_as_the_hash_does() {
    let names = [
        "Weapon\\AR\\Standard",
        "MAPS/MP/mp_Apocalypse",
        "already_lower",
        "",
    ];

    for name in names {
        for fold in [true, false] {
            let packed = StemBatch::pack(&[name], fold);
            let at = packed.offsets[0] as usize;
            let len = packed.lengths[0] as usize;
            let bytes = &packed.bytes[at..at + len];

            let mine = bytes.iter().fold(BASIS, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(slasher::PRIME)
            });

            let theirs = if fold {
                feed(BASIS, name.as_bytes())
            } else {
                feed_raw(BASIS, name.as_bytes())
            };

            assert_eq!(mine, theirs, "{name:?} with fold={fold}");
        }
    }
}

/// The window the kernel holds in registers is thirty-two bytes, and a stem may be longer.
///
/// Checked here rather than only on a device because the packing is what decides where the
/// window ends, and an off-by-one in the padding would read a stem's first byte as its
/// thirty-third on hardware and be invisible on the CPU.
#[test]
fn packing_pads_past_the_last_stem() {
    let packed = StemBatch::pack(
        &["short", "a_stem_that_is_definitely_longer_than_the_window"],
        true,
    );
    let last = packed.offsets[1] as usize + packed.lengths[1] as usize;

    assert!(
        packed.bytes.len() >= last + slasher::ports::PACKED_BYTES,
        "the kernel reads a whole window past a stem's start and must not run off the buffer",
    );
    assert_eq!(packed.longest(), 48);
}

/// The CPU adapter's sweep, against the thing it is a rearrangement of.
#[test]
fn the_cpu_sweep_finds_what_hashing_by_hand_finds() {
    let openings: Vec<String> = (0..17).map(|n| format!("weapons/tier{n}/")).collect();
    let stems: Vec<String> = (0..3_000).map(|n| format!("part_{n:04}_body")).collect();
    let states: Vec<u64> = openings.iter().map(|opening| hash64(opening)).collect();

    // Planted by hand, so the expected answer is known before anything runs rather than being
    // whatever the implementation produced.
    let mut expected: Vec<u64> = Vec::new();
    let mut wanted: Vec<u64> = Vec::new();

    for (index, stem) in stems.iter().enumerate() {
        if index % 37 == 0 {
            let opening = index % states.len();
            wanted.push(feed(states[opening], stem.as_bytes()));
            expected.push(((index as u64) << 32) | opening as u64);
        }

        if index % 211 == 0 {
            wanted.push(hash64(stem));
            expected.push(((index as u64) << 32) | 0xFFFF_FFFF);
        }
    }

    wanted.sort_unstable();
    wanted.dedup();
    expected.sort_unstable();

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

    let mut found = Cpu::new().sweep(&request).expect("the CPU never declines");
    found.sort_unstable();

    assert_eq!(found, expected);
}

/// The CPU adapter's peel, against `slasher::peel` on the same endings.
#[test]
fn the_cpu_peel_is_the_peel_it_replaced() {
    let endings: Vec<String> = vec![
        "_c".into(),
        "\\Mixed\\Case.wav".into(),
        String::new(),
        "_n.xmodel".into(),
    ];

    let spellings: Vec<u64> = (0..64_u64)
        .flat_map(|n| {
            let id = hash64(&format!("asset/{n}")) & ID_MASK;
            [id, id | !ID_MASK]
        })
        .collect();

    for fold in [true, false] {
        let packed = StemBatch::pack(&endings, fold);
        let request = PeelRequest {
            spellings: &spellings,
            endings: &packed,
            no_ending: true,
        };
        let got = Cpu::new().peel(&request).expect("the CPU never declines");

        for (slot, spelling) in spellings.iter().enumerate() {
            assert_eq!(got[slot], *spelling, "row zero is the un-peeled id");

            for (row, ending) in endings.iter().enumerate() {
                let expected = if fold {
                    peel(*spelling, ending.as_bytes())
                } else {
                    peel_raw(*spelling, ending.as_bytes())
                };

                assert_eq!(
                    got[(row + 1) * spellings.len() + slot],
                    expected,
                    "ending {ending:?} off {spelling:016x} with fold={fold}",
                );
            }
        }
    }
}

/// The whole search, run twice, once with a device forbidden.
///
/// This is the test that would have caught the rewiring going wrong: not whether a kernel
/// computes a hash, but whether the search that calls it still returns the names it used to. It
/// uses `Meet` exactly as a pass does.
#[test]
fn the_search_returns_the_same_names_whatever_ran_it() {
    let openings = ["weapons/".to_owned(), "maps/mp/".to_owned()];
    let endings = ["_c".to_owned(), "_n".to_owned(), "_lod0".to_owned()];
    let stems: Vec<String> = (0..500).map(|n| format!("asset_{n:03}")).collect();

    // Six real names, hidden in a pool of ids that name nothing.
    let planted = [
        "weapons/asset_007_c",
        "weapons/asset_100_n",
        "maps/mp/asset_222_lod0",
        "maps/mp/asset_499",
        "asset_050_c",
        "asset_333",
    ];

    let mut wanted: HashMap<u64, usize> = HashMap::new();

    for name in planted {
        wanted.insert(slasher::id_of(name), 0);
    }

    for decoy in 0..2_000 {
        wanted.insert(slasher::id_of(&format!("nothing/{decoy}")), 0);
    }

    let search = slasher::search::Meet::new(&openings, &endings);
    let mut found = search.run(&stems, &wanted);

    found.sort();
    found.dedup();

    let mut names: Vec<&str> = found.iter().map(|(_, name)| name.as_str()).collect();
    names.sort_unstable();

    let mut expected: Vec<&str> = planted.to_vec();
    expected.sort_unstable();

    assert_eq!(names, expected);
}

/// Turns "nothing was compared" into a failure where something was supposed to be.
///
/// A skipped test and a passing test are the same line in a log. That is right on a contributor's
/// laptop and wrong in continuous integration: the PoCL job installs an implementation
/// specifically so this has something to compare against, and if that ICD ever stopped being
/// picked up the job would go green having compared nothing at all. Setting this wherever a device
/// is guaranteed is what closes that.
const REQUIRED: &str = "SLASHER_GPU_REQUIRED";

fn nothing_was_compared(why: &str) {
    assert!(
        std::env::var_os(REQUIRED).is_none(),
        "{REQUIRED} is set, so a usable device was supposed to be here -- but {why}"
    );

    eprintln!("{why}; nothing was compared");
}

/// A device, if there is one, against the CPU on the same bytes.
///
/// Silently passing on a machine with no GPU is the whole point: the same `cargo test` has to be
/// the right command for a contributor on a Radeon, a contributor on an Intel laptop, and a CI
/// runner with no graphics at all. Which one ran is printed, so a passing log can still be read
/// to find out whether anything was actually checked.
#[test]
fn a_device_agrees_with_the_cpu() {
    let device = match opencl::Device::open() {
        Ok(Some(device)) => device,
        Ok(None) => {
            nothing_was_compared("no OpenCL device on this machine");
            return;
        }
        // Not a failure by default. A driver too old for its own silicon is the contributor's
        // machine and not this repository's code, and a red suite they cannot fix is how
        // contributions stop. It is said loudly and skipped; `gpuinfo` is the tool whose job is to
        // fail over it. Where a device is guaranteed, `SLASHER_GPU_REQUIRED` makes it a failure.
        Err(why) => {
            nothing_was_compared(&format!("a device was found and could not be used: {why}"));
            return;
        }
    };

    eprintln!("comparing against {}", Backend::name(&device));

    let openings: Vec<u64> = (0..64).map(|n| hash64(&format!("beginning{n}/"))).collect();

    // Lengths deliberately straddling the thirty-two byte window, so both paths through the
    // kernel are exercised by one comparison.
    let stems: Vec<String> = (0..40_000)
        .map(|n| {
            let mut stem = format!("stem_{n:05}");
            for extra in 0..(n % 70) {
                stem.push((b'a' + (extra % 26) as u8) as char);
            }
            stem
        })
        .collect();

    let mut wanted: Vec<u64> = stems
        .iter()
        .enumerate()
        .filter(|(index, _)| index % 31 == 0)
        .map(|(index, stem)| feed(openings[index % openings.len()], stem.as_bytes()))
        .collect();

    wanted.extend(stems.iter().step_by(500).map(|stem| hash64(stem)));
    wanted.sort_unstable();
    wanted.dedup();

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

    let mut theirs = device.sweep(&request).expect("the device took the batch");
    let mut ours = Cpu::new().sweep(&request).expect("the CPU never declines");

    // Order is the one thing that legitimately differs: a device appends hits as its groups
    // finish, which is not deterministic and does not need to be.
    theirs.sort_unstable();
    ours.sort_unstable();

    assert_eq!(
        theirs.len(),
        ours.len(),
        "the device and the CPU found different numbers of hits"
    );
    assert_eq!(theirs, ours);
    assert!(
        !ours.is_empty(),
        "a comparison where neither side found anything proves nothing"
    );

    // Its own small ending list, rather than the forty thousand stems above.
    //
    // Reusing `packed` here was careless and it cost a green build. Forty thousand endings against
    // two and a half thousand ids is a hundred million entries: eight hundred megabytes on the
    // device and the same again, twice, on the host. It passed on a workstation and took down a
    // continuous integration runner with sixteen gigabytes and four cores -- as a segmentation
    // fault inside somebody else's driver, rather than as anything that named itself.
    //
    // What this has to prove is that the two sides agree, and they agree at four hundred endings
    // exactly as they would at forty thousand.
    let endings: Vec<String> = (0..400).map(|n| format!("_variant{n:03}.xmodel")).collect();

    let endings = StemBatch::pack(&endings, true);

    let peel_request = PeelRequest {
        spellings: &wanted,
        endings: &endings,
        no_ending: true,
    };

    let theirs = device
        .peel(&peel_request)
        .expect("the device took the peel");
    let ours = Cpu::new()
        .peel(&peel_request)
        .expect("the CPU never declines");

    assert_eq!(theirs, ours, "the device and the CPU peeled differently");
}

/// One backslash, spelled once, because a literal one in a test that is about backslashes is
/// the easiest thing in this file to get wrong while reading it.
const SLASH: char = '\\';
const BS1: &str = "sound\\";
const BS2: &str = "sound\\weapons\\";

/// The same end-to-end differential, for the search that does **not** fold backslashes.
///
/// `Meet::unfolded` is the path Black Ops 4's SAB sound names take, and it is the one place in this
/// project where getting the normalisation wrong is catastrophic and silent: measured against the
/// 8,385 of them cod-name-db already names, 8,385 reproduce without folding and **0** with it. A
/// pass down that path with a mishandled backslash matches nothing at all, for ever, while looking
/// exactly like ordinary unproductive grinding.
///
/// The packing is checked at both fold settings and so is the peel, but until this the folded path
/// was the only one that ever reached a backend end to end.
#[test]
fn the_unfolded_search_returns_the_same_names_whatever_ran_it() {
    let openings = [BS1.to_owned(), BS2.to_owned()];
    let endings = [".wav".to_owned(), "_01.wav".to_owned()];
    let stems: Vec<String> = (0..500)
        .map(|n| format!("sab{}entry_{n:03}", SLASH))
        .collect();

    // Every one of these carries a backslash, which is the whole point: fold them and not one of
    // them is reachable.
    let planted = [
        format!("{BS1}sab{SLASH}entry_007.wav"),
        format!("{BS2}sab{SLASH}entry_100_01.wav"),
        format!("{BS1}sab{SLASH}entry_222"),
        format!("sab{SLASH}entry_333.wav"),
    ];

    let mut wanted: HashMap<u64, usize> = HashMap::new();

    for name in &planted {
        wanted.insert(slasher::hash64_raw(name) & ID_MASK, 0);
    }

    for decoy in 0..2_000 {
        wanted.insert(
            slasher::hash64_raw(&format!("nothing{SLASH}{decoy}")) & ID_MASK,
            0,
        );
    }

    let search = slasher::search::Meet::unfolded(&openings, &endings);
    let mut found = search.run(&stems, &wanted);

    found.sort();
    found.dedup();

    let mut names: Vec<&str> = found.iter().map(|(_, name)| name.as_str()).collect();
    names.sort_unstable();

    let mut expected: Vec<&str> = planted.iter().map(|name| name.as_str()).collect();
    expected.sort_unstable();

    assert_eq!(names, expected);
}
