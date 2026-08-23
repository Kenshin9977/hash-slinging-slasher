//! The step before the first step.
//!
//! Every instruction in this repository begins with `bin\windows\start.exe`, which is a path
//! *inside* the repository. How to get the repository is written nowhere near the top, and that is
//! where people stop. From Discord, from somebody who wanted to help:
//!
//! > can someone help me setup the hash slinging slasher? the tutorial feels confusing cuz i just
//! > see code and get confused. i want to help dehash cold war
//!
//! The answer he was given was to point an AI agent at the repository, which is a real answer and
//! is also a second thing to go and learn. He did not come back.
//!
//! This is one file to download and run. It finds or installs git, clones the repository, and
//! starts `start`, which already does everything else -- it installs what is missing, brings the
//! clone up to date, reads what other people have in flight so tonight is not a duplicate, and
//! works out what is worth grinding here. None of that needed changing. It only needed reaching.
//!
//! It deliberately does nothing else. It does not choose a search, does not configure anything and
//! does not touch a submission: `start` owns all of that and is better at it. This exists to end
//! at the point where the documentation currently begins.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the repository comes from.
///
/// Upstream rather than any fork, because a newcomer wants the project and not somebody's branch,
/// and because `start` reads open pull requests against wherever it was cloned from to avoid
/// duplicating work in flight.
const SOURCE: &str = "https://github.com/KingslayerKyle/hash-slinging-slasher.git";

/// The folder made next to wherever this was run from.
const FOLDER: &str = "hash-slinging-slasher";

fn main() {
    println!("hash-slinging-slasher\n");
    println!("This recovers Call of Duty asset names that are stored as numbers rather than text.");
    println!("You do not need the game, and you do not need to understand any of it to help:");
    println!("your computer does the searching, and the results are checked automatically.\n");

    // Already inside a checkout -- somebody downloaded the repository and then ran this from
    // inside it. Nothing to fetch; go straight to the thing that knows what to do.
    if let Some(here) = checkout_containing(&std::env::current_dir().unwrap_or_default()) {
        println!("Found the repository here already. Starting it.\n");
        hand_over(&here);
    }

    let into = std::env::current_dir().unwrap_or_default().join(FOLDER);

    if into.join("AGENTS.md").is_file() {
        println!("Already downloaded, in {}. Starting it.\n", into.display());
        hand_over(&into);
    }

    if !have("git") {
        println!("It needs `git` to download the project, and this machine does not have it.\n");

        if !slasher::startup::install("Git.Git", "git") || !have("git") {
            println!(
                "\nInstall git from https://git-scm.com/downloads, then run this again.\n\
                 Nothing else is needed and nothing here has been changed."
            );
            stop(1);
        }
    }

    println!("Downloading the project into {} ...", into.display());
    println!("It is a few hundred megabytes, mostly the captured asset numbers.\n");

    let cloned = Command::new("git")
        .args(["clone", "--depth", "1", SOURCE])
        .arg(&into)
        .status()
        .map(|status| status.success())
        .unwrap_or(false);

    if !cloned {
        println!(
            "\nThe download did not finish. That is almost always the network or a firewall.\n\
             Trying again usually works; nothing has been left behind that would stop it."
        );
        stop(1);
    }

    println!("\nDownloaded. Handing over to the project's own setup, which takes it from here.\n");
    hand_over(&into);
}

/// Runs `start` in the checkout and exits with whatever it exits with.
///
/// A handover rather than a wrapper. Everything a contributor needs to be told next -- what is
/// missing, what other people are already doing, what is worth grinding on this machine -- `start`
/// works out and says better than anything here could repeat.
fn hand_over(repo: &Path) -> ! {
    let Some(start) = start_binary(repo) else {
        println!(
            "The project downloaded, but the executable for this platform is not in it.\n\
             Open {} and follow README.md from there.",
            repo.display()
        );
        stop(1);
    };

    let status = Command::new(&start).current_dir(repo).status();

    match status {
        Ok(status) => stop(status.code().unwrap_or(0)),
        Err(why) => {
            println!("Could not run {}: {why}", start.display());
            stop(1)
        }
    }
}

/// The committed `start` for whatever this machine is.
fn start_binary(repo: &Path) -> Option<PathBuf> {
    let (folder, name) = if cfg!(windows) {
        ("windows", "start.exe")
    } else if cfg!(target_os = "macos") {
        ("macos", "start")
    } else {
        ("linux", "start")
    };

    let path = repo.join("bin").join(folder).join(name);

    path.is_file().then_some(path)
}

/// The checkout `from` is inside, if it is inside one.
///
/// `AGENTS.md` is the marker: it is at the top of the repository, it is not going anywhere, and
/// its presence means the thing around it is this project rather than a folder that happens to
/// share a name.
fn checkout_containing(from: &Path) -> Option<PathBuf> {
    let mut here = Some(from);

    while let Some(directory) = here {
        if directory.join("AGENTS.md").is_file() && directory.join("snapshots").is_dir() {
            return Some(directory.to_path_buf());
        }

        here = directory.parent();
    }

    None
}

fn have(tool: &str) -> bool {
    let probe = if cfg!(windows) { "where" } else { "which" };

    Command::new(probe)
        .arg(tool)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Exits, having waited if there is somebody reading.
///
/// Double-clicked on Windows, this runs in a console window that closes the instant the program
/// ends -- so whatever it just said about what went wrong is gone before it can be read. That is
/// the single most common way a first-time contributor is left with nothing to act on.
fn stop(code: i32) -> ! {
    use std::io::IsTerminal;

    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        println!("\nPress Enter to close.");
        let _ = std::io::stdin().read_line(&mut String::new());
    }

    std::process::exit(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run from inside a checkout, there is nothing to download and it must say so rather than
    /// cloning a second copy alongside the one somebody is standing in.
    #[test]
    fn a_checkout_is_recognised_from_anywhere_inside_it() {
        let here = std::env::current_dir().expect("a working directory");
        let found = checkout_containing(&here).expect("the tests run inside the repository");

        assert!(found.join("AGENTS.md").is_file());
        assert!(found.join("snapshots").is_dir());

        // And from a folder well inside it, since that is where somebody double-clicking in
        // `bin\windows` would be.
        let deep = here.join("src").join("bin");

        assert_eq!(
            checkout_containing(&deep).as_deref(),
            Some(found.as_path()),
            "walking up from a subfolder must find the same repository"
        );
    }

    /// Somewhere that is not a checkout is not a checkout. Getting this wrong would mean handing
    /// over to a `start` that is not there, having reported success.
    #[test]
    fn an_unrelated_folder_is_not_mistaken_for_one() {
        let elsewhere = std::env::temp_dir();

        assert!(
            checkout_containing(&elsewhere).is_none(),
            "the temporary folder is not this project, whatever it happens to contain"
        );
    }

    /// The committed executable is looked for under this platform's own folder. If the name or
    /// the layout ever moves, this is what says so -- rather than a contributor being told the
    /// download worked and then nothing happening.
    #[test]
    fn the_start_binary_is_where_bin_keeps_it() {
        let repo = checkout_containing(&std::env::current_dir().unwrap()).unwrap();

        assert!(
            start_binary(&repo).is_some(),
            "bin/ has no start for this platform, so bootstrap would download and then stall"
        );
    }
}
