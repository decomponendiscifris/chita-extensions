//! The Git extension: built on the same API as every extension (`chita-ext-sdk`). It runs
//! the `git` on this Mac through Chita, in the folder being viewed, and shows what it says:
//! a column with each item's state, a panel with the branch and the last commit, and
//! commands to stage, unstage, commit, and see changes and history.

use chita_ext_sdk::{
    ColumnArgument, CommandArgument, CommandResult, Panel, PanelArgument, Prompt, ToolOutput,
};

pub mod status;
use status::Status;

chita_ext_sdk::extension! {
    command: run_command,
    column: column_texts,
    panel: show_panel,
}

/// Runs git in the viewed folder.
fn git(args: &[&str]) -> Result<ToolOutput, String> {
    chita_ext_sdk::run_tool("git", args)
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

/// The selected items as names relative to the viewed folder, for git's arguments.
fn names(folder: Option<&str>, selection: &[String]) -> Vec<String> {
    let Some(folder) = folder else {
        return Vec::new();
    };
    let folder = folder.trim_end_matches('/');
    selection
        .iter()
        .filter_map(|path| {
            path.strip_prefix(folder)
                .and_then(|r| r.strip_prefix('/'))
                .filter(|n| !n.is_empty())
                .map(str::to_owned)
        })
        .collect()
}

/// What a git that didn't succeed said, for a message.
fn failure(out: &ToolOutput) -> String {
    let said = out.stderr.trim();
    if said.is_empty() {
        format!("git ended with status {}", out.status)
    } else {
        said.lines().take(3).collect::<Vec<_>>().join("\n")
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
        Ok(out) if out.succeeded() => {
            // Push and fetch report on stderr even when all went well.
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
        Ok(out) => {
            let mut text = failure(&out);
            if needs_sign_in(&out.stderr) {
                text.push_str(
                    "\n\nSign in again from Terminal (git push, or gh auth login): Chita never \
                     keeps your passwords, it uses what git and your SSH agent already have.",
                );
            }
            CommandResult::message(text)
        }
        Err(reason) => CommandResult::message(reason),
    }
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

pub fn run_command(argument: CommandArgument) -> CommandResult {
    let names = names(argument.folder.as_deref(), &argument.selection);
    match argument.command.as_str() {
        "stage" => act(&["add"], &names),
        "unstage" => act(&["restore", "--staged"], &names),
        "commit" => match argument.input.as_deref().map(str::trim) {
            None | Some("") => CommandResult::prompt(Prompt {
                title: "Commit".into(),
                message: "Commits what is staged.".into(),
                placeholder: "What changed, and why".into(),
                action: "Commit".into(),
            }),
            Some(message) => match git(&["commit", "-m", message]) {
                Ok(out) if out.succeeded() => CommandResult::message(
                    out.stdout.lines().next().unwrap_or("Committed.").to_owned(),
                ),
                Ok(out) => CommandResult::message(failure(&out)),
                Err(reason) => CommandResult::message(reason),
            },
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
        "push" => reach(&["push"]),
        other => CommandResult::message(format!("The Git extension has no command {other}.")),
    }
}

pub fn column_texts(argument: ColumnArgument) -> Vec<String> {
    match repository() {
        Some((status, prefix)) => status.column(&argument.folder, &prefix, &argument.paths),
        None => argument.paths.iter().map(|_| String::new()).collect(),
    }
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
