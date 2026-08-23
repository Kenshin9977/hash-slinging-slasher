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

use std::process::{Command, Stdio};
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

    // One rung, or one check, started by the run below. Both say what happened through their exit
    // code, because what starts them is a program and not a person.
    if let Some(rung) = numbered("--rung") {
        std::process::exit(run_one_rung(rung));
    }

    if let Some(index) = numbered("--check") {
        std::process::exit(run_one_check(index));
    }

    let mut report = Report::open();

    report.say(&preamble());
    report.say(&opencl::report());

    match opencl::Device::open() {
        Ok(Some(_)) => {}
        Ok(None) => {
            report.say("\nNothing to check: no device was opened.\n");
            report.finish(0);
        }
        Err(why) => {
            report.say(&format!(
                "\nA device was found and could not be used:\n\n{why}\n"
            ));
            report.finish(1);
        }
    }

    // The ladder first. When something is wrong this is what says *what*, and it says it before
    // the full checks get a chance to fail in a way that needs interpreting.
    let mut failures = climb(&mut report);
    failures += full_checks(&mut report);

    if std::env::args().any(|argument| argument == "--bench") {
        // In this process: it is a measurement rather than a check, and by the time it runs the
        // device has already been proved survivable by everything above.
        match opencl::Device::open() {
            Ok(Some(device)) => report.say(&bench(&device)),
            _ => report.say("\n(no device to time)\n"),
        }
    }

    if failures == 0 {
        report.say("\nEverything passed. This device can be trusted with a pass.\n");
        report.finish(0);
    }

    report.say(&format!(
        "\n{failures} thing(s) failed. The lowest failing rung is the specific one; anything \
         below it is a consequence of it.\n"
    ));

    report.finish(1);
}

/// The file a person is asked to send, and the reason this program writes one at all.
///
/// Somebody with a Radeon is doing us a favour and gets one run. Scrollback gets truncated, gets
/// pasted without the device line at the top, gets reformatted by a chat client. A file does not.
const REPORT: &str = "gpuinfo-report.txt";

/// The report, written as it happens rather than at the end.
///
/// Buffering it would be tidier and was tried, and it is wrong for exactly the reason this whole
/// program exists: a driver that takes the process down takes the buffer with it, and what reaches
/// the person helping is a segmentation fault and an empty screen. Every line is printed and
/// flushed to disk as it is produced, so a run that dies halfway still leaves a report that says
/// where it got to.
struct Report {
    file: Option<std::fs::File>,
}

impl Report {
    fn open() -> Self {
        Self {
            file: std::fs::File::create(REPORT).ok(),
        }
    }

    fn say(&mut self, text: &str) {
        print!("{text}");
        let _ = std::io::Write::flush(&mut std::io::stdout());

        if let Some(file) = &mut self.file {
            let _ = std::io::Write::write_all(file, text.as_bytes());
            let _ = std::io::Write::flush(file);
        }
    }

    fn finish(&mut self, code: i32) -> ! {
        if self.file.is_none() {
            println!("\n(could not write {REPORT}; the text above is the whole report)");
        } else if code == 0 {
            println!("\n(also saved to {REPORT})");
        } else {
            println!(
                "\nPlease send {REPORT}. It has everything needed to work out what is wrong with \
                 this device, so that nobody has to ask you to go and try something else."
            );
        }

        std::process::exit(code)
    }
}

/// What the report opens with: the things that are true of the machine rather than the device.
fn preamble() -> String {
    format!(
        "hash-slinging-slasher GPU report\n\
         {}\n\
         built for {} on {}\n\n",
        std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| "?".to_owned()),
        std::env::consts::ARCH,
        std::env::consts::OS,
    )
}

// ---------------------------------------------------------------------------------------------
// The capability ladder

/// The number after a flag, if that flag was given.
fn numbered(flag: &str) -> Option<usize> {
    let mut args = std::env::args();

    while let Some(argument) = args.next() {
        if argument == flag {
            return args.next().and_then(|n| n.parse().ok());
        }
    }

    None
}

/// What a child process said about the one thing it was asked to do.
struct Outcome {
    /// A short word for the column: `ok`, `WRONG`, `CRASHED`.
    verdict: String,
    /// Whatever the child had to say, whether it succeeded or not.
    detail: String,
    /// Whether this counts against the device.
    bad: bool,
}

/// Runs this program again, for one numbered piece of work, and interprets how it ended.
///
/// The whole design rests on this: a child that is killed rather than exiting has told us
/// something no return value could, and it has told us without taking the report with it.
fn ask_a_child(flag: &str, index: usize) -> Outcome {
    let Ok(me) = std::env::current_exe() else {
        return Outcome {
            verdict: "skipped".to_owned(),
            detail: "cannot find this program on disk to run it again".to_owned(),
            bad: false,
        };
    };

    let mut command = Command::new(&me);

    command
        .arg(flag)
        .arg(index.to_string())
        .env(opencl::guard::PROBED, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // The child has to work at the same size as the parent was asked to, or a report taken with
    // `--small` would quietly be a report of something else.
    if std::env::args().any(|argument| argument == "--small") {
        command.arg("--small");
    }

    match command.output() {
        Err(why) => Outcome {
            verdict: "skipped".to_owned(),
            detail: format!("could not start it: {why}"),
            bad: false,
        },
        Ok(done) => {
            let said = String::from_utf8_lossy(&done.stderr);
            let noted = String::from_utf8_lossy(&done.stdout);

            let detail = if said.trim().is_empty() {
                noted.trim().to_owned()
            } else {
                said.trim().to_owned()
            };

            match done.status.code() {
                Some(0) => Outcome {
                    verdict: "ok".to_owned(),
                    detail,
                    bad: false,
                },
                Some(1) => Outcome {
                    verdict: "WRONG".to_owned(),
                    detail,
                    bad: true,
                },
                Some(2) => Outcome {
                    verdict: "no device".to_owned(),
                    detail,
                    bad: false,
                },
                Some(3) => Outcome {
                    verdict: "skipped".to_owned(),
                    detail,
                    bad: false,
                },
                Some(code) => Outcome {
                    verdict: format!("exit {code}"),
                    detail,
                    bad: true,
                },
                // Killed rather than having exited: the driver took the process down. This is the
                // case the whole arrangement exists for.
                None => Outcome {
                    verdict: "CRASHED".to_owned(),
                    detail: "the driver took the process down".to_owned(),
                    bad: true,
                },
            }
        }
    }
}

/// One rung, in this process, reporting through the exit code and stderr.
fn run_one_rung(rung: usize) -> i32 {
    if rung >= opencl::ladder::RUNGS.len() {
        return 3;
    }

    match opencl::Device::open() {
        Ok(Some(device)) => match opencl::ladder::run(&device, rung) {
            Ok(()) => 0,
            Err(why) => {
                eprintln!("{why}");
                1
            }
        },
        Ok(None) => 3,
        Err(why) => {
            eprintln!("{why}");
            2
        }
    }
}

/// Climbs the ladder, each rung in a process of its own, writing as it goes.
///
/// Separate processes because a driver that faults takes its process with it, and a ladder that
/// stopped at the first fault would lose exactly the rungs that say how far the damage reaches. A
/// rung that dies is recorded as having died, and the next one still runs.
fn climb(report: &mut Report) -> u32 {
    report.say(
        "\ncapability ladder -- one thing per rung, lowest first, each in its own process\n\n",
    );

    let mut failures = 0;
    let mut first_bad: Option<usize> = None;

    for (index, rung) in opencl::ladder::RUNGS.iter().enumerate() {
        let outcome = ask_a_child("--rung", index);

        if outcome.bad {
            failures += 1;
            first_bad.get_or_insert(index);
        }

        report.say(&format!(
            "  {:>2}  {:<9} {}\n",
            index + 1,
            outcome.verdict,
            rung.what
        ));

        if !outcome.detail.is_empty() {
            report.say(&format!("      {}\n", outcome.detail));
        }

        if outcome.bad {
            report.say(&format!("      what that means: {}\n", rung.so_what));
        }
    }

    if let Some(index) = first_bad {
        report.say(&format!(
            "\n  Lowest rung that failed: {} -- {}.\n  {}\n",
            index + 1,
            opencl::ladder::RUNGS[index].what,
            opencl::ladder::RUNGS[index].so_what,
        ));
    }

    failures
}

// ---------------------------------------------------------------------------------------------
// The full checks, run the same way and for the same reason

type Check = fn(&opencl::Device) -> Result<String, String>;

/// Every full check, in the order that makes a failure easiest to read.
const CHECKS: &[(&str, Check)] = &[
    ("the hash itself", known_vectors),
    ("the forward sweep", sweep_agrees),
    ("the backward peel", peel_agrees),
    ("a stem longer than the register window", long_stems),
    ("an empty batch", degenerate),
];

/// One check, in this process, saying what happened through its exit code.
fn run_one_check(index: usize) -> i32 {
    let Some((_, check)) = CHECKS.get(index) else {
        return 3;
    };

    match opencl::Device::open() {
        Ok(Some(device)) => match check(&device) {
            Ok(note) => {
                // On stdout rather than stderr, so the parent can tell a note apart from a reason.
                print!("{note}");
                0
            }
            Err(why) => {
                eprintln!("{why}");
                1
            }
        },
        Ok(None) => 3,
        Err(why) => {
            eprintln!("{why}");
            2
        }
    }
}

/// Runs every full check, each in its own process.
fn full_checks(report: &mut Report) -> u32 {
    report.say("\nfull checks, against the CPU on the same bytes\n\n");

    let mut failures = 0;

    for (index, (name, _)) in CHECKS.iter().enumerate() {
        let outcome = ask_a_child("--check", index);

        if outcome.bad {
            failures += 1;
        }

        report.say(&format!("  {:<9} {name}\n", outcome.verdict));

        if !outcome.detail.is_empty() {
            report.say(&format!("      {}\n", outcome.detail));
        }
    }

    failures
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

/// How much work each check does.
///
/// A simulator that bounds-checks every memory access runs roughly a hundred times slower than
/// the processor under it, so the sizes that make a real device sweat make Oclgrind take an hour.
/// `--small` is what continuous integration passes: the same checks, the same code paths, few
/// enough candidates to finish. It is not a weaker check -- a wrong index is wrong at forty
/// spellings exactly as it is at ten thousand -- it is a shorter one.
fn scale() -> usize {
    if std::env::args().any(|argument| argument == "--small") {
        1
    } else {
        100
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
    let scale = scale();
    let openings: Vec<String> = (0..(2 * scale).min(170).max(6))
        .map(|n| format!("weapons/tier{n}/"))
        .collect();
    let stems: Vec<String> = (0..200 * scale)
        .map(|n| format!("part_{n:05}_body"))
        .collect();

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
    let scale = scale();
    let endings: Vec<String> = (0..(4 * scale).min(400))
        .map(|n| format!("_variant{n:03}.xmodel"))
        .collect();
    let packed = StemBatch::pack(&endings, true);

    let spellings: Vec<u64> = (0..50 * scale as u64)
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
fn bench(device: &opencl::Device) -> String {
    let mut out = String::from("\ntiming, same work both sides, transfers included\n\n");

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

    out.push_str(&format!(
        "  {candidates} candidates ({:.2}B)\n",
        candidates as f64 / 1e9
    ));
    out.push_str(&format!(
        "  device  {on_device:>7.2}s   {:>8.2}M/s\n",
        candidates as f64 / on_device / 1e6
    ));
    out.push_str(&format!(
        "  CPU     {on_cpu:>7.2}s   {:>8.2}M/s\n",
        candidates as f64 / on_cpu / 1e6
    ));
    out.push_str(&format!("  {:.1}x\n", on_cpu / on_device));

    let _ = BASIS;

    out
}
