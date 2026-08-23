//! Turning what `start` already decided into something a first-timer can act on.
//!
//! # What this is not
//!
//! It is not "run the top suggestion". `suggest` in `startup.rs` carries a warning with a
//! measurement behind it: the suggestions are a *floor, not a plan*, and running the top of a
//! global ranking is how one contributor came to spend fourteen submissions on a method that was
//! already spent -- the last testing a hundred and one trillion candidates for twenty names. They
//! were doing exactly what the list told them to.
//!
//! A prompt reading `run the suggested search? [Y/n]` would turn that ranking into a ladder with a
//! button on it, and would do it specifically to the people least able to know better. Worse, it
//! would do it to *fresh clones*, which all get the same suggestion, so every newcomer would
//! generate the same candidates and `submit` would drop all but the first.
//!
//! So what is offered is the same short list `suggest` prints, numbered, with nothing preselected.
//! The choice a contributor makes is the one they would have made by reading; what is removed is
//! having to retype it, which is the step people actually stop at -- "the tutorial feels confusing
//! cuz i just see code and get confused", as somebody put it while trying to help.
//!
//! # Why it stays quiet unless a person is there
//!
//! `AGENTS.md` tells an assistant to grind for hours rather than stop and ask things, and `start`
//! is the first thing it runs. A prompt on a pipe would hang that, and hang CI with it. So this
//! asks only when both ends are a terminal, and takes no answer as no.

use std::io::{BufRead, IsTerminal, Write};

use crate::runner;

/// One thing `start` worked out is worth doing here, kept whole so it can be run exactly as shown.
pub struct Offer {
    /// The command line, as printed. What runs and what was shown must be the same string.
    pub command: String,
    /// The first line of why, for the menu. The full reasoning is already above it on screen.
    pub gist: String,
}

/// Whether there is a person at this terminal to ask.
///
/// Both ends checked. Output redirected to a file means somebody is capturing a log; input from a
/// pipe means a program is driving. Either way there is nobody to answer, and the right behaviour
/// is the behaviour this program has always had.
fn somebody_is_there() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

/// Offers the choices, and runs the one that is picked.
///
/// Does nothing at all when the answer is anything but a number on the list, including when it is
/// empty. Nothing is preselected and Enter alone chooses nothing: a contributor who did not read
/// the reasoning above should end up having run nothing, not having run whatever was first.
pub fn ask(offers: &[Offer]) {
    if offers.is_empty() || !somebody_is_there() {
        return;
    }

    println!("\nIf you would rather not retype one of those, pick a number.\n");

    for (index, offer) in offers.iter().enumerate() {
        println!("  {}) {}", index + 1, offer.command);
        println!("     {}", offer.gist);
    }

    println!("\n  anything else, or just Enter -- run nothing, and choose your own");
    print!("\n> ");
    let _ = std::io::stdout().flush();

    let mut answer = String::new();

    if std::io::stdin().lock().read_line(&mut answer).is_err() {
        return;
    }

    let Some(offer) = answer
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|n| *n >= 1 && *n <= offers.len())
        .map(|n| &offers[n - 1])
    else {
        println!("\nNothing run. Everything above is still there to read.");
        return;
    };

    // Said before it starts, because the next thing on screen is an hour of a search's own output
    // and somebody seeing that for the first time should know what they set off and how to stop
    // it. A pass is interruptible at any point: what it has already confirmed is on disk.
    println!("\nrunning: {}\n", offer.command);
    println!("This runs until it is done -- often an hour. Ctrl+C stops it, and anything it has");
    println!("already found is kept. When it finishes, `submit` sends what it found.\n");

    match runner::run(&offer.command) {
        Ok(true) => println!("\nFinished. Run `submit` to send what that found."),
        Ok(false) => println!(
            "\nThat stopped without finishing. Anything it did confirm is still on disk, and \
             running `start` again will say what is worth doing next."
        ),
        Err(why) => println!("\nCould not start it: {why}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gate that keeps this out of everybody else's way.
    ///
    /// A test binary's streams are not a terminal, and neither are an assistant's or a CI job's.
    /// If this ever returned having run something, `start` would hang the moment `AGENTS.md` told
    /// an assistant to run it -- and the failure would look like the whole project being broken.
    #[test]
    fn nothing_is_offered_when_nobody_is_watching() {
        assert!(
            !somebody_is_there(),
            "a test's streams are pipes; if this says otherwise the gate is not the gate"
        );

        // Would block forever on a read if the gate were wrong, so reaching the next line is the
        // assertion.
        ask(&[Offer {
            command: "this must never run".to_owned(),
            gist: "nor this".to_owned(),
        }]);
    }

    /// Having nothing to offer is not a reason to say anything.
    #[test]
    fn an_empty_list_is_silent() {
        ask(&[]);
    }
}
