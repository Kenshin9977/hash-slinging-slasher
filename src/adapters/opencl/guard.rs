//! Finding out whether the driver survives being asked to build something, in a process that is
//! allowed to die.
//!
//! # Why this exists
//!
//! Ubuntu 24.04 ships Intel's compute runtime from November 2023. Handed an Arrow Lake iGPU,
//! which is a year newer than it, its compiler segmentation-faults building a sixty-three byte
//! kernel that does nothing but write a one. Not this kernel -- *any* kernel. Measured on
//! 2026-08-23 against NEO 23.43.027642; the current 26.31 builds both that kernel and this one
//! without complaint.
//!
//! A driver being broken is not the interesting part. This is: **the same call that returns a
//! clean `CL_BUILD_PROGRAM_FAILURE` to a C program kills a Rust one.** Intel's compiler installs
//! a handler to catch its own faults and turn them into an error code, and in a Rust process it
//! does not get to. So a C tool prints "the driver refused to build the kernel" and carries on,
//! and this one dies at startup with no message at all.
//!
//! That is the worst possible failure for what this program is. `AGENTS.md` promises one command
//! that grinds for hours while nobody is watching. A binary that segmentation-faults the moment
//! it is started, on a laptop whose owner did nothing wrong, does not fail that promise politely.
//!
//! # What it does
//!
//! Before the real process opens a device, it asks `gpuinfo` to open one first, in a process of
//! its own. If that process dies, the device is declared unusable, the reason is printed once,
//! and the grind proceeds on the CPU exactly as it would on a machine with no GPU at all.
//!
//! A subprocess rather than a signal handler, and a sibling binary rather than a fork:
//!
//! - Catching the fault in-process means running Rust code on a stack a foreign compiler has
//!   already corrupted, which is not something to do in a program that will report findings.
//! - `fork` would work on Unix and does not exist on Windows, and this has to protect a Windows
//!   contributor as much as a Linux one.
//! - Re-running *this* binary would mean `start` doing its network preflight twice, so the child
//!   is `gpuinfo`, whose whole job is already to open a device and say what happened.
//!
//! If `gpuinfo` is not next to the running binary -- somebody copied one file out of `bin/` --
//! the check is skipped rather than treated as a failure. A missing probe is not evidence of a
//! bad driver.

use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Set on the child, so that the device it opens does not go looking for a child of its own.
pub const PROBED: &str = "SLASHER_GPU_PROBED";

/// The argument `gpuinfo` takes to mean "open a device, build the kernel, say nothing, exit".
pub const PROBE_FLAG: &str = "--probe";

/// Whether this process is the probe.
pub fn is_probe() -> bool {
    std::env::args().any(|argument| argument == PROBE_FLAG)
}

/// Whether a driver survived building the kernel.
///
/// `Ok(())` also covers "there was no way to check", which is the right answer: the alternative
/// is refusing to use a working GPU because a helper binary was not shipped alongside.
pub fn survives_a_build() -> Result<(), String> {
    // The child must not recurse, and neither must a caller who has already been checked.
    if std::env::var_os(PROBED).is_some() || is_probe() {
        return Ok(());
    }

    let Some(probe) = sibling_gpuinfo() else {
        return Ok(());
    };

    let outcome = Command::new(&probe)
        .arg(PROBE_FLAG)
        .env(PROBED, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    let outcome = match outcome {
        Ok(outcome) => outcome,
        // Could not run it at all: not the driver's fault, so not the driver's problem.
        Err(_) => return Ok(()),
    };

    if outcome.status.success() {
        return Ok(());
    }

    let said = String::from_utf8_lossy(&outcome.stderr);
    let said = said.trim();

    // A process that exits with a code refused the work and explained itself. A process with no
    // code was killed, which on every platform here means it faulted -- and that is the case this
    // whole file is about.
    let what = match outcome.status.code() {
        Some(code) if !said.is_empty() => format!("the check failed ({code}): {said}"),
        Some(code) => format!("the check exited {code} without saying why"),
        None => "the graphics driver crashed the process while building the kernel".to_owned(),
    };

    Err(format!(
        "{what}.\n  \
         The grind will run on the CPU, at the speed it always has. If this is an Intel device on \
         a distribution's own driver, it is very likely too old for the hardware -- \
         intel/compute-runtime has current packages. Run `gpuinfo` for the details.",
    ))
}

/// `gpuinfo`, next to whatever is running now.
///
/// Two places, because a test binary does not live where the binaries it tests do: cargo puts it
/// in `target/release/deps/` and puts `gpuinfo` one level up. That matters more than it looks --
/// `cargo test` on a machine with a bad driver is exactly when somebody first meets this, and a
/// probe that only works for shipped binaries would let the test suite be the one thing that
/// still crashes.
fn sibling_gpuinfo() -> Option<PathBuf> {
    let here = std::env::current_exe().ok()?;
    let folder = here.parent()?;

    let name = if cfg!(windows) {
        "gpuinfo.exe"
    } else {
        "gpuinfo"
    };

    // `gpuinfo` probing a copy of itself is allowed, and has to be: it is the one binary somebody
    // runs *because* their device is misbehaving, so it is the last thing that should be left as
    // the one that still crashes. The child is given `--probe`, which its `main` acts on before
    // anything else, so this is a check and not a second full run.
    //
    // Bound rather than returned directly: `folder` borrows `here`, and a temporary in a block's
    // tail expression is dropped after that block's locals are.
    let found = [Some(folder), folder.parent()]
        .into_iter()
        .flatten()
        .map(|at| at.join(name))
        .find(|probe| probe.is_file());

    found
}
