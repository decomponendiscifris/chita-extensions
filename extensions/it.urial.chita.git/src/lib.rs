//! The Git extension: built on the same API as every extension (`chita-ext-sdk`). It runs
//! the `git` on this Mac through Chita, in the folder being viewed, and shows what it says:
//! a repository as a place (the header with the branch and the state, the Changes and
//! Branches screens, the diff of what's selected in the Inspector), a column with each
//! item's state, a panel, and commands to commit, fetch, pull, push and keep `.gitignore`.

use chita_ext_sdk::{
    ColumnArgument, ColumnCell, CommandArgument, CommandResult, Context, ContextArgument, Panel,
    PanelArgument, Prompt, Screen, ScreenArgument, ToolOutput,
};

pub mod flow;
pub mod history;
pub mod ignore;
pub mod screens;
pub mod status;
pub mod templates;
use flow::{CommitStyle, Conventions, FlowBranch, Kind};
use history::{Commit, Submodule, Tag, Worktree};
use screens::{Branch, DISCARD_KEEP_SECONDS, DISCARD_PREFIX, InProgress, Stash};
use status::Status;

thread_local! {
    /// How many commits History shows; Show Older Commits doubles it (the module's memory
    /// lives as long as the host does).
    static HISTORY_LIMIT: std::cell::Cell<usize> = const { std::cell::Cell::new(200) };
}

chita_ext_sdk::extension! {
    command: run_command,
    cells: column_cells,
    panel: show_panel,
    context: describe_context,
    screen: show_screen,
}

/// Runs git in the viewed folder.
fn git(args: &[&str]) -> Result<ToolOutput, String> {
    chita_ext_sdk::run_tool("git", args)
}

/// Git's output when it succeeded, or why not.
fn git_ok(args: &[&str]) -> Result<String, String> {
    match git(args) {
        Ok(out) if out.succeeded() => Ok(out.stdout),
        Ok(out) => Err(failure(&out)),
        Err(reason) => Err(reason),
    }
}

/// The folder's status, and its path below the repository's root ("" at the root); `None`
/// when it isn't in a repository.
fn repository() -> Option<(Status, String)> {
    let prefix = git(&["rev-parse", "--show-prefix"]).ok()?;
    if !prefix.succeeded() {
        return None;
    }
    let out = git(&[
        "status",
        "--porcelain=v2",
        "-z",
        "--branch",
        "--untracked-files=normal",
        "--ignored=matching",
    ])
    .ok()?;
    if !out.succeeded() {
        return None;
    }
    Some((
        Status::parse(out.stdout.as_bytes()),
        prefix.stdout.trim_end_matches(['\n', '\r']).to_owned(),
    ))
}

/// The repository's root, from the viewed folder and its prefix below the root.
fn root_of(folder: &str, prefix: &str) -> String {
    let folder = folder.trim_end_matches('/');
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() {
        folder.to_owned()
    } else {
        folder
            .strip_suffix(prefix)
            .map(|r| r.trim_end_matches('/').to_owned())
            .unwrap_or_else(|| folder.to_owned())
    }
}

/// A merge, rebase, cherry-pick or revert that stopped, when one did.
fn in_progress() -> Option<InProgress> {
    let present =
        |name: &str| git(&["rev-parse", "-q", "--verify", name]).is_ok_and(|out| out.succeeded());
    if present("REBASE_HEAD") {
        Some(InProgress::Rebase)
    } else if present("MERGE_HEAD") {
        Some(InProgress::Merge)
    } else if present("CHERRY_PICK_HEAD") {
        Some(InProgress::CherryPick)
    } else if present("REVERT_HEAD") {
        Some(InProgress::Revert)
    } else {
        None
    }
}

/// The stashes, newest first; discarded changes older than a week are dropped first.
fn stashes() -> Vec<Stash> {
    let list = |out: Result<ToolOutput, String>| match out {
        Ok(out) if out.succeeded() => Stash::parse(&out.stdout),
        _ => Vec::new(),
    };
    let format = "--format=%gd%x1f%gs%x1f%cr%x1f%ct";
    let mut stashes = list(git(&["stash", "list", format]));
    // The clock, from git: the committer time it would stamp now.
    let now = git_ok(&["var", "GIT_COMMITTER_IDENT"])
        .ok()
        .and_then(|ident| {
            ident
                .split_whitespace()
                .rev()
                .nth(1)
                .and_then(|t| t.parse::<i64>().ok())
        })
        .unwrap_or(0);
    let expired: Vec<String> = stashes
        .iter()
        .filter(|s| s.is_discard() && now > 0 && now - s.time > DISCARD_KEEP_SECONDS)
        .map(|s| s.id.clone())
        .collect();
    if !expired.is_empty() {
        // Highest first: dropping one renumbers the ones after it.
        for id in expired.iter().rev() {
            let _ = git(&["stash", "drop", "-q", id]);
        }
        stashes = list(git(&["stash", "list", format]));
    }
    stashes
}

/// The selected items (or a screen's rows) as names relative to the viewed folder, for
/// git's arguments.
fn names(folder: Option<&str>, paths: &[String]) -> Vec<String> {
    let Some(folder) = folder else {
        return Vec::new();
    };
    let folder = folder.trim_end_matches('/');
    paths
        .iter()
        .filter_map(|path| {
            path.strip_prefix(folder)
                .and_then(|r| r.strip_prefix('/'))
                .filter(|n| !n.is_empty())
                .map(str::to_owned)
        })
        .collect()
}

/// What a git that didn't succeed said, for a message: its error output, or its last lines
/// (a merge reports its conflicts there), or just the status.
fn failure(out: &ToolOutput) -> String {
    let said = out.stderr.trim();
    if !said.is_empty() {
        return said.lines().take(3).collect::<Vec<_>>().join("\n");
    }
    let printed: Vec<&str> = out.stdout.trim().lines().rev().take(3).collect();
    if printed.is_empty() {
        format!("git ended with status {}", out.status)
    } else {
        printed.into_iter().rev().collect::<Vec<_>>().join("\n")
    }
}

/// Runs git with `args` then the names; a message when it fails, nothing when it succeeds.
fn act(args: &[&str], names: &[String]) -> CommandResult {
    let mut all: Vec<&str> = args.to_vec();
    all.push("--");
    all.extend(names.iter().map(String::as_str));
    match git(&all) {
        Ok(out) if out.succeeded() => CommandResult::nothing(),
        Ok(out) => CommandResult::message(failure(&out)),
        Err(reason) => CommandResult::message(reason),
    }
}

/// Runs git with `args` (no names); a message when it fails, nothing when it succeeds.
fn plain(args: &[&str]) -> CommandResult {
    match git(args) {
        Ok(out) if out.succeeded() => CommandResult::nothing(),
        Ok(out) => CommandResult::message(failure(&out)),
        Err(reason) => CommandResult::message(reason),
    }
}

/// Runs git with `args` then the names; its output, or a message when there is none.
fn show(args: &[&str], names: &[String], nothing: &str) -> CommandResult {
    let mut all: Vec<&str> = args.to_vec();
    all.push("--");
    all.extend(names.iter().map(String::as_str));
    match git(&all) {
        Ok(out) if out.succeeded() => {
            if out.stdout.trim().is_empty() {
                CommandResult::message(nothing)
            } else {
                CommandResult::output(out.stdout)
            }
        }
        Ok(out) => CommandResult::message(failure(&out)),
        Err(reason) => CommandResult::message(reason),
    }
}

/// Fetch, pull or push, with the repository's own remotes (the rules allow no URL): what git
/// said, and when it couldn't sign in, what to do about it.
fn reach(args: &[&str]) -> CommandResult {
    match git(args) {
        Ok(out) if out.succeeded() => reach_said(&out),
        Ok(out) => reach_failure(&out),
        Err(reason) => CommandResult::message(reason),
    }
}

/// What a network command that succeeded said (push and fetch report on stderr even when
/// all went well).
fn reach_said(out: &ToolOutput) -> CommandResult {
    let said = out.stderr.trim();
    let said = if said.is_empty() {
        out.stdout.trim()
    } else {
        said
    };
    let last: Vec<&str> = said.lines().rev().take(3).collect();
    if last.is_empty() {
        CommandResult::message("Done.")
    } else {
        CommandResult::message(last.into_iter().rev().collect::<Vec<_>>().join("\n"))
    }
}

/// What a network command that failed said, and what to do when it couldn't sign in.
fn reach_failure(out: &ToolOutput) -> CommandResult {
    let mut text = failure(out);
    if needs_sign_in(&out.stderr) {
        text.push_str(
            "\n\nSign in again from Terminal (git push, or gh auth login): Chita never \
             keeps your passwords, it uses what git and your SSH agent already have.",
        );
    }
    if out.stderr.contains("gpg failed") || out.stderr.contains("signing failed") {
        text.push_str("\n\nSigning failed: GPG asks for its passphrase in its own window, not Chita\u{2019}s. Unlock the key from Terminal first, or sign with an SSH key.");
    }
    CommandResult::message(text)
}

/// Git couldn't sign in: a missing or expired credential, a locked key.
fn needs_sign_in(stderr: &str) -> bool {
    [
        "Authentication failed",
        "Permission denied",
        "could not read Username",
        "could not read Password",
        "terminal prompts disabled",
        "Invalid username or token",
        "Host key verification failed",
    ]
    .iter()
    .any(|s| stderr.contains(s))
}

// MARK: Commands

/// The items a command applies to: a screen's rows when it came from one, else the
/// selection.
fn targets(argument: &CommandArgument) -> Vec<String> {
    if argument.rows.is_empty() {
        argument.selection.clone()
    } else {
        argument.rows.clone()
    }
}

pub fn run_command(argument: CommandArgument) -> CommandResult {
    let folder = argument.folder.as_deref();
    let targets = targets(&argument);
    let names = names(folder, &targets);
    let input = argument
        .input
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    match argument.command.as_str() {
        "stage" => act(
            &["add"],
            &if names.is_empty() {
                vec![".".into()]
            } else {
                names
            },
        ),
        "unstage" => act(
            &["restore", "--staged"],
            &if names.is_empty() {
                vec![".".into()]
            } else {
                names
            },
        ),
        "resolve" => act(
            &["add"],
            &if names.is_empty() {
                vec![".".into()]
            } else {
                names
            },
        ),
        "discard" => discard(&names, &targets),
        "undo-discard" => undo_discard(),
        "commit" => commit(input),
        "identity-name" => identity("user.name", input, "commit"),
        "identity-email" => identity("user.email", input, "commit"),
        "amend" => amend(input),
        "continue" | "abort" => match in_progress() {
            Some(what) => plain(&[what.subcommand(), &format!("--{}", argument.command)]),
            None => CommandResult::message("Nothing is in progress."),
        },
        "stash-save" => match input {
            None => CommandResult::prompt(
                Prompt::line(
                    "Stash",
                    "Puts every change away, to bring back later.",
                    "Stash",
                )
                .placeholder("What it is (optional)"),
            ),
            Some(message) => plain(&["stash", "push", "-u", "-m", message]),
        },
        "stash-pop" | "stash-apply" | "stash-drop" => {
            let Some(id) = targets.first() else {
                return CommandResult::message("Choose a stash first.");
            };
            let verb = &argument.command["stash-".len()..];
            if verb == "drop" {
                plain(&["stash", "drop", "-q", id])
            } else {
                plain(&["stash", verb, id])
            }
        }
        "checkout" => match targets.first().and_then(|id| id.split_once(':')) {
            Some(("remote", name)) => {
                let local = name.split_once('/').map(|(_, b)| b).unwrap_or(name);
                plain(&["switch", local])
            }
            Some(("tag", name)) => plain(&["switch", "--detach", name]),
            Some((_, name)) => plain(&["switch", name]),
            None => CommandResult::message("Choose a branch first."),
        },
        "new-branch" => match input {
            None => CommandResult::prompt(
                Prompt::line(
                    "New Branch",
                    "Made from the current commit, and checked out.",
                    "Create",
                )
                .placeholder("feature/name"),
            ),
            Some(name) => plain(&["switch", "-c", name]),
        },
        "delete-branch" => match targets.first().and_then(|id| id.strip_prefix("local:")) {
            Some(name) => match git(&["branch", "-d", name]) {
                Ok(out) if out.succeeded() => CommandResult::nothing(),
                Ok(out) if out.stderr.contains("not fully merged") => {
                    CommandResult::message(format!(
                        "{name} has commits that aren\u{2019}t merged anywhere. Merge it first, or delete it from Terminal with git branch -D."
                    ))
                }
                Ok(out) => CommandResult::message(failure(&out)),
                Err(reason) => CommandResult::message(reason),
            },
            None => CommandResult::message("Choose a branch first."),
        },
        "merge" => match targets.first().and_then(|id| id.split_once(':')) {
            Some((_, name)) => plain(&["merge", "--no-edit", name]),
            None => CommandResult::message("Choose a branch first."),
        },
        "diff" => diff(folder, &targets),
        "history-more" => {
            HISTORY_LIMIT.with(|l| l.set(l.get() * 2));
            CommandResult::nothing()
        }
        "commit-show" => match targets.first() {
            Some(hash) => match git_ok(&["show", "--stat", "--patch", "--format=%h %s%n%an <%ae> \u{00B7} %cd%n%n%b", hash]) {
                Ok(text) => CommandResult::diff(hash.chars().take(7).collect::<String>(), text),
                Err(reason) => CommandResult::message(reason),
            },
            None => CommandResult::nothing(),
        },
        "checkout-commit" => match targets.first() {
            Some(hash) => plain(&["switch", "--detach", hash]),
            None => CommandResult::message("Choose a commit first."),
        },
        "branch-here" => match (targets.first(), input) {
            (Some(_), None) => CommandResult::prompt(
                Prompt::line("New Branch Here", "Made from this commit, and checked out.", "Create")
                    .placeholder("feature/name"),
            ),
            (Some(hash), Some(name)) => plain(&["switch", "-c", name, hash]),
            (None, _) => CommandResult::message("Choose a commit first."),
        },
        "cherry-pick" => match targets.first() {
            Some(hash) => plain(&["cherry-pick", hash]),
            None => CommandResult::message("Choose a commit first."),
        },
        "revert" => match targets.first() {
            Some(hash) => plain(&["revert", "--no-edit", hash]),
            None => CommandResult::message("Choose a commit first."),
        },
        "tag-new" | "tag-here" => match input {
            None => CommandResult::prompt(
                Prompt::line(
                    "New Tag",
                    if argument.command == "tag-here" { "On this commit." } else { "On the current commit." },
                    "Tag",
                )
                .placeholder("v1.0"),
            ),
            Some(name) => {
                let mut args = vec!["tag", "-a", name, "-m", name];
                if argument.command == "tag-here"
                    && let Some(hash) = targets.first()
                {
                    args.push(hash);
                }
                plain(&args)
            }
        },
        "tag-delete" => match targets.first().and_then(|id| id.strip_prefix("tag:")) {
            Some(name) => plain(&["tag", "-d", name]),
            None => CommandResult::message("Choose a tag first."),
        },
        "worktree-add" => match input {
            None => CommandResult::prompt(
                Prompt::line(
                    "Add Worktree",
                    "A branch checked out in a folder of its own, next to this repository (../<repository>-<branch>). A branch that doesn\u{2019}t exist is made.",
                    "Add",
                )
                .placeholder("feature/name"),
            ),
            Some(branch) => {
                let folder = argument.folder.as_deref().unwrap_or("");
                let name = folder.trim_end_matches('/').rsplit('/').next().unwrap_or("repo");
                let leaf = branch.replace('/', "-");
                let path = format!("../{name}-{leaf}");
                let exists = git(&["rev-parse", "-q", "--verify", &format!("refs/heads/{branch}")])
                    .is_ok_and(|out| out.succeeded());
                if exists {
                    plain(&["worktree", "add", &path, branch])
                } else {
                    plain(&["worktree", "add", "-b", branch, &path])
                }
            }
        },
        "worktree-remove" => match targets.first() {
            Some(path) => {
                // By its path next to the repository (git reports it resolved, the viewed
                // folder may not be: matched by the parent's name), or inside it.
                let folder = argument.folder.as_deref().unwrap_or("").trim_end_matches('/');
                let path = path.trim_end_matches('/');
                let (path_parent, leaf) = path.rsplit_once('/').unwrap_or(("", path));
                let same_parent = |a: &str, b: &str| {
                    a.rsplit('/').next().unwrap_or(a) == b.rsplit('/').next().unwrap_or(b)
                };
                let folder_parent = folder.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
                let relative = if let Some(inside) = path.strip_prefix(&format!("{folder}/")) {
                    Some(inside.to_owned())
                } else if !leaf.is_empty() && same_parent(path_parent, folder_parent) {
                    Some(format!("../{leaf}"))
                } else {
                    None
                };
                match relative {
                    Some(relative) => plain(&["worktree", "remove", &relative]),
                    None => CommandResult::message(
                        "That worktree isn\u{2019}t next to this repository; remove it from Terminal.",
                    ),
                }
            }
            None => CommandResult::message("Choose a worktree first."),
        },
        "branch-log" => match targets.first().and_then(|id| id.split_once(':')) {
            Some((_, name)) => {
                match git_ok(&["log", "--format=%h  %s  (%cr, %an)", "-n", "40", name]) {
                    Ok(text) if text.trim().is_empty() => CommandResult::message("No commits."),
                    Ok(text) => CommandResult {
                        output: Some(text),
                        title: Some(name.to_owned()),
                        ..Default::default()
                    },
                    Err(reason) => CommandResult::message(reason),
                }
            }
            None => CommandResult::nothing(),
        },
        "changes" => {
            // Everything since the last commit, staged or not; before the first commit,
            // what's staged.
            let head = show(&["diff", "HEAD"], &names, "No changes.");
            if head.message.as_deref().is_some_and(|m| m.contains("HEAD")) {
                show(&["diff", "--cached"], &names, "No changes.")
            } else {
                head
            }
        }
        "history" => show(&["log", "--oneline", "-n", "50"], &names, "No commits yet."),
        "fetch" => reach(&["fetch"]),
        "pull" => reach(&["pull"]),
        "push" => push(),
        "force-push" => reach(&["push", "--force-with-lease"]),
        "rebase" => match targets.first().and_then(|id| id.split_once(':')) {
            Some((_, name)) => plain(&["rebase", name]),
            None => CommandResult::message("Choose a branch first."),
        },
        "file-history" => {
            if names.is_empty() {
                return CommandResult::message("Select a file first.");
            }
            let mut args = vec!["log", "--format=%h  %s  (%cr, %an)", "--follow", "-n", "100", "--"];
            args.push(&names[0]);
            match git_ok(&args) {
                Ok(text) if text.trim().is_empty() => CommandResult::message("Not in any commit yet."),
                Ok(text) => CommandResult {
                    output: Some(text),
                    title: Some(format!("History of {}", names[0])),
                    ..Default::default()
                },
                Err(reason) => CommandResult::message(reason),
            }
        }
        "clone" => match chita_ext_sdk::clone_repository() {
            Ok(out) if out.succeeded() => CommandResult::message("Cloned."),
            Ok(out) => reach_failure(&out),
            Err(reason) if reason == "cancelled" => CommandResult::nothing(),
            Err(reason) => CommandResult::message(reason),
        },
        "init" => {
            if repository().is_some() {
                return CommandResult::message("This folder is in a repository already.");
            }
            match git(&["init", "-q", "-b", "main"]) {
                Ok(out) if out.succeeded() => CommandResult::message("A repository, on the branch main."),
                Ok(out) => CommandResult::message(failure(&out)),
                Err(reason) => CommandResult::message(reason),
            }
        }
        "submodule-update" => {
            let mut args = vec!["submodule", "update", "--init", "--recursive"];
            if !targets.is_empty() {
                args.push("--");
                args.extend(names.iter().map(String::as_str));
            }
            match git(&args) {
                Ok(out) if out.succeeded() => CommandResult::nothing(),
                Ok(out) => reach_failure(&out),
                Err(reason) => CommandResult::message(reason),
            }
        }
        "flow-setup" => flow_setup(),
        "flow-start-feature" | "flow-start-release" | "flow-start-hotfix" => {
            let kind = match argument.command.as_str() {
                "flow-start-feature" => Kind::Feature,
                "flow-start-release" => Kind::Release,
                _ => Kind::Hotfix,
            };
            match input {
                None => CommandResult::prompt(
                    Prompt::line(
                        format!("Start a {}", kind.word()),
                        format!("A branch from {}, checked out.", kind.base(&conventions())),
                        "Start",
                    )
                    .placeholder(match kind {
                        Kind::Feature => "name",
                        Kind::Release | Kind::Hotfix => "1.2.0",
                    }),
                ),
                Some(name) => flow_start(kind, name),
            }
        }
        "flow-finish" => match targets.first().and_then(|id| id.strip_prefix("local:")) {
            Some(name) => flow_finish(name),
            None => CommandResult::message("Choose a branch first."),
        },
        "flow-publish" => match targets.first().and_then(|id| id.strip_prefix("local:")) {
            Some(name) => reach(&["push", "-u", "origin", name]),
            None => CommandResult::message("Choose a branch first."),
        },
        "flow-style-plain" | "flow-style-imperative" | "flow-style-conventional" => {
            let style = &argument.command["flow-style-".len()..];
            match git_ok(&["config", "--local", "chita.commits", style]) {
                Ok(_) => CommandResult::nothing(),
                Err(reason) => CommandResult::message(reason),
            }
        }
        "ignore" => ignore(&names, &targets, ".gitignore"),
        "ignore-local" => ignore(&names, &targets, ".git/info/exclude"),
        "ignore-ext" => {
            let lines: Vec<String> = names
                .iter()
                .filter_map(|n| ignore::pattern_for_kind(n))
                .collect();
            if lines.is_empty() {
                return CommandResult::message("The selected items have no extension to ignore.");
            }
            add_ignore_lines(".gitignore", &lines)
        }
        "untrack" => {
            let mut all = vec!["rm", "-r", "--cached", "-q", "--"];
            all.extend(names.iter().map(String::as_str));
            match git(&all) {
                Ok(out) if out.succeeded() => ignore(&names, &targets, ".gitignore"),
                Ok(out) => CommandResult::message(failure(&out)),
                Err(reason) => CommandResult::message(reason),
            }
        }
        "edit-ignore" => match input {
            None => {
                let current = chita_ext_sdk::read_file(".gitignore")
                    .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                    .unwrap_or_default();
                let mut prompt = Prompt::line(
                    ".gitignore",
                    "One pattern per line: a name, /name for this folder only, name/ for a folder, *.ext for a kind of file, # for a note. Insert adds a template at the end.",
                    "Save",
                )
                .editor(current);
                for (title, text) in templates::TEMPLATES {
                    prompt = prompt.insert(*title, *text);
                }
                CommandResult::prompt(prompt)
            }
            Some(_) => {
                let text = argument.input.unwrap_or_default();
                let text = if text.ends_with('\n') {
                    text
                } else {
                    format!("{text}\n")
                };
                match chita_ext_sdk::write_file(".gitignore", text.as_bytes()) {
                    Ok(()) => CommandResult::nothing(),
                    Err(reason) => CommandResult::message(reason),
                }
            }
        },
        other => CommandResult::message(format!("The Git extension has no command {other}.")),
    }
}

/// Discards the changes to `names`: put away in a stash named after them (kept a week,
/// brought back by Undo Discard), which also takes them out of the working tree.
fn discard(names: &[String], targets: &[String]) -> CommandResult {
    let what = if names.is_empty() {
        "everything".to_owned()
    } else {
        let shown: Vec<&str> = targets
            .iter()
            .map(|t| t.rsplit('/').next().unwrap_or(t))
            .take(5)
            .collect();
        let mut text = shown.join(", ");
        if targets.len() > 5 {
            text.push_str(&format!(" and {} more", targets.len() - 5));
        }
        text
    };
    let message = format!("{DISCARD_PREFIX}{what}");
    let mut all = vec!["stash", "push", "-u", "-q", "-m", &message, "--"];
    if names.is_empty() {
        all.push(".");
    } else {
        all.extend(names.iter().map(String::as_str));
    }
    plain(&all)
}

/// Brings back what Discard put away last.
fn undo_discard() -> CommandResult {
    match stashes().into_iter().find(Stash::is_discard) {
        Some(stash) => match git(&["stash", "pop", "-q", &stash.id]) {
            Ok(out) if out.succeeded() => CommandResult::nothing(),
            Ok(out) => CommandResult::message(format!(
                "{}\n\nThe discarded changes are still in the stash {}.",
                failure(&out),
                stash.id
            )),
            Err(reason) => CommandResult::message(reason),
        },
        None => CommandResult::message("Nothing was discarded lately."),
    }
}

/// Commit…: the name and address first when the repository has none, then the message.
fn commit(input: Option<&str>) -> CommandResult {
    match input {
        None => {
            if let Some(prompt) = identity_missing() {
                return CommandResult::prompt(prompt);
            }
            CommandResult::prompt(
                Prompt::line("Commit", "Commits what is staged.", "Commit")
                    .placeholder("What changed, and why"),
            )
        }
        Some(message) => {
            if let Some(problem) = commit_style().check(message) {
                return CommandResult::prompt(
                    Prompt::line("Commit", problem, "Commit")
                        .placeholder("What changed, and why")
                        .editor(message),
                );
            }
            match git(&["commit", "-m", message]) {
                Ok(out) if out.succeeded() => CommandResult::message(
                    out.stdout.lines().next().unwrap_or("Committed.").to_owned(),
                ),
                Ok(out) => CommandResult::message(failure(&out)),
                Err(reason) => CommandResult::message(reason),
            }
        }
    }
}

/// The commit message convention the repository asks for.
fn commit_style() -> CommitStyle {
    git_ok(&["config", "--get", "chita.commits"])
        .map(|v| CommitStyle::parse(&v))
        .unwrap_or_default()
}

/// Pushes; a branch with no upstream yet gets one on origin.
fn push() -> CommandResult {
    match git(&["push"]) {
        Ok(out) if !out.succeeded() && out.stderr.contains("has no upstream branch") => {
            let branch = repository()
                .and_then(|(s, _)| s.branch.map(|b| b.head))
                .unwrap_or_default();
            if branch.is_empty() || branch == "(detached)" {
                return CommandResult::message(failure(&out));
            }
            reach(&["push", "-u", "origin", &branch])
        }
        Ok(out) if out.succeeded() => reach_said(&out),
        Ok(out) => reach_failure(&out),
        Err(reason) => CommandResult::message(reason),
    }
}

/// The flow's conventions from the repository's configuration.
fn conventions() -> Conventions {
    let configured = git_ok(&["config", "--get-regexp", "^gitflow\\."]).unwrap_or_default();
    let exists = |name: &str| {
        git(&["rev-parse", "-q", "--verify", &format!("refs/heads/{name}")])
            .is_ok_and(|o| o.succeeded())
    };
    let main = if exists("main") {
        "main"
    } else if exists("master") {
        "master"
    } else {
        "main"
    };
    Conventions::parse(&configured, main)
}

/// Writes the flow's conventions into the repository, and makes the develop branch when
/// it's missing.
fn flow_setup() -> CommandResult {
    let c = conventions();
    for (key, value) in c.settings() {
        if let Err(reason) = git_ok(&["config", "--local", key, &value]) {
            return CommandResult::message(reason);
        }
    }
    let develop_exists = git(&[
        "rev-parse",
        "-q",
        "--verify",
        &format!("refs/heads/{}", c.develop),
    ])
    .is_ok_and(|o| o.succeeded());
    if !develop_exists && let Err(reason) = git_ok(&["branch", &c.develop, &c.master]) {
        return CommandResult::message(reason);
    }
    CommandResult::nothing()
}

fn flow_start(kind: Kind, name: &str) -> CommandResult {
    let c = conventions();
    let branch = format!("{}{}", kind.prefix(&c), name);
    plain(&["switch", "-c", &branch, kind.base(&c)])
}

/// Finishes a flow branch: a feature merges into develop; a release or a hotfix merges
/// into master, is tagged, and merges into develop; the branch goes.
fn flow_finish(name: &str) -> CommandResult {
    let c = conventions();
    let Some((kind, short)) = c.kind_of(name) else {
        return CommandResult::message(format!(
            "{name} isn't a feature, release or hotfix branch."
        ));
    };
    let step = |args: &[&str]| -> Result<(), String> {
        match git(args) {
            Ok(out) if out.succeeded() => Ok(()),
            Ok(out) => Err(format!(
                "{}\n\nStopped at git {}.",
                failure(&out),
                args.join(" ")
            )),
            Err(reason) => Err(reason),
        }
    };
    let result = (|| {
        match kind {
            Kind::Feature => {
                step(&["switch", &c.develop])?;
                step(&["merge", "--no-ff", "--no-edit", name])?;
            }
            Kind::Release | Kind::Hotfix => {
                let tag = format!("{}{}", c.versiontag, short);
                step(&["switch", &c.master])?;
                step(&["merge", "--no-ff", "--no-edit", name])?;
                step(&["tag", "-a", &tag, "-m", &tag])?;
                step(&["switch", &c.develop])?;
                step(&["merge", "--no-ff", "--no-edit", &c.master])?;
            }
        }
        step(&["branch", "-d", name])
    })();
    match result {
        Ok(()) => CommandResult::nothing(),
        Err(message) => CommandResult::message(message),
    }
}

/// Amend…: the last commit again with a new message (what's staged goes in too).
fn amend(input: Option<&str>) -> CommandResult {
    match input {
        None => {
            let last = git_ok(&["log", "-1", "--format=%s"]).unwrap_or_default();
            CommandResult::prompt(
                Prompt::line(
                    "Amend",
                    "Replaces the last commit with its message changed and what\u{2019}s staged added. Not for commits already pushed.",
                    "Amend",
                )
                .placeholder("The message")
                .editor(last.trim_end()),
            )
        }
        Some(message) => plain(&["commit", "--amend", "-m", message]),
    }
}

/// The prompt for what the repository lacks about the user, when it lacks something.
fn identity_missing() -> Option<Prompt> {
    let missing =
        |key: &str| git(&["config", "--get", key]).is_ok_and(|out| out.stdout.trim().is_empty());
    if missing("user.name") {
        Some(
            Prompt::line(
                "Your name for commits",
                "This repository doesn\u{2019}t know who commits. The name goes into every commit you make here.",
                "Next",
            )
            .placeholder("Name Surname")
            .next("identity-name"),
        )
    } else if missing("user.email") {
        Some(
            Prompt::line(
                "Your email for commits",
                "The address goes into every commit you make here.",
                "Save",
            )
            .placeholder("you@example.com")
            .next("identity-email"),
        )
    } else {
        None
    }
}

/// Sets `key` for the repository from the prompt's answer, then goes on to `then`.
fn identity(key: &str, input: Option<&str>, then: &str) -> CommandResult {
    let Some(value) = input else {
        return CommandResult::nothing();
    };
    if let Err(reason) = git_ok(&["config", "--local", key, value]) {
        return CommandResult::message(reason);
    }
    match identity_missing() {
        Some(prompt) => CommandResult::prompt(prompt),
        None => match then {
            "commit" => commit(None),
            _ => CommandResult::nothing(),
        },
    }
}

/// What the Inspector shows for the rows selected in Changes: the diff of files, a stash's
/// diff, nothing for a mix.
fn diff(folder: Option<&str>, targets: &[String]) -> CommandResult {
    let Some(first) = targets.first() else {
        return CommandResult::nothing();
    };
    if first.starts_with("stash@") {
        return match git_ok(&["stash", "show", "-p", "--include-untracked", first]) {
            Ok(text) => CommandResult::diff(first.clone(), text),
            Err(reason) => CommandResult::message(reason),
        };
    }
    let names = names(folder, targets);
    if names.is_empty() {
        return CommandResult::nothing();
    }
    let title = if names.len() == 1 {
        names[0].clone()
    } else {
        format!("{} files", names.len())
    };
    // Everything since the last commit, staged or not (before the first commit, what's
    // staged), and untracked files whole.
    let mut text = String::new();
    let mut tracked = vec!["diff", "HEAD", "--"];
    tracked.extend(names.iter().map(String::as_str));
    match git(&tracked) {
        Ok(out) if out.succeeded() => text.push_str(&out.stdout),
        _ => {
            let mut cached = vec!["diff", "--cached", "--"];
            cached.extend(names.iter().map(String::as_str));
            if let Ok(out) = git(&cached) {
                text.push_str(&out.stdout);
            }
        }
    }
    for name in &names {
        if text.contains(&format!("diff --git a/{name}")) || text.contains(&format!(" b/{name}\n"))
        {
            continue;
        }
        if let Ok(bytes) = chita_ext_sdk::read_file(name) {
            if bytes.contains(&0) {
                text.push_str(&format!(
                    "diff --git a/{name} b/{name}\nBinary file {name} ({} bytes)\n",
                    bytes.len()
                ));
                continue;
            }
            let body = String::from_utf8_lossy(&bytes);
            let count = body.lines().count();
            text.push_str(&format!(
                "diff --git a/{name} b/{name}\nnew file\n--- /dev/null\n+++ b/{name}\n@@ -0,0 +1,{count} @@\n"
            ));
            for line in body.lines() {
                text.push('+');
                text.push_str(line);
                text.push('\n');
            }
        }
    }
    if text.trim().is_empty() {
        CommandResult::message("No changes.")
    } else {
        CommandResult::diff(title, text)
    }
}

/// Adds the patterns for `names` to `file` (`.gitignore` of the viewed folder, or the
/// repository's own exclude file when viewing its root).
fn ignore(names: &[String], targets: &[String], file: &str) -> CommandResult {
    let lines: Vec<String> = names
        .iter()
        .zip(targets)
        .map(|(name, target)| ignore::pattern_for(name, target.ends_with('/') || is_folder(name)))
        .collect();
    if lines.is_empty() {
        return CommandResult::message("Select something in the folder first.");
    }
    add_ignore_lines(file, &lines)
}

/// Whether `name` (relative to the viewed folder) is a folder, as the listing says.
fn is_folder(name: &str) -> bool {
    let (parent, leaf) = name.rsplit_once('/').unwrap_or((".", name));
    chita_ext_sdk::list_folder(parent)
        .map(|entries| entries.iter().any(|e| e.name == leaf && e.is_folder))
        .unwrap_or(false)
}

fn add_ignore_lines(file: &str, lines: &[String]) -> CommandResult {
    let current = chita_ext_sdk::read_file(file)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    let text = ignore::with_lines(&current, lines);
    if text == current {
        return CommandResult::nothing();
    }
    match chita_ext_sdk::write_file(file, text.as_bytes()) {
        Ok(()) => CommandResult::nothing(),
        Err(reason) => CommandResult::message(reason),
    }
}

// MARK: The column, the panel, the context, the screens

/// The Git column: each item's state as a cell with a mark in its colour; ignored items
/// dimmed; a submodule said so.
pub fn column_cells(argument: ColumnArgument) -> Vec<ColumnCell> {
    let Some((status, prefix)) = repository() else {
        return argument
            .paths
            .iter()
            .map(|_| ColumnCell::default())
            .collect();
    };
    let submodules: Vec<String> = if chita_ext_sdk::read_file(".gitmodules").is_ok() {
        git_ok(&["submodule", "status"])
            .map(|out| Submodule::parse(&out).into_iter().map(|s| s.path).collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let folder = argument.folder.trim_end_matches('/');
    status
        .column(&argument.folder, &prefix, &argument.paths)
        .into_iter()
        .zip(&argument.paths)
        .map(|(text, path)| {
            let relative = path
                .strip_prefix(folder)
                .and_then(|r| r.strip_prefix('/'))
                .map(|r| format!("{prefix}{r}"))
                .unwrap_or_default();
            if submodules.contains(&relative) {
                return ColumnCell::text(if text.is_empty() {
                    "Submodule".to_owned()
                } else {
                    format!("Submodule, {}", text.to_lowercase())
                })
                .symbol("square.stack.3d.up", "blue");
            }
            cell(text)
        })
        .collect()
}

/// A cell for a state's text.
fn cell(text: String) -> ColumnCell {
    let (symbol, color) = if text.is_empty() {
        return ColumnCell::default();
    } else if text == "Ignored" {
        return ColumnCell::text(text).dim();
    } else if text.starts_with("Conflict") {
        ("exclamationmark.triangle.fill", "red")
    } else if text.starts_with("Untracked") {
        ("questionmark.circle", "gray")
    } else if text.starts_with("Added") {
        ("plus.circle.fill", "green")
    } else if text.starts_with("Deleted") {
        ("minus.circle.fill", "red")
    } else if text.starts_with("Renamed") || text.starts_with("Copied") {
        ("arrow.right.circle.fill", "blue")
    } else if text.ends_with("changes") || text.ends_with("change") {
        ("circle.fill", "orange")
    } else {
        ("pencil.circle.fill", "orange")
    };
    ColumnCell::text(text).symbol(symbol, color)
}

pub fn show_panel(_argument: PanelArgument) -> Panel {
    let Some((status, _)) = repository() else {
        return Panel::default();
    };
    let Some(branch) = &status.branch else {
        return Panel::default();
    };
    let mut panel = Panel::default()
        .row("Branch", branch.head.clone())
        .row("Upstream", branch.upstream_text())
        .row("Changes", status.summary());
    if !branch.initial
        && let Ok(out) = git(&["log", "-1", "--format=%h%x1f%s%x1f%cr"])
        && out.succeeded()
    {
        let mut parts = out.stdout.trim().split('\u{1f}');
        let hash = parts.next().unwrap_or_default();
        // The first line only, and not all of a long one.
        let subject = parts.next().unwrap_or_default();
        let subject = if subject.chars().count() > 72 {
            format!("{}\u{2026}", subject.chars().take(71).collect::<String>())
        } else {
            subject.to_owned()
        };
        let when = parts.next().unwrap_or_default();
        panel = panel.row("Last commit", format!("{hash} {subject} \u{00B7} {when}"));
    }
    panel
}

pub fn describe_context(_argument: ContextArgument) -> Context {
    let Some((status, _)) = repository() else {
        return Context::inactive();
    };
    screens::context(&status, in_progress(), &stashes())
}

pub fn show_screen(argument: ScreenArgument) -> Screen {
    let Some((status, prefix)) = repository() else {
        return Screen::new().empty("Not a repository.");
    };
    match argument.screen.as_str() {
        "changes" => {
            let root = root_of(argument.folder.as_deref().unwrap_or(""), &prefix);
            screens::changes(&status, &root, in_progress(), &stashes())
        }
        "branches" => {
            let list = git_ok(&[
                "for-each-ref",
                // Tabs between the fields (for-each-ref knows no escapes): nothing but the
                // subject can hold one, and it's parsed from both ends.
                "--format=%(refname:short)\t%(HEAD)\t%(upstream:short)\t%(upstream:track)\t%(committerdate:relative)\t%(subject)\t%(refname)",
                "refs/heads",
                "refs/remotes",
            ])
            .map(|out| Branch::parse(&out))
            .unwrap_or_default();
            let mut screen = screens::branches(&list);
            let tags = git_ok(&[
                "for-each-ref",
                "--sort=-creatordate",
                "--format=%(refname:short)\t%(creatordate:relative)\t%(subject)",
                "refs/tags",
            ])
            .map(|out| Tag::parse(&out))
            .unwrap_or_default();
            if !tags.is_empty() {
                screen = screen.section(history::tags(&tags));
            }
            screen
        }
        "history" => {
            let limit = HISTORY_LIMIT.with(std::cell::Cell::get);
            let count = (limit + 1).to_string();
            let commits = git_ok(&[
                "log",
                "--format=%H%x1f%h%x1f%P%x1f%an%x1f%cr%x1f%D%x1f%s",
                "--date-order",
                "-n",
                &count,
                "--all",
            ])
            .map(|out| Commit::parse(&out))
            .unwrap_or_default();
            let more = commits.len() > limit;
            history::history(&commits[..commits.len().min(limit)], more)
        }
        "worktrees" => {
            let list = git_ok(&["worktree", "list", "--porcelain"])
                .map(|out| Worktree::parse(&out))
                .unwrap_or_default();
            let mut screen = history::worktrees(&list);
            if chita_ext_sdk::read_file(".gitmodules").is_ok() {
                let root = root_of(argument.folder.as_deref().unwrap_or(""), &prefix);
                let modules = git_ok(&["submodule", "status", "--recursive"])
                    .map(|out| Submodule::parse(&out))
                    .unwrap_or_default();
                screen = screen.section(history::submodules(&modules, &root));
                screen = screen.toolbar(
                    chita_ext_sdk::Action::new("submodule-update", "Update All Submodules")
                        .symbol("arrow.down.circle"),
                );
            }
            screen
        }
        "flow" => {
            let c = conventions();
            let list = git_ok(&[
                "for-each-ref",
                "--format=%(refname:short)\t%(HEAD)\t%(upstream:short)\t%(upstream:track)\t%(committerdate:relative)\t%(subject)\t%(refname)",
                "refs/heads",
            ])
            .map(|out| Branch::parse(&out))
            .unwrap_or_default();
            let branches: Vec<FlowBranch> = list
                .iter()
                .filter_map(|b| {
                    let (kind, short) = c.kind_of(&b.name)?;
                    Some(FlowBranch {
                        kind,
                        name: b.name.clone(),
                        short: short.to_owned(),
                        current: b.current,
                        upstream: b.upstream.clone(),
                        when: b.when.clone(),
                        subject: b.subject.clone(),
                    })
                })
                .collect();
            let develop_exists = list.iter().any(|b| b.name == c.develop);
            flow::flow(&c, &branches, commit_style(), develop_exists)
        }
        other => Screen::new().empty(format!("The Git extension has no screen {other}.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_comes_from_the_folder_and_the_prefix() {
        assert_eq!(root_of("/r/sub/deep", "sub/deep/"), "/r");
        assert_eq!(root_of("/r", ""), "/r");
        assert_eq!(root_of("/r/", ""), "/r");
    }
}
