//! Everything that must be true before a search, done rather than asked for.
//!
//! Three things decide whether a night of grinding is worth anything, and all three have failed
//! in the field repeatedly:
//!
//! 1. **git and the GitHub CLI have to work, and `gh` has to be signed in.** People arrive keen,
//!    are told to "install gh and run `gh auth login`", and are then stuck: the installer amends
//!    PATH but the terminal they are standing in already started, so `gh` is "not recognised" and
//!    it looks like the install failed. Hours have gone on this.
//! 2. **The clone has to be current.** Everything a search excludes against -- the tables, the
//!    merged submissions -- moves daily. A stale clone rediscovers last night's work and the run
//!    looks like a success right up until the pull request is a duplicate.
//! 3. **What everyone else has in flight has to be known.** The clone cannot see an open pull
//!    request, and that is precisely where duplicates come from.
//!
//! Each of those was documented, and each was skipped anyway. So this is a program. There is one
//! command to remember, it does all three, and it exits non-zero when it cannot -- which is the
//! only kind of instruction that survives contact with a tired assistant at four in the morning.

use std::path::Path;

use crate::{
    config, disk, github, offer, paths, readiness, recon, snapshot, tables, update, LOW_VALUE_POOLS,
};

/// Where findings go, and therefore whose open pull requests are worth reading.
pub const DEFAULT_REPO: &str = "KingslayerKyle/hash-slinging-slasher";

/// The argument `start` re-runs itself with once it is safely in the temporary folder.
pub const RELAUNCHED: &str = "--relaunched";

/// Skips installing anything, for somebody who manages their own tools.
pub const NO_INSTALL: &str = "--no-install";

/// Runs every check, fixes what it can, and returns whether a grind may begin.
pub fn run() -> bool {
    let arguments: Vec<String> = std::env::args().collect();
    let may_install = !arguments.iter().any(|argument| argument == NO_INSTALL);

    println!("getting this machine ready to grind\n");

    let repo = Path::new(".");
    let mut blocked: Vec<String> = Vec::new();
    let mut warned: Vec<String> = Vec::new();

    // 1. git. The tables are fetched with it and the clone is updated with it, so nothing below
    //    this line works without it.
    if !update::have_git() {
        if may_install && install("Git.Git", "git") && update::have_git() {
            println!("  [fixed]   git installed");
        } else {
            println!("  [BLOCKED] git is not installed");
            blocked.push(format!(
                "Install git -- the hash tables are fetched with it and the clone is updated \
                 with it:\n              {}\n          Then run this again.",
                update::install_git_hint()
            ));
        }
    } else {
        println!("  [ok]      git");
    }

    // 2. The clone itself. Done before anything reads a file out of it, because a pull changes
    //    what those files say.
    if update::have_git() {
        match update::check(repo) {
            update::State::Current => println!("  [ok]      clone is up to date ({})", update::head(repo)),
            update::State::Ahead(by) => {
                println!("  [ok]      clone is {by} commit(s) ahead of the remote, which is fine")
            }
            update::State::Behind(by) => {
                println!("  [....]    clone is {by} commit(s) behind -- updating");

                match update::bring_current(repo) {
                    Ok(said) => println!("  [fixed]   {said}updated to {}", update::head(repo)),
                    Err(why) => {
                        println!("  [BLOCKED] the clone could not be updated");
                        blocked.push(why);
                    }
                }
            }
            update::State::Diverged { ahead, behind } => {
                println!("  [BLOCKED] clone has diverged: {ahead} local, {behind} remote");
                blocked.push(
                    "This clone and the remote have both moved, so there is no fast-forward and \
                     guessing at a merge is not this program's business. Sort it out with `git \
                     status` and `git log --oneline --graph --all`, then run this again."
                        .to_owned(),
                );
            }
            update::State::NotAClone => {
                println!("  [warn]    this is not a git checkout");
                warned.push(
                    "this folder was downloaded rather than cloned, so it cannot be updated and \
                     will fall behind. Clone it instead:\n                git clone \
                     https://github.com/KingslayerKyle/hash-slinging-slasher"
                        .to_owned(),
                );
            }
            update::State::NoRemote => {
                warned.push("this clone has no remote, so it cannot be checked for updates.".to_owned())
            }
            update::State::Unknown(why) => {
                println!("  [warn]    could not check for updates: {why}");
                warned.push(format!(
                    "the remote could not be reached ({why}), so this clone may be behind. \
                     Anything found tonight may already have been submitted by somebody else."
                ));
            }
        }
    }

    if let Some(complaint) = update::binaries_behind_source(repo) {
        warned.push(complaint);
    }

    // 3. GitHub. The loudest check, because a night of grinding with nowhere to send it is a
    //    night wasted, and this is the one people get stuck on.
    match sign_in_state(may_install) {
        SignIn::Ready { who, on_path } => {
            if on_path {
                println!("  [ok]      signed in to GitHub as {who}");
            } else {
                println!(
                    "  [ok]      signed in to GitHub as {who} (found by install path; this \
                     terminal started before the install and does not know the plain `gh` \
                     command, which is harmless)"
                );
            }
        }
        SignIn::NotSignedIn(gh) => {
            println!("  [BLOCKED] GitHub: installed, but not signed in");
            blocked.push(format!(
                "Sign in to GitHub. This is the one step that cannot be done for you, because it \
                 opens a browser and asks you to approve it.\n\n              {}\n\n          \
                 Answer: GitHub.com, then HTTPS, then \"Login with a web browser\". Copy the code \
                 it shows, press Enter, and paste the code into the page that opens.\n          \
                 Findings cannot be submitted without this, so there is no point starting a \
                 search until it passes.",
                gh.login_hint()
            ));
        }
        SignIn::Missing => {
            println!("  [BLOCKED] GitHub CLI (`gh`) is not installed");
            blocked.push(
                "Install the GitHub CLI:\n              winget install --id GitHub.cli\n          \
                 (anywhere else: https://cli.github.com). Then run this again -- it will tell you \
                 exactly how to sign in, including the full path, because a terminal opened \
                 before the install does not know the `gh` command yet."
                    .to_owned(),
            );
        }
    }

    // 4. The snapshots. What a name is confirmed against, and the only thing here that cannot be
    //    regenerated without owning the game.
    let mut games = Vec::new();
    if let Ok(entries) = std::fs::read_dir(paths::snapshots()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("ids") {
                continue;
            }

            match snapshot::Snapshot::read(&path) {
                Ok(snap) => {
                    println!("  [ok]      snapshot for {} holds {} assets", snap.game(), snap.len());
                    games.push(snap.game().to_owned());
                }
                Err(why) => {
                    println!("  [BLOCKED] {} is unreadable: {why}", path.display());
                    blocked.push(format!("{} could not be read: {why}", path.display()));
                }
            }
        }
    }

    if games.is_empty() {
        println!("  [BLOCKED] no snapshot found");
        blocked.push(
            "The snapshots ship with the repository and never change, so this means one has been \
             deleted or `snapshots` in config.toml points elsewhere. Nothing can be confirmed \
             without one."
                .to_owned(),
        );
    }

    // 5. The tables. A search that cannot read them reports every published name in the game as
    //    a discovery, which looks exactly like success.
    let mut table_count = 0;
    match tables::ensure(&paths::tables(), false) {
        Ok(count) => {
            table_count = count;

            // Upstream's own commit, not our file timestamps. A freshly fetched checkout of
            // month-old data has timestamps of zero hours ago, so reporting those would say
            // "current" about the one situation worth warning about.
            let version = tables::upstream_version(&paths::tables())
                .map(|(sha, date)| format!(", at cod-name-db {sha} of {date}"))
                .unwrap_or_default();

            println!("  [ok]      {count} hash tables{version}");
        }
        Err(why) => {
            println!("  [BLOCKED] hash tables: {why}");
            blocked.push(format!(
                "The tables could not be fetched from {}. Nothing can be judged without them -- a \
                 search that cannot read them calls every published name in the game a discovery.\
                 \n          {why}",
                tables::UPSTREAM
            ));
        }
    }

    // 6. What everybody else is doing. The part no local file can answer.
    let landscape = survey(&mut warned);

    // 7. What this machine is set to hunt, and whether any of it is known to be a waste.
    if let Some(moved) = paths::migrate_flat_findings() {
        println!("  [fixed]   {moved}");
    }

    let targets = config::targets();
    println!("  [ok]      hunting {} in {}", targets.describe(), config::game());

    for (pool, _) in LOW_VALUE_POOLS {
        if crate::pool_index(pool).map(|index| targets.wants(index)).unwrap_or(false) {
            warned.push(format!(
                "config.toml has this machine hunting `{pool}`, which is a known waste of a night \
                 -- see LOW_VALUE_POOLS in src/lib.rs for what it cost last time."
            ));
        }
    }

    // 8. Disk, because a grind ends abruptly by design and the last session may not have tidied.
    let findings = paths::findings();
    if !findings.exists() && std::fs::create_dir_all(&findings).is_err() {
        warned.push("the findings folder could not be created; results will have nowhere to go.".to_owned());
    }

    let reclaimable: u64 = disk::leftovers(Path::new(".")).iter().map(|left| left.bytes).sum();
    if reclaimable > 200_000_000 {
        println!(
            "  [warn]    {} of leftovers from previous sessions -- `tidy` reclaims it",
            disk::human(reclaimable)
        );
    }

    if !warned.is_empty() {
        println!();
        for warning in &warned {
            println!("  [warn]    {warning}");
        }
    }

    if !blocked.is_empty() {
        println!("\n{} thing(s) must be fixed before grinding:\n", blocked.len());
        for (number, fix) in blocked.iter().enumerate() {
            println!("  {}. {fix}\n", number + 1);
        }
        println!("Nothing else in this repository should be run until the above passes.");
        return false;
    }

    let _ = readiness::write(&update::head(repo), table_count, landscape.claimed.len());
    let _ = recon::save(&landscape);

    report(&landscape);
    true
}

/// Reads the open pull requests and the merged submissions, and says what that means for tonight.
fn survey(warned: &mut Vec<String>) -> recon::Landscape {
    let repo = config::path("submit_repo")
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| DEFAULT_REPO.to_owned());

    println!("  [....]    reading what other people have in flight");
    let landscape = recon::survey(&repo, &paths::submissions());

    if let Some(why) = &landscape.offline {
        println!("  [warn]    open pull requests could not be read");
        warned.push(format!(
            "GitHub would not answer ({why}), so only merged submissions could be excluded. \
             Anything found tonight may duplicate a pull request that is open right now."
        ));
    } else {
        println!(
            "  [ok]      {} open submission(s), {} name(s) claimed in them and not yet merged",
            landscape.open.len(),
            landscape.claimed_in_flight
        );
    }

    landscape
}

/// The closing advice: what is already taken, and therefore what tonight should not be.
fn report(landscape: &recon::Landscape) {
    println!("\nready. Nothing here will stop a grind.\n");

    if !landscape.open.is_empty() {
        println!("in flight right now -- do not submit these names again:");
        for submission in &landscape.open {
            println!(
                "  #{:<5} {:<16} {} name(s)  {}",
                submission.number,
                submission.author,
                submission.names.len(),
                submission.title
            );
        }
        println!();
    }

    if !landscape.swept.is_empty() {
        println!(
            "{} search configuration(s) have already been run to exhaustion and submitted.\n\
             A search whose fingerprint is one of those will find exactly what it found for them,\n\
             and the tools will say so before it starts.\n",
            landscape.swept.len()
        );
    }

    let game = which_game();
    library();
    let offers = suggest(&game);

    println!(
        "\nSubmit after each job rather than at the end of the night, and do not ask first --\n\
         `submit` re-checks all of this at the moment of sending and drops anything already taken."
    );

    // Only ever reaches a person at a terminal. An assistant, or a CI job, sees exactly what
    // it saw before this existed -- which is what `AGENTS.md` promises it.
    offer::ask(&offers);
}

/// Which game to grind next, and why -- chosen rather than asked.
///
/// This project is called a Cold War **and** Black Ops 4 solver, and until now it was neither by
/// default: `config.toml` does not exist in a fresh clone, the fallback is Cold War, and so every
/// contributor ground the same title. Black Ops 4 has **more** unnamed assets in the five types
/// that matter -- 141,889 against 136,467 -- and is far less picked over: 64% of its images are
/// named against 81% of Cold War's.
///
/// Asking would be the obvious fix and it is the wrong one. The whole instruction to an assistant
/// here is *do not stop and ask*, and a question at the top of every session is both a
/// contradiction and one more thing for a tired user to get wrong at four in the morning.
///
/// So it alternates, per clone, by **how many passes each game has had here** -- and passes
/// rather than names on purpose. Counting names starves whichever game yields fewer of them: one
/// good Cold War night puts it thousands ahead, and a rule chasing the smaller number would send
/// every pass to Black Ops 4 until it caught up, which could be weeks. Counting passes gives each
/// game every other run whatever they return, which is the behaviour actually wanted.
///
/// A fresh clone has none of either, and the tie goes to Black Ops 4: it has more unnamed assets
/// in the five types that matter (141,889 against 136,467) and is much less picked over -- 64% of
/// its images are named against 81% of Cold War's -- and exactly one contributor has ever ground
/// it, because switching meant editing a file most people never create.
///
/// Nobody chooses, both get ground, and one flag overrides it for anybody who cares.
fn which_game() -> String {
    // `start --game X` means X, and it has to be written down as well as obeyed. Without this the
    // flag was accepted, silently ignored, and then *contradicted*: the turn-taking overwrote
    // `state/game.txt` with its own pick, so every search afterwards went to the other game. A
    // flag that quietly does the opposite of what it says is worse than no flag.
    if std::env::args().any(|argument| argument == "--game") {
        let chosen = config::game();
        let _ = config::choose_game(&chosen);

        println!(
            "grinding {chosen}, because `--game` said so. That holds for the searches that follow \
             too.\n\n  Run `start` with no flag to go back to the two taking turns.\n"
        );
        return chosen;
    }

    // Switched off deliberately, with a key that cannot be in an old config file.
    if !config::alternates() {
        let chosen = config::game();
        println!(
            "grinding {chosen} only: config.toml says `alternate_games = false`.\n\n  \
             Remove that line and the two games take turns again, so neither is left behind.\n"
        );
        return chosen;
    }

    // Black Ops 4 first in the list, so it wins a tie without needing a special case.
    let order = ["BLKOPS04", "BLKOPSCW"];

    let mut standings: Vec<(usize, &str)> = order
        .iter()
        .filter(|game| config::GAMES.contains(game))
        .map(|game| (passes_here(game), *game))
        .collect();

    standings.sort_by_key(|(passes, _)| *passes);

    let least = standings[0].1;

    // Written down rather than merely announced, so the searches pick it up without anybody
    // having to carry a flag across from an earlier command's output.
    let _ = config::choose_game(least);

    println!("what to grind next:\n");

    // No "<- configured" marker here on purpose: reaching this point means nothing was chosen,
    // and pointing at the fallback as though it were a decision is how the fallback came to be
    // treated as one.
    for (passes, game) in &standings {
        println!(
            "  {game:<10} {passes:>4} pass(es) on this machine, {} name(s) confirmed",
            confirmed_here(game)
        );
    }

    println!(
        "\n  Grinding {least} next -- it has had the fewer passes here, so the two take turns and\n  \
         neither is left behind. Every search below picks this up on its own; no flag needed.\n  \
         Findings are kept per game, so switching costs nothing.\n\n  \
         `--game <TAG>` forces one game for one run, when there is a reason to. To stop the\n  \
         turn-taking altogether, put `alternate_games = false` in config.toml."
    );

    println!();
    least.to_owned()
}

/// How many search passes this machine has run for one game.
fn passes_here(game: &str) -> usize {
    let folder = paths::findings_root().join(game.to_lowercase());

    std::fs::read_dir(&folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            entry.path().is_dir()
                && entry.file_name().to_string_lossy().starts_with("run_")
        })
        .count()
}

/// How many names this machine has confirmed for one game. Reported, never used to choose.
fn confirmed_here(game: &str) -> usize {
    let folder = paths::findings_root().join(game.to_lowercase());

    std::fs::read_dir(&folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("txt"))
        .map(|entry| std::fs::read_to_string(entry.path()).unwrap_or_default().lines().count())
        .sum()
}

/// Everything already in the script library, printed before anything suggests inventing.
///
/// "Invent a new method" is the instruction this repository leans on hardest, and it has an
/// obvious failure mode: an assistant with no memory of last night invents a method that already
/// exists, under a different name, and spends the evening re-deriving `continuations.py`. Telling
/// it to "read `scripts/README.md` first" is another instruction competing for the same
/// attention, and instructions are what this project keeps discovering do not survive.
///
/// So the inventory is printed rather than referenced. Every assistant reads this output --
/// it is the one command they are told to run -- so the list of what exists lands in front of
/// them before the word "invent" appears, which is the only ordering that helps.
///
/// The one-line purpose is the first line of each script's own docstring, so a contributed script
/// describes itself here the moment it lands and nobody has to maintain a second list.
fn library() {
    let mut listed: Vec<(String, String)> = Vec::new();

    for folder in ["scripts", "scripts/contributed"] {
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };

        let mut here: Vec<(String, String)> = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("py") {
                continue;
            }

            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };

            // `settings.py` is a library the others read config through, not a method.
            if name == "settings.py" {
                continue;
            }

            if let Some(purpose) = first_docstring_line(&path) {
                here.push((format!("{folder}/{name}"), purpose));
            }
        }

        here.sort();
        listed.extend(here);
    }

    if listed.is_empty() {
        return;
    }

    println!("the script library -- read this before inventing anything, so you do not invent it twice:\n");

    for (path, purpose) in &listed {
        let purpose: String = purpose.chars().take(88).collect();
        println!("  {path:<34} {purpose}");
    }

    println!("\n  scripts/README.md says which are reconnaissance and which are methods.");
    println!("  METHODS.md says what each one reaches and when it is spent.\n");
}

/// The first line of a Python file's module docstring, which is where these scripts state what
/// they are for.
fn first_docstring_line(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let rest = text.split_once("\"\"\"")?.1;
    let line = rest.lines().next()?.trim();

    (!line.is_empty()).then(|| line.trim_end_matches("\"\"\"").trim().to_owned())
}

/// What to run tonight, chosen from what this clone has *not* run.
///
/// The alternative -- "see METHODS.md" -- is what the documentation said before, and what it
/// produced was everybody running the first method listed and submitting the same names. So the
/// suggestion is specific, and it is specific to this machine: a run folder's label records which
/// method made it, so a method already run here drops down the list rather than being offered
/// again first.
///
/// It is a suggestion and says so. Anything in `METHODS.md` is a legitimate choice, and inventing
/// something that is not in it is the best choice of all.
fn suggest(game: &str) -> Vec<offer::Offer> {
    let mut chosen: Vec<offer::Offer> = Vec::new();
    let ran = methods_already_run();

    // Every suggested command carries the game, so following it verbatim grinds the right one.
    let on = if game == config::game() { String::new() } else { format!(" --game {game}") };

    // In the order METHODS.md ranks them, with what each reaches, so the choice is informed
    // rather than obeyed.
    // Ordered by what the **run record** measures, not by age. This list used to offer the general
    // search first, described as "the thing to run first in a fresh clone", and every method on it
    // was a built-in binary. The consequence was measurable on a contributor's account: fourteen
    // submissions, six of them the general search, and the last of those tested 101,782,194,076,950
    // candidates for **20 new names** -- one per five trillion. They were not doing anything wrong.
    // They were doing what this list told them to.
    //
    // Anything added here must carry its measured figure, so a reader can disagree with the order.
    let ladder: &[(&str, &str, &str)] = &[
        (
            "closure",
            "python scripts/derive_closure.py",
            "free, and it multiplies everything else: 61 seeds from one pass became 477 names. Run it after any pass at all, including one you are about to write off",
        ),
        (
            "plan",
            "python scripts/tails.py --head --length 3 --write-plan plans/heads3.txt && confirm_plan plans/heads3.txt",
            "replaces a known name's first three characters -- 692 names on Cold War in one pass, on ground nothing had ever asked about",
        ),
        (
            "plan",
            "python scripts/tails.py --length 3 --write-plan plans/tails3.txt && confirm_plan plans/tails3.txt",
            "the same for the last three characters: 1,151 names in twenty-one seconds, which the report calls free",
        ),
        (
            "list",
            "python scripts/final_byte.py | confirm_list - --label \"final byte solved backwards\"",
            "solves rather than searches -- the hash inverts for a name's final character. One name per 18 candidates, the best measured here",
        ),
        (
            "images",
            "images_from_materials",
            "the strongest measured cross-type seam: material and image share 15,770 cores",
        ),
        (
            "variants",
            "confirm_variants",
            "walks numbers in place, which no beginning-stem-ending rule can reach. The method that fits xanim",
        ),
        (
            "sounds",
            "confirm_sounds",
            "everything past the first dot, which nothing else can put back on",
        ),
        (
            "all",
            "confirm_cw",
            "the general search. Widest net and the right first pass in a *fresh* clone -- but measured at 1 name per 5 trillion candidates once the corpus is picked over, so it is last here on purpose",
        ),
    ];

    // Said before the ladder, because the ladder is the weaker instrument and putting it first
    // is how a contributor came to spend fourteen submissions on the one method that was spent.
    //
    // The two halves are deliberately different in kind. A *ranking* is safe to broadcast when it
    // is used to rule things out -- "do not spend your night on that" cannot cause a collision.
    // Used the other way it is a ladder with extra steps: it is deterministic and global, so
    // everybody who runs its top row generates the same candidates and `submit` drops them all.
    // And it can only rank methods that already exist, so leaning on it steers away from the one
    // thing this project is for.
    //
    // What diverges naturally is the negative space: where the unnamed assets are, which relations
    // nothing has mined, what the lists structurally cannot express. Two people reading those pick
    // different ground.
    println!("before choosing, and this order matters:
");
    println!("  python scripts/methods_report.py --efficiency");
    println!("      what is spent. Use a ranking to rule methods OUT -- that is what it is good for.
");
    println!("  python scripts/coverage.py --five");
    println!("  python scripts/seams.py");
    println!("  python scripts/reach.py --missing");
    println!("      where nothing reaches yet. This is where a new method comes from, and unlike");
    println!("      the ranking it points different people at different ground.
");
    println!("  Run the top of the ranking *because* it is the top and you will collide with");
    println!("  everybody else who did. What follows is a floor, not a plan.
");

    println!("suggested next, given what this clone has already run:
");

    let mut offered = 0;

    // Sound is offered ahead of the ladder rather than inside it, because only two entries are
    // ever printed and sound would sit at the bottom of a list it can never reach the top of.
    //
    // It is also the largest unnamed ground in either game by a wide margin -- 70,878 of Black
    // Ops 4's 79,263 `sound_asset` ids, and 43,603 of Cold War's 50,890 `sound_alias` ids -- and
    // it is the one pass nobody runs by accident, because it needs a flag. That combination is
    // exactly how Black Ops 4 itself went unground for months: not a hard problem, just never the
    // default. The turn-taking fixed that for the game; this fixes it for the pass.
    if !ran.iter().any(|label| label == "soundfiles") {
        let fold = if game == "BLKOPS04" { " --no-fold" } else { "" };

        println!(
            "  confirm_cw --sounds{fold}{on}\n      sound files and aliases: the largest unnamed \
             ground in either game, and the one pass that needs a flag rather than happening by \
             default. Its own measured lists (`data/sound.*.txt`) and its own vocabulary.\n"
        );

        offered += 1;
        chosen.push(offer::Offer {
            command: format!("confirm_cw --sounds{fold}{on}"),
            gist: "the largest unnamed ground in either game".to_owned(),
        });
    }

    for (label, command, what) in ladder {
        if ran.contains(&(*label).to_owned()) {
            continue;
        }

        println!("  {command}{on}\n      {what}\n");
        offered += 1;
        chosen.push(offer::Offer {
            command: format!("{command}{on}"),
            gist: what.split(". ").next().unwrap_or(what).to_owned(),
        });

        if offered == 2 {
            break;
        }
    }

    if offered == 0 {
        println!("  every method in METHODS.md has been run in this clone at least once.");
        println!();
        println!("  Re-measuring the lists is not the way round again. It changes the general");
        println!("      search's fingerprint without changing what it can reach: three consecutive");
        println!("      folds returned 55 names, then 294, then 51, the last on a corpus two and a");
        println!("      half times larger. `derive_lists.py` is for when the lists have lost");
        println!("      vocabulary -- it reports what it dropped -- not for reopening a method the");
        println!("      ground has already spent.");
        println!();
        println!("  So invent one. `confirm_list` takes candidate names on standard input, so");
        println!("  a method is a script that prints names -- no Rust required. That is the");
        println!("  highest-value thing anybody does here, and the reason this repository is");
        println!("  pointed at an assistant rather than run as a fixed program.");
    } else {
        println!("  (or invent one -- `confirm_list` takes candidates on standard input, so a");
        println!("   method is just a script that prints names. See METHODS.md and");
        println!("   scripts/README.md.)");
    }

    chosen
}

/// Which methods this clone has run, from the label each run folder carries in its name.
fn methods_already_run() -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![paths::findings()];

    while let Some(here) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&here) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();

            // `run_<stamp>_<label>` is how `Results::write_run` names them.
            if let Some(rest) = name.strip_prefix("run_") {
                if let Some((_, label)) = rest.split_once('_') {
                    found.push(label.to_owned());
                }
            } else {
                stack.push(path);
            }
        }
    }

    found
}

/// The three states GitHub access can be in, each needing different words.
enum SignIn {
    Ready { who: String, on_path: bool },
    NotSignedIn(github::Gh),
    Missing,
}

fn sign_in_state(may_install: bool) -> SignIn {
    let mut located = github::locate();

    if located.is_none() && may_install && install("GitHub.cli", "gh") {
        located = github::locate_uncached();
    }

    let Some(gh) = located else {
        return SignIn::Missing;
    };

    let Ok(output) = gh.command().arg("auth").arg("status").output() else {
        return SignIn::Missing;
    };

    if !output.status.success() {
        return SignIn::NotSignedIn(gh);
    }

    // gh writes this to stderr on older versions and stdout on newer ones.
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // "Logged in to github.com account NAME (...)" on current versions, "as NAME" on older ones.
    for line in text.lines() {
        for marker in ["account ", " as "] {
            if let Some(at) = line.find(marker) {
                let rest = line[at + marker.len()..].trim();
                let who = rest
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_');

                if !who.is_empty() {
                    return SignIn::Ready { who: who.to_owned(), on_path: gh.on_path };
                }
            }
        }
    }

    SignIn::Ready { who: "an account it did not name".to_owned(), on_path: gh.on_path }
}

/// Installs a missing prerequisite with the platform's package manager.
///
/// Done rather than instructed, deliberately. The people this project needs are not all
/// comfortable in a terminal, and "install the GitHub CLI" has cost more contributor-hours here
/// than any search has saved. Only ever the two tools this project actually requires, only
/// through the official package id, and `--no-install` turns it off for anybody who would rather
/// manage their own.
pub fn install(package: &str, tool: &str) -> bool {
    if !cfg!(windows) {
        println!(
            "  [....]    {tool} is missing. Install it with your package manager and run this \
             again."
        );
        return false;
    }

    println!("  [....]    {tool} is missing -- installing it with winget (this takes a minute)");

    let installed = std::process::Command::new("winget")
        .args([
            "install",
            "--id",
            package,
            "-e",
            "--source",
            "winget",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
        ])
        .status()
        .map(|status| status.success())
        .unwrap_or(false);

    if !installed {
        println!("  [....]    winget could not install {tool}; falling back to instructions");
    }

    installed
}

#[cfg(test)]
mod tests {
    /// The repository submissions go to is also the one whose open pull requests are read, since
    /// a duplicate is only a duplicate against the place it is being sent.
    #[test]
    fn the_submit_repo_is_the_repo_watched_for_duplicates() {
        assert_eq!(super::DEFAULT_REPO, "KingslayerKyle/hash-slinging-slasher");
    }
}
