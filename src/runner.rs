//! Running a suggested command, for somebody who has never used a terminal.
//!
//! `start` works out what is worth grinding on this machine and prints it. Somebody who has used
//! a terminal before copies that line and is away. Somebody who has not is looking at what they
//! described on Discord as "just code", and stops -- which is the whole loss this module exists
//! to prevent. They wanted to help; they got as far as the last step and could not take it.
//!
//! Two things make that step hard, and neither is about understanding:
//!
//! - The printed commands say `confirm_cw`, and what is on disk is `bin\windows\confirm_cw.exe`.
//!   A reader is expected to make that substitution silently. Here it is done by putting the
//!   platform's `bin` folder on the path, which leaves the printed command exactly as printed.
//! - Some of them are shell lines with a pipe or an `&&` in them, so they need a shell rather
//!   than a process. Which shell differs by platform, and that is not a thing to have to know.

use std::path::PathBuf;
use std::process::Command;

use crate::paths;

/// Where this platform keeps the committed executables.
///
/// The names are the ones the repository already uses in `bin/`, so this is a lookup rather than a
/// decision.
fn committed_binaries() -> Option<PathBuf> {
    let folder = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };

    let path = paths::root().join("bin").join(folder);

    path.is_dir().then_some(path)
}

/// Everywhere a suggested command's programs might be, ahead of whatever the machine already has.
///
/// Both, in this order, because both are real situations: somebody who downloaded the repository
/// has `bin/`, and somebody who built it with cargo has `target/release` and possibly no `bin/` at
/// all. Putting them first rather than last means a stale copy on the machine's own path cannot
/// quietly answer instead.
fn search_path() -> Option<std::ffi::OsString> {
    let mut ours: Vec<PathBuf> = Vec::new();

    if let Some(folder) = committed_binaries() {
        ours.push(folder);
    }

    let built = paths::root().join("target").join("release");

    if built.is_dir() {
        ours.push(built);
    }

    if ours.is_empty() {
        return None;
    }

    let existing = std::env::var_os("PATH").unwrap_or_default();
    let mut all = std::env::split_paths(&existing).collect::<Vec<_>>();

    ours.append(&mut all);

    std::env::join_paths(ours).ok()
}

/// Runs one suggested command line, exactly as it was printed, and returns whether it succeeded.
///
/// Through a shell on purpose. The suggestions are shell lines -- one pipes a generator into a
/// confirmer, another chains two commands with `&&` -- and rewriting them into process
/// invocations would mean the thing that runs and the thing that was printed could drift apart.
/// What somebody is shown is what happens.
///
/// Output is not captured. A pass runs for an hour and prints its progress as it goes, and that
/// progress is the only thing telling a first-time contributor that their machine is doing
/// anything at all.
pub fn run(command: &str) -> std::io::Result<bool> {
    let mut process = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        c
    };

    process.current_dir(repo_or_here());

    if let Some(path) = search_path() {
        process.env("PATH", path);
    }

    Ok(process.status()?.success())
}

/// The repository, which `paths::root` finds from here or from wherever this executable sits.
///
/// A suggested command is written as though typed at the top of the repository -- it names
/// `scripts/tails.py` and writes into `plans/` -- so it has to run from there. Somebody who
/// double-clicked an executable inside `bin\windows` is not there, and that is the ordinary case
/// for the person this is for.
fn repo_or_here() -> PathBuf {
    paths::root()
}

#[cfg(test)]
mod tests {
    /// A suggested command runs through a shell, because some of them are shell lines -- one pipes
    /// a generator into a confirmer, another chains two commands. Both platforms are covered by
    /// the same call here, which is the point.
    #[test]
    fn a_command_runs_and_reports_how_it_went() {
        assert!(super::run("echo hash-slinging-slasher").unwrap());
    }

    /// And a command that fails is reported as failing rather than as an error. The difference
    /// matters to the caller: one means the search stopped early, the other means it never
    /// started, and a first-time contributor is told something different for each.
    #[test]
    fn a_failing_command_is_not_an_error() {
        let outcome = super::run("exit 3");

        assert!(
            outcome.is_ok(),
            "the shell ran; the command inside it did not succeed"
        );
        assert!(!outcome.unwrap());
    }
}
