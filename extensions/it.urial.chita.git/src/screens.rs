//! The header line and the screens: what Chita draws for a repository (the context), the
//! Changes screen (conflicts, staged, not staged, untracked, stashes) and the Branches
//! screen. Pure builders over what git said; the git runs are in `lib.rs`.

use crate::status::{Kind, Status};
use chita_ext_sdk::{Action, Context, Screen, ScreenRow, Section};

/// What's going on in the repository beyond the working tree: a merge, a rebase, a
/// cherry-pick or a revert that stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InProgress {
    Merge,
    Rebase,
    CherryPick,
    Revert,
}

impl InProgress {
    /// The git subcommand that continues or aborts it.
    pub fn subcommand(self) -> &'static str {
        match self {
            InProgress::Merge => "merge",
            InProgress::Rebase => "rebase",
            InProgress::CherryPick => "cherry-pick",
            InProgress::Revert => "revert",
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            InProgress::Merge => "merging",
            InProgress::Rebase => "rebasing",
            InProgress::CherryPick => "cherry-picking",
            InProgress::Revert => "reverting",
        }
    }
}

/// A stash as `git stash list` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stash {
    /// "stash@{0}".
    pub id: String,
    /// The message; "Discarded: …" for what Discard put away.
    pub subject: String,
    /// "2 hours ago".
    pub when: String,
    /// Seconds since 1970.
    pub time: i64,
}

impl Stash {
    /// Reads `stash list --format=%gd%x1f%gs%x1f%cr%x1f%ct`.
    pub fn parse(output: &str) -> Vec<Stash> {
        output
            .lines()
            .filter_map(|line| {
                let mut parts = line.split('\u{1f}');
                let id = parts.next()?.trim().to_owned();
                let subject = parts.next()?.trim().to_owned();
                let when = parts.next().unwrap_or_default().trim().to_owned();
                let time = parts
                    .next()
                    .and_then(|t| t.trim().parse().ok())
                    .unwrap_or(0);
                // "On main: message" and "WIP on main: …": the message.
                let subject = subject
                    .split_once(": ")
                    .map(|(_, m)| m.to_owned())
                    .unwrap_or(subject);
                Some(Stash {
                    id,
                    subject,
                    when,
                    time,
                })
            })
            .collect()
    }

    /// Put away by Discard, to undo.
    pub fn is_discard(&self) -> bool {
        self.subject.starts_with(DISCARD_PREFIX)
    }
}

/// What Discard names its stashes.
pub const DISCARD_PREFIX: &str = "Discarded: ";
/// Discarded changes are kept this long.
pub const DISCARD_KEEP_SECONDS: i64 = 7 * 24 * 3600;

/// A branch as `for-each-ref` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    /// "main", or "origin/main" for a remote one.
    pub name: String,
    pub current: bool,
    pub remote: bool,
    pub upstream: String,
    /// "[ahead 1, behind 2]" as git prints it, "" when nothing.
    pub track: String,
    pub when: String,
    pub subject: String,
}

impl Branch {
    /// Reads `for-each-ref` with tab-separated fields: short name, HEAD mark, upstream,
    /// its track, the committer date, the subject (which may hold tabs itself) and the
    /// full ref name last.
    pub fn parse(output: &str) -> Vec<Branch> {
        output
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() < 7 {
                    return None;
                }
                let name = parts[0].trim().to_owned();
                // The remote's own HEAD pointer is no branch.
                if name.ends_with("/HEAD") {
                    return None;
                }
                let refname = parts[parts.len() - 1];
                Some(Branch {
                    name,
                    current: parts[1].trim() == "*",
                    remote: refname.starts_with("refs/remotes/"),
                    upstream: parts[2].trim().to_owned(),
                    track: parts[3].trim().to_owned(),
                    when: parts[4].trim().to_owned(),
                    subject: parts[5..parts.len() - 1].join("\t").trim().to_owned(),
                })
            })
            .collect()
    }
}

/// What the header shows.
pub fn context(status: &Status, in_progress: Option<InProgress>, stashes: &[Stash]) -> Context {
    let Some(branch) = &status.branch else {
        return Context::inactive();
    };
    let title = if branch.head == "(detached)" {
        "Detached HEAD".to_owned()
    } else {
        branch.head.clone()
    };
    let mut parts = Vec::new();
    if let Some(what) = in_progress {
        let conflicts = status
            .changes
            .iter()
            .filter(|c| c.kind == Kind::Unmerged)
            .count();
        parts.push(match conflicts {
            0 => format!("{}, nothing left to resolve", what.word()),
            1 => format!("{}, 1 conflict", what.word()),
            n => format!("{}, {n} conflicts", what.word()),
        });
    } else if branch.initial {
        parts.push("no commits yet".to_owned());
    } else {
        parts.push(match &branch.upstream {
            None => "no upstream".to_owned(),
            Some(upstream) => match (branch.ahead, branch.behind) {
                (0, 0) => format!("up to date with {upstream}"),
                (a, 0) => format!("{a} ahead of {upstream}"),
                (0, b) => format!("{b} behind {upstream}"),
                (a, b) => format!("{a} ahead, {b} behind {upstream}"),
            },
        });
    }
    let summary = status.summary();
    parts.push(if summary == "None" {
        "clean".to_owned()
    } else {
        summary
    });
    let (staged, changed) = counts(status);
    let mut context = Context::active(title).subtitle(parts.join(" \u{00B7} "));
    if let Some(what) = in_progress {
        context = context
            .action(
                Action::new("continue", "Continue")
                    .symbol("play.fill")
                    .prominent(),
            )
            .action(
                Action::new("abort", "Abort")
                    .symbol("xmark")
                    .destructive(format!(
                        "Abort the {}? The repository goes back to how it was before it started.",
                        what.subcommand()
                    )),
            );
    } else {
        context = context
            .action(Action::new("fetch", "Fetch").symbol("arrow.down.to.line"))
            .action(Action::new("pull", "Pull").symbol("arrow.down"))
            .action(Action::new("push", "Push").symbol("arrow.up"));
        let commit = Action::new("commit", "Commit\u{2026}").symbol("checkmark.circle");
        context = context.action(if staged > 0 {
            commit.prominent()
        } else {
            commit
        });
    }
    if changed > 0 {
        context = context.badge("changes", changed.to_string());
    }
    if stashes.iter().any(Stash::is_discard) {
        context = context
            .action(Action::new("undo-discard", "Undo Discard").symbol("arrow.uturn.backward"));
    }
    context
}

/// Staged entries, and entries of any kind but ignored.
fn counts(status: &Status) -> (usize, usize) {
    let staged = status
        .changes
        .iter()
        .filter(|c| c.staged_word().is_some())
        .count();
    let changed = status
        .changes
        .iter()
        .filter(|c| c.kind != Kind::Ignored)
        .count();
    (staged, changed)
}

/// The Changes screen. `root` is the repository's root, where the status paths start.
pub fn changes(
    status: &Status,
    root: &str,
    in_progress: Option<InProgress>,
    stashes: &[Stash],
) -> Screen {
    let root = root.trim_end_matches('/');
    let full = |path: &str| format!("{root}/{}", path.trim_end_matches('/'));
    let name = |path: &str| path.trim_end_matches('/').to_owned();
    let mut screen = Screen::new()
        .multiple()
        .inspector("diff")
        .empty("Nothing changed since the last commit.");

    let (staged, _) = counts(status);
    if let Some(what) = in_progress {
        screen = screen
            .toolbar(
                Action::new("continue", "Continue")
                    .symbol("play.fill")
                    .prominent(),
            )
            .toolbar(
                Action::new("abort", "Abort")
                    .symbol("xmark")
                    .destructive(format!(
                        "Abort the {}? The repository goes back to how it was before it started.",
                        what.subcommand()
                    )),
            );
    } else {
        let commit = Action::new("commit", "Commit\u{2026}").symbol("checkmark.circle");
        screen = screen
            .toolbar(if staged > 0 {
                commit.prominent()
            } else {
                commit
            })
            .toolbar(Action::new("amend", "Amend\u{2026}").symbol("pencil"))
            .toolbar(Action::new("stash-save", "Stash All\u{2026}").symbol("tray.and.arrow.down"));
    }
    if stashes.iter().any(Stash::is_discard) {
        screen = screen
            .toolbar(Action::new("undo-discard", "Undo Discard").symbol("arrow.uturn.backward"));
    }

    let mut conflicts = Section::new("conflicts", "Conflicts")
        .action(Action::new("resolve", "Mark All Resolved").symbol("checkmark"));
    let mut staged_section = Section::new("staged", "Staged")
        .action(Action::new("unstage", "Unstage All").symbol("minus.circle"));
    let mut unstaged = Section::new("unstaged", "Changed")
        .action(Action::new("stage", "Stage All").symbol("plus.circle"))
        .action(Action::new("discard", "Discard All").symbol("arrow.uturn.backward").destructive(
            "Discard every change not staged? A copy is kept for a week: Undo Discard brings it back.",
        ));
    let mut untracked = Section::new("untracked", "Untracked")
        .action(Action::new("stage", "Stage All").symbol("plus.circle"));
    let discard = |what: &str| {
        Action::new("discard", "Discard")
            .symbol("arrow.uturn.backward")
            .destructive(format!(
                "Discard the changes to {what}? A copy is kept for a week: Undo Discard brings it back."
            ))
    };
    for change in &status.changes {
        let title = name(&change.path);
        match &change.kind {
            Kind::Unmerged => {
                conflicts = conflicts.row(
                    ScreenRow::file(full(&change.path), title)
                        .subtitle("Conflict")
                        .symbol("exclamationmark.triangle.fill", "red")
                        .action(Action::new("resolve", "Mark Resolved").symbol("checkmark"))
                        .primary("resolve"),
                );
            }
            Kind::Untracked => {
                untracked = untracked.row(
                    ScreenRow::file(full(&change.path), title.clone())
                        .subtitle(if change.path.ends_with('/') {
                            "Untracked folder"
                        } else {
                            "Untracked"
                        })
                        .symbol("questionmark.circle", "gray")
                        .action(Action::new("stage", "Stage").symbol("plus.circle"))
                        .action(Action::new("ignore", "Ignore").symbol("eye.slash"))
                        .action(discard(&title))
                        .primary("stage"),
                );
            }
            Kind::Ignored => {}
            Kind::Tracked { renamed_from, .. } => {
                if let Some(word) = change.staged_word() {
                    let subtitle = match renamed_from {
                        Some(from) => format!("{word} from {from}"),
                        None => word.to_owned(),
                    };
                    staged_section = staged_section.row(
                        ScreenRow::file(full(&change.path), title.clone())
                            .subtitle(subtitle)
                            .symbol(symbol_for(word), color_for(word))
                            .action(Action::new("unstage", "Unstage").symbol("minus.circle"))
                            .primary("unstage"),
                    );
                }
                if let Some(word) = change.unstaged_word() {
                    unstaged = unstaged.row(
                        ScreenRow::file(full(&change.path), title.clone())
                            .subtitle(word)
                            .symbol(symbol_for(word), color_for(word))
                            .action(Action::new("stage", "Stage").symbol("plus.circle"))
                            .action(discard(&title))
                            .primary("stage"),
                    );
                }
            }
        }
    }
    for section in [conflicts, staged_section, unstaged, untracked] {
        if !section.rows.is_empty() {
            screen = screen.section(section);
        }
    }
    if !stashes.is_empty() {
        let mut section = Section::new("stashes", "Stashes");
        for stash in stashes {
            let row = if stash.is_discard() {
                ScreenRow::new(stash.id.clone(), stash.subject.clone())
                    .subtitle(format!("{} \u{00B7} kept a week", stash.when))
                    .symbol("arrow.uturn.backward.circle", "orange")
                    .action(
                        Action::new("stash-pop", "Restore")
                            .symbol("arrow.uturn.backward")
                            .prominent(),
                    )
                    .action(
                        Action::new("stash-drop", "Forget")
                            .symbol("trash")
                            .destructive("Forget these discarded changes for good?"),
                    )
                    .primary("stash-pop")
            } else {
                ScreenRow::new(stash.id.clone(), stash.subject.clone())
                    .subtitle(stash.when.clone())
                    .symbol("tray.full", "blue")
                    .action(
                        Action::new("stash-pop", "Pop")
                            .symbol("tray.and.arrow.up")
                            .prominent(),
                    )
                    .action(Action::new("stash-apply", "Apply").symbol("doc.on.doc"))
                    .action(
                        Action::new("stash-drop", "Drop")
                            .symbol("trash")
                            .destructive("Drop this stash for good?"),
                    )
                    .primary("stash-pop")
            };
            section = section.row(row);
        }
        screen = screen.section(section);
    }
    screen
}

fn symbol_for(word: &str) -> &'static str {
    match word {
        "Added" => "plus.circle.fill",
        "Deleted" => "minus.circle.fill",
        "Renamed" | "Copied" => "arrow.right.circle.fill",
        "Conflict" => "exclamationmark.triangle.fill",
        _ => "pencil.circle.fill",
    }
}

fn color_for(word: &str) -> &'static str {
    match word {
        "Added" => "green",
        "Deleted" => "red",
        "Renamed" | "Copied" => "blue",
        "Conflict" => "red",
        _ => "orange",
    }
}

/// The Branches screen.
pub fn branches(list: &[Branch]) -> Screen {
    let mut screen = Screen::new()
        .inspector("branch-log")
        .empty("No branches yet: the first commit makes one.")
        .toolbar(
            Action::new("new-branch", "New Branch\u{2026}")
                .symbol("plus")
                .prominent(),
        )
        .toolbar(Action::new("fetch", "Fetch").symbol("arrow.down.to.line"));
    let mut local = Section::new("local", "Branches");
    let mut remote = Section::new("remote", "Remote branches");
    for branch in list {
        let mut parts = Vec::new();
        if !branch.upstream.is_empty() {
            parts.push(if branch.track.is_empty() {
                format!("up to date with {}", branch.upstream)
            } else {
                format!("{} {}", branch.upstream, branch.track)
            });
        }
        if !branch.when.is_empty() {
            parts.push(format!("{} \u{00B7} {}", branch.when, branch.subject));
        }
        let subtitle = parts.join(" \u{00B7} ");
        if branch.remote {
            remote = remote.row(
                ScreenRow::new(format!("remote:{}", branch.name), branch.name.clone())
                    .subtitle(subtitle)
                    .symbol("cloud", "gray")
                    .action(Action::new("checkout", "Check Out").symbol("arrow.right.circle"))
                    .primary("checkout"),
            );
        } else if branch.current {
            local = local.row(
                ScreenRow::new(format!("local:{}", branch.name), branch.name.clone())
                    .subtitle(subtitle)
                    .symbol("checkmark.circle.fill", "green")
                    .badge("current"),
            );
        } else {
            local = local.row(
                ScreenRow::new(format!("local:{}", branch.name), branch.name.clone())
                    .subtitle(subtitle)
                    .symbol("arrow.triangle.branch", "gray")
                    .action(
                        Action::new("checkout", "Check Out")
                            .symbol("arrow.right.circle")
                            .prominent(),
                    )
                    .action(
                        Action::new("merge", "Merge into Current").symbol("arrow.triangle.merge"),
                    )
                    .action(
                        Action::new("delete-branch", "Delete")
                            .symbol("trash")
                            .destructive(format!(
                                "Delete the branch {}? Commits only it has are lost.",
                                branch.name
                            )),
                    )
                    .primary("checkout"),
            );
        }
    }
    for section in [local, remote] {
        if !section.rows.is_empty() {
            screen = screen.section(section);
        }
    }
    screen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::Status;

    #[test]
    fn stashes_and_branches_are_read_from_gits_lines() {
        let stashes = Stash::parse(
            "stash@{0}\u{1f}On main: Discarded: a.txt\u{1f}2 hours ago\u{1f}1700000000\nstash@{1}\u{1f}WIP on main: 1a2b3c work\u{1f}3 days ago\u{1f}1699000000\n",
        );
        assert_eq!(stashes.len(), 2);
        assert!(stashes[0].is_discard() && stashes[0].subject == "Discarded: a.txt");
        assert_eq!(
            (stashes[1].subject.as_str(), stashes[1].time),
            ("1a2b3c work", 1699000000)
        );
        let list = Branch::parse(
            "main\t*\torigin/main\t[ahead 1]\t2 days ago\tFix it\trefs/heads/main\n\
             feature\t \t\t\t1 hour ago\tStart\tthe\tsubject\trefs/heads/feature\n\
             origin/HEAD\t \t\t\t\t\trefs/remotes/origin/HEAD\n\
             origin/main\t \t\t\t2 days ago\tFix it\trefs/remotes/origin/main\n",
        );
        assert_eq!(list.len(), 3);
        assert!(list[0].current && !list[0].remote && list[0].track == "[ahead 1]");
        assert!(list[2].remote && list[2].name == "origin/main");
        assert_eq!(list[1].subject, "Start\tthe\tsubject");
        let screen = branches(&list);
        assert_eq!(screen.sections[0].rows[0].badge, "current");
        assert_eq!(screen.sections[0].rows[1].primary, "checkout");
        assert_eq!(screen.sections[1].title, "Remote branches");
    }

    #[test]
    fn the_changes_screen_sorts_what_git_said_into_sections() {
        let status = Status::parse(
            b"# branch.oid abc\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +1 -0\0\
              1 M. N... 100644 100644 100644 h h staged.txt\0\
              1 .M N... 100644 100644 100644 h h changed.txt\0\
              1 MM N... 100644 100644 100644 h h both.txt\0\
              u UU N... 100644 100644 100644 100644 h h h conflict.txt\0\
              ? new.txt\0\
              ! build/\0",
        );
        let stashes = Stash::parse("stash@{0}\u{1f}On main: Discarded: x\u{1f}now\u{1f}1\n");
        let screen = changes(&status, "/repo", None, &stashes);
        let titles: Vec<&str> = screen.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(
            titles,
            vec!["Conflicts", "Staged", "Changed", "Untracked", "Stashes"]
        );
        assert_eq!(
            screen.sections[1]
                .rows
                .iter()
                .map(|r| r.title.as_str())
                .collect::<Vec<_>>(),
            vec!["staged.txt", "both.txt"]
        );
        assert_eq!(
            screen.sections[2]
                .rows
                .iter()
                .map(|r| r.title.as_str())
                .collect::<Vec<_>>(),
            vec!["changed.txt", "both.txt"]
        );
        assert_eq!(screen.sections[2].rows[0].path, "/repo/changed.txt");
        assert_eq!(
            screen.sections[3].rows[0]
                .actions
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            vec!["stage", "ignore", "discard"]
        );
        assert!(screen.toolbar.iter().any(|a| a.id == "undo-discard"));
        assert_eq!(screen.toolbar[0].style, "prominent");
        assert_eq!(screen.rows(), 7);

        let header = context(&status, None, &stashes);
        assert!(header.active && header.title == "main");
        assert_eq!(
            header.subtitle,
            "1 ahead of origin/main \u{00B7} 1 in conflict, 2 staged, 2 modified, 1 untracked"
        );
        assert_eq!(header.badges.get("changes").map(String::as_str), Some("5"));
        assert!(header.actions.iter().any(|a| a.id == "undo-discard"));

        let merging = context(&status, Some(InProgress::Merge), &[]);
        assert!(merging.subtitle.starts_with("merging, 1 conflict"));
        assert_eq!(
            merging
                .actions
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            vec!["continue", "abort"]
        );
        assert!(!context(&Status::default(), None, &[]).active);
    }
}
