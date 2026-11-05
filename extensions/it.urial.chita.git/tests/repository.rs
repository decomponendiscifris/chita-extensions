//! The extension against a real repository: the real git runs in a folder made for the
//! test, in the viewed folder as Chita runs it (Chita's own rules on arguments are tested
//! in Chita; the app's tests drive this extension through them too).

use chita_ext_sdk::{
    Answer, Ask, ColumnArgument, CommandArgument, ContextArgument, PanelArgument, ScreenArgument,
};
use std::cell::RefCell;
use std::path::PathBuf;
use std::process::Command;

struct Repo {
    root: PathBuf,
}

impl Repo {
    fn new() -> Repo {
        // Tests run together: each gets its own folder.
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "chita-git-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let repo = Repo { root };
        repo.setup(&["init", "-q", "-b", "main"]);
        repo.setup(&["config", "user.email", "test@example.com"]);
        repo.setup(&["config", "user.name", "Test"]);
        repo
    }

    /// Setup steps run git directly (what a developer would have done already). The
    /// user's own configuration stays out, so the tests see the repository's alone.
    fn setup(&self, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn path(&self, name: &str) -> String {
        self.root.join(name).to_string_lossy().into_owned()
    }

    fn folder(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }

    /// The extension's asks run git in the folder being viewed (`viewing`), as Chita
    /// does (a clean environment, no prompts); files are refused.
    fn answer_from_viewed_folder() {
        chita_ext_sdk::testing::answer_with(move |ask| match ask {
            Ask::RunTool { tool, args } if tool == "git" => {
                let folder = VIEWED.with(|v| v.borrow().clone());
                match Command::new("git")
                    .args(args)
                    .current_dir(&folder)
                    .env_clear()
                    .env("PATH", "/usr/bin:/bin:/opt/homebrew/bin:/usr/local/bin")
                    .env("HOME", std::env::var_os("HOME").unwrap_or_default())
                    .env("LANG", "C")
                    .env("LC_ALL", "C")
                    .env("GIT_EDITOR", "true")
                    .env("GIT_PAGER", "cat")
                    .env("GIT_TERMINAL_PROMPT", "0")
                    .env("GIT_OPTIONAL_LOCKS", "0")
                    .env("GIT_CONFIG_GLOBAL", "/dev/null")
                    .env("GIT_CONFIG_NOSYSTEM", "1")
                    .output()
                {
                    Ok(out) => Answer::Tool {
                        status: out.status.code().unwrap_or(-1),
                        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                    },
                    Err(e) => Answer::Refused {
                        reason: e.to_string(),
                    },
                }
            }
            Ask::RunTool { tool, .. } => Answer::Refused {
                reason: format!("{tool} isn't a tool Chita runs"),
            },
            // Files in the viewed folder, read and written, as the manifest's grant says.
            Ask::ReadFile { path } => {
                let folder = VIEWED.with(|v| v.borrow().clone());
                match std::fs::read(folder.join(path)) {
                    Ok(contents) => Answer::File { contents },
                    Err(e) => Answer::Refused {
                        reason: e.to_string(),
                    },
                }
            }
            Ask::WriteFile { path, contents } => {
                let folder = VIEWED.with(|v| v.borrow().clone());
                match std::fs::write(folder.join(path), contents) {
                    Ok(()) => Answer::Written {},
                    Err(e) => Answer::Refused {
                        reason: e.to_string(),
                    },
                }
            }
            Ask::ListFolder { path } => {
                let folder = VIEWED.with(|v| v.borrow().clone());
                let entries = std::fs::read_dir(folder.join(path))
                    .map(|r| {
                        r.flatten()
                            .map(|e| chita_ext_sdk::Entry {
                                name: e.file_name().to_string_lossy().into_owned(),
                                is_folder: e.path().is_dir(),
                                size: 0,
                                modified: 0,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Answer::Folder { entries }
            }
        });
    }
}

thread_local! {
    static VIEWED: RefCell<PathBuf> = const { RefCell::new(PathBuf::new()) };
}

/// The folder the user is looking at: tools run there.
fn viewing(folder: &str) {
    VIEWED.with(|v| *v.borrow_mut() = PathBuf::from(folder));
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn column(repo: &Repo, folder: &str, names: &[&str]) -> Vec<String> {
    viewing(folder);
    chita_git::column_texts(ColumnArgument {
        column: "state".into(),
        folder: folder.to_owned(),
        paths: names.iter().map(|n| repo.path(n)).collect(),
    })
}

fn run(
    repo: &Repo,
    command: &str,
    names: &[&str],
    input: Option<&str>,
) -> chita_ext_sdk::CommandResult {
    viewing(&repo.folder());
    chita_git::run_command(CommandArgument {
        command: command.into(),
        folder: Some(repo.folder()),
        selection: names.iter().map(|n| repo.path(n)).collect(),
        input: input.map(str::to_owned),
        ..Default::default()
    })
}

/// An action of a screen on `rows` (ids).
fn act_on(
    repo: &Repo,
    screen: &str,
    command: &str,
    rows: &[&str],
    input: Option<&str>,
) -> chita_ext_sdk::CommandResult {
    viewing(&repo.folder());
    chita_git::run_command(CommandArgument {
        command: command.into(),
        folder: Some(repo.folder()),
        screen: Some(screen.into()),
        rows: rows.iter().map(|r| (*r).to_owned()).collect(),
        input: input.map(str::to_owned),
        ..Default::default()
    })
}

fn screen(repo: &Repo, id: &str) -> chita_ext_sdk::Screen {
    viewing(&repo.folder());
    chita_git::show_screen(ScreenArgument {
        screen: id.into(),
        folder: Some(repo.folder()),
        ..Default::default()
    })
}

fn context(repo: &Repo) -> chita_ext_sdk::Context {
    viewing(&repo.folder());
    chita_git::describe_context(ContextArgument {
        context: "repository".into(),
        folder: Some(repo.folder()),
        ..Default::default()
    })
}

/// Row titles of a screen's section, by section id.
fn rows_of(screen: &chita_ext_sdk::Screen, section: &str) -> Vec<String> {
    screen
        .sections
        .iter()
        .find(|s| s.id == section)
        .map(|s| s.rows.iter().map(|r| r.title.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn the_column_the_panel_and_the_commands_follow_the_repository() {
    let repo = Repo::new();
    Repo::answer_from_viewed_folder();
    viewing(&repo.folder());
    repo.write("a.txt", "one\n");
    repo.write("dir/b.txt", "two\n");
    repo.write("dir/c.txt", "three\n");

    // Untracked files, and a folder of them.
    assert_eq!(
        column(&repo, &repo.folder(), &["a.txt", "dir", "none.txt"]),
        vec!["Untracked", "Untracked", ""]
    );
    // Before the first commit: the panel says so.
    viewing(&repo.folder());
    let panel = chita_git::show_panel(PanelArgument::default());
    let rows: Vec<(&str, &str)> = panel
        .rows
        .iter()
        .map(|r| (r.label.as_str(), r.value.as_str()))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("Branch", "main"),
            ("Upstream", "None"),
            // The folder counts once, as git lists it.
            ("Changes", "2 untracked")
        ]
    );

    // Stage one, commit with the prompt's text.
    assert_eq!(
        run(&repo, "stage", &["a.txt"], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert_eq!(
        column(&repo, &repo.folder(), &["a.txt"]),
        vec!["Added (staged)"]
    );
    let asked = run(&repo, "commit", &[], None);
    assert_eq!(
        asked.prompt.as_ref().map(|p| p.title.as_str()),
        Some("Commit")
    );
    let committed = run(&repo, "commit", &[], Some("First"));
    assert!(
        committed.message.as_deref().unwrap_or("").contains("First"),
        "{committed:?}"
    );
    assert_eq!(
        column(&repo, &repo.folder(), &["a.txt", "dir"]),
        vec!["", "Untracked"]
    );

    // A change, seen in the column, the panel, the changes and the history.
    repo.write("a.txt", "one\nmore\n");
    assert_eq!(column(&repo, &repo.folder(), &["a.txt"]), vec!["Modified"]);
    let panel = chita_git::show_panel(PanelArgument::default());
    assert_eq!(panel.rows[2].value, "1 modified, 1 untracked");
    assert!(panel.rows[3].value.contains("First"), "{:?}", panel.rows[3]);
    let changes = run(&repo, "changes", &["a.txt"], None);
    assert!(
        changes.output.as_deref().unwrap_or("").contains("+more"),
        "{changes:?}"
    );
    let history = run(&repo, "history", &[], None);
    assert!(
        history.output.as_deref().unwrap_or("").contains("First"),
        "{history:?}"
    );

    // Staged, then unstaged again; inside the subfolder the prefix is applied.
    run(&repo, "stage", &["a.txt", "dir"], None);
    assert_eq!(
        column(&repo, &repo.folder(), &["a.txt", "dir"]),
        vec!["Modified (staged)", "2 changes"]
    );
    assert_eq!(
        column(&repo, &repo.path("dir"), &["dir/b.txt", "dir/c.txt"]),
        vec!["Added (staged)", "Added (staged)"]
    );
    assert_eq!(
        run(&repo, "unstage", &["dir/b.txt"], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert_eq!(
        column(&repo, &repo.path("dir"), &["dir/b.txt"]),
        vec!["Untracked"]
    );

    // Nothing to show says so (an untracked file has no changes since the last commit);
    // an unknown command too.
    assert_eq!(
        run(&repo, "changes", &["dir/b.txt"], None)
            .message
            .as_deref(),
        Some("No changes.")
    );
    assert!(
        run(&repo, "nope", &[], None)
            .message
            .unwrap()
            .contains("no command nope")
    );
}

#[test]
fn outside_a_repository_there_is_nothing_to_show() {
    let folder = std::env::temp_dir().join(format!("chita-git-plain-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    Repo::answer_from_viewed_folder();
    viewing(&folder.to_string_lossy());
    let paths = vec![folder.join("x").to_string_lossy().into_owned()];
    assert_eq!(
        chita_git::column_texts(ColumnArgument {
            column: "state".into(),
            folder: folder.to_string_lossy().into_owned(),
            paths,
        }),
        vec![""]
    );
    assert_eq!(
        chita_git::show_panel(PanelArgument::default()),
        chita_ext_sdk::Panel::default()
    );
    let said = chita_git::run_command(CommandArgument {
        command: "history".into(),
        folder: Some(folder.to_string_lossy().into_owned()),
        ..Default::default()
    });
    assert!(said.message.unwrap().contains("not a git repository"));
    let _ = std::fs::remove_dir_all(&folder);
}

/// Fetch, pull and push with a remote the repository names (a bare repository here), and
/// never with a URL.
#[test]
fn the_network_commands_reach_only_the_repository_s_remotes() {
    let repo = Repo::new();
    Repo::answer_from_viewed_folder();
    viewing(&repo.folder());
    repo.write("a.txt", "one\n");
    repo.setup(&["add", "a.txt"]);
    repo.setup(&["commit", "-q", "-m", "First"]);
    let bare = repo.root.join("remote.git");
    repo.setup(&["init", "-q", "--bare", &bare.to_string_lossy()]);
    repo.setup(&["remote", "add", "origin", &bare.to_string_lossy()]);
    repo.setup(&["push", "-q", "-u", "origin", "main"]);

    let pushed = run(&repo, "push", &[], None);
    assert!(
        pushed
            .message
            .as_deref()
            .unwrap_or("")
            .contains("up-to-date"),
        "{pushed:?}"
    );
    let fetched = run(&repo, "fetch", &[], None);
    assert_eq!(fetched.message.as_deref(), Some("Done."));
    let pulled = run(&repo, "pull", &[], None);
    assert!(
        pulled
            .message
            .as_deref()
            .unwrap_or("")
            .contains("up to date"),
        "{pulled:?}"
    );
    // The panel knows the upstream now. (That a URL or `remote add` is refused before git
    // runs is Chita's rule, tested in Chita: `chita_ext::tools`.)
    let panel = chita_git::show_panel(PanelArgument::default());
    assert_eq!(panel.rows[1].value, "origin/main, up to date");
}

#[test]
fn the_header_the_screens_discard_and_ignore_follow_the_repository() {
    let repo = Repo::new();
    Repo::answer_from_viewed_folder();
    repo.write("a.txt", "one\n");
    repo.write("b.txt", "two\n");
    repo.setup(&["add", "."]);
    repo.setup(&["commit", "-q", "-m", "First"]);

    // The header: branch, state, actions; nothing in progress.
    let header = context(&repo);
    assert!(header.active && header.title == "main", "{header:?}");
    assert_eq!(header.subtitle, "no upstream \u{00B7} clean");
    assert_eq!(
        header
            .actions
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        vec!["fetch", "pull", "push", "commit"]
    );
    assert!(header.badges.is_empty());

    // Changes: a modified file, a new one; the badge counts them.
    repo.write("a.txt", "one\nmore\n");
    repo.write("c.txt", "three\n");
    let changes = screen(&repo, "changes");
    assert_eq!(rows_of(&changes, "unstaged"), vec!["a.txt"]);
    assert_eq!(rows_of(&changes, "untracked"), vec!["c.txt"]);
    assert_eq!(
        context(&repo).badges.get("changes").map(String::as_str),
        Some("2")
    );
    // The diff for the Inspector: the tracked file's change, the new file whole.
    let diff = act_on(
        &repo,
        "changes",
        "diff",
        &[&repo.path("a.txt"), &repo.path("c.txt")],
        None,
    );
    let text = diff.diff.clone().unwrap_or_default();
    assert!(
        text.contains("+more") && text.contains("+three"),
        "{diff:?}"
    );
    assert_eq!(diff.title.as_deref(), Some("2 files"));

    // Stage from the screen, then the Staged section has it and Commit is prominent.
    act_on(&repo, "changes", "stage", &[&repo.path("c.txt")], None);
    let changes = screen(&repo, "changes");
    assert_eq!(rows_of(&changes, "staged"), vec!["c.txt"]);
    assert_eq!(changes.toolbar[0].id, "commit");
    assert_eq!(changes.toolbar[0].style, "prominent");

    // Discard puts the change away: the file is as committed, a stash keeps it, Undo brings
    // it back.
    assert_eq!(
        act_on(&repo, "changes", "discard", &[&repo.path("a.txt")], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert_eq!(
        std::fs::read_to_string(repo.root.join("a.txt")).unwrap(),
        "one\n"
    );
    let changes = screen(&repo, "changes");
    assert!(rows_of(&changes, "unstaged").is_empty());
    let stashes = rows_of(&changes, "stashes");
    assert_eq!(stashes, vec!["Discarded: a.txt"]);
    assert!(changes.toolbar.iter().any(|a| a.id == "undo-discard"));
    assert!(
        context(&repo)
            .actions
            .iter()
            .any(|a| a.id == "undo-discard")
    );
    assert_eq!(
        run(&repo, "undo-discard", &[], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert_eq!(
        std::fs::read_to_string(repo.root.join("a.txt")).unwrap(),
        "one\nmore\n"
    );
    assert!(rows_of(&screen(&repo, "changes"), "stashes").is_empty());
    assert_eq!(
        run(&repo, "undo-discard", &[], None).message.as_deref(),
        Some("Nothing was discarded lately.")
    );

    // Stash all with a message, pop it back.
    let asked = run(&repo, "stash-save", &[], None);
    assert_eq!(
        asked.prompt.as_ref().map(|p| p.action.as_str()),
        Some("Stash")
    );
    run(&repo, "stash-save", &[], Some("Half done"));
    let changes = screen(&repo, "changes");
    assert_eq!(rows_of(&changes, "stashes"), vec!["Half done"]);
    assert!(rows_of(&changes, "unstaged").is_empty() && rows_of(&changes, "staged").is_empty());
    let id = changes
        .sections
        .iter()
        .find(|s| s.id == "stashes")
        .unwrap()
        .rows[0]
        .id
        .clone();
    assert_eq!(
        act_on(&repo, "changes", "stash-pop", &[&id], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert_eq!(rows_of(&screen(&repo, "changes"), "staged"), vec!["c.txt"]);

    // Ignore: the pattern lands in .gitignore and the item leaves Untracked.
    repo.write("build/out.o", "x");
    repo.write("notes.log", "x");
    assert_eq!(
        run(&repo, "ignore", &["build"], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert_eq!(
        run(&repo, "ignore-ext", &["notes.log"], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    let ignored = std::fs::read_to_string(repo.root.join(".gitignore")).unwrap();
    assert!(
        ignored.contains("/build/\n") && ignored.contains("*.log\n"),
        "{ignored}"
    );
    let untracked = rows_of(&screen(&repo, "changes"), "untracked");
    assert!(
        untracked.contains(&".gitignore".to_owned())
            && !untracked
                .iter()
                .any(|n| n.contains("build") || n.contains("notes")),
        "{untracked:?}"
    );
    assert_eq!(
        column(&repo, &repo.folder(), &["build", "notes.log"]),
        vec!["Ignored", "Ignored"]
    );
    // Stop tracking a committed file: out of git, kept on disk, ignored.
    assert_eq!(
        run(&repo, "untrack", &["b.txt"], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert!(repo.root.join("b.txt").is_file());
    assert!(
        std::fs::read_to_string(repo.root.join(".gitignore"))
            .unwrap()
            .contains("/b.txt\n")
    );
    assert_eq!(
        rows_of(&screen(&repo, "changes"), "staged"),
        vec!["b.txt", "c.txt"]
    );
    // The editor: the file as it is, then saved as typed.
    let edit = run(&repo, "edit-ignore", &[], None);
    let prompt = edit.prompt.unwrap();
    assert!(prompt.multiline && prompt.initial.contains("*.log"));
    run(&repo, "edit-ignore", &[], Some("# mine\n*.tmp"));
    assert_eq!(
        std::fs::read_to_string(repo.root.join(".gitignore")).unwrap(),
        "# mine\n*.tmp\n"
    );

    // Branches: a new one is current; checkout, merge and delete from the screen.
    run(&repo, "commit", &[], Some("Second"));
    assert_eq!(
        run(&repo, "new-branch", &[], Some("feature/x")),
        chita_ext_sdk::CommandResult::nothing()
    );
    let branches = screen(&repo, "branches");
    assert_eq!(rows_of(&branches, "local"), vec!["feature/x", "main"]);
    assert_eq!(branches.sections[0].rows[0].badge, "current");
    assert_eq!(context(&repo).title, "feature/x");
    repo.write("d.txt", "four\n");
    repo.setup(&["add", "d.txt"]);
    repo.setup(&["commit", "-q", "-m", "On the feature"]);
    assert_eq!(
        act_on(&repo, "branches", "checkout", &["local:main"], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert_eq!(context(&repo).title, "main");
    let not_merged = act_on(
        &repo,
        "branches",
        "delete-branch",
        &["local:feature/x"],
        None,
    );
    assert!(not_merged.message.unwrap().contains("aren\u{2019}t merged"));
    assert_eq!(
        act_on(&repo, "branches", "merge", &["local:feature/x"], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert!(repo.root.join("d.txt").is_file());
    assert_eq!(
        act_on(
            &repo,
            "branches",
            "delete-branch",
            &["local:feature/x"],
            None
        ),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert_eq!(rows_of(&screen(&repo, "branches"), "local"), vec!["main"]);
    let log = act_on(&repo, "branches", "branch-log", &["local:main"], None);
    assert!(log.output.unwrap().contains("On the feature"));
}

#[test]
fn a_conflict_is_shown_and_resolved_and_the_identity_is_asked_once() {
    let repo = Repo::new();
    Repo::answer_from_viewed_folder();
    repo.write("a.txt", "one\n");
    repo.setup(&["add", "."]);
    repo.setup(&["commit", "-q", "-m", "First"]);
    repo.setup(&["switch", "-q", "-c", "other"]);
    repo.write("a.txt", "theirs\n");
    repo.setup(&["commit", "-q", "-am", "Theirs"]);
    repo.setup(&["switch", "-q", "main"]);
    repo.write("a.txt", "ours\n");
    repo.setup(&["commit", "-q", "-am", "Ours"]);
    // The merge stops on the conflict: the header says so, Continue and Abort appear.
    let merged = act_on(&repo, "branches", "merge", &["local:other"], None);
    assert!(merged.message.unwrap().to_lowercase().contains("conflict"));
    let header = context(&repo);
    assert!(
        header.subtitle.starts_with("merging, 1 conflict"),
        "{header:?}"
    );
    assert_eq!(
        header
            .actions
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        vec!["continue", "abort"]
    );
    let changes = screen(&repo, "changes");
    assert_eq!(rows_of(&changes, "conflicts"), vec!["a.txt"]);
    // Resolved by hand, marked, continued: the merge commit is made.
    repo.write("a.txt", "both\n");
    assert_eq!(
        act_on(&repo, "changes", "resolve", &[&repo.path("a.txt")], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert_eq!(
        run(&repo, "continue", &[], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert!(context(&repo).subtitle.ends_with("clean"));
    assert_eq!(
        run(&repo, "continue", &[], None).message.as_deref(),
        Some("Nothing is in progress.")
    );

    // No name or address: Commit… asks for them first, once, then for the message.
    repo.setup(&["config", "--unset", "user.name"]);
    repo.setup(&["config", "--unset", "user.email"]);
    repo.write("a.txt", "again\n");
    repo.setup(&["add", "a.txt"]);
    let asked = run(&repo, "commit", &[], None).prompt.unwrap();
    assert_eq!(
        (asked.title.as_str(), asked.next.as_str()),
        ("Your name for commits", "identity-name")
    );
    let asked = run(&repo, "identity-name", &[], Some("Someone"))
        .prompt
        .unwrap();
    assert_eq!(asked.next, "identity-email");
    let asked = run(&repo, "identity-email", &[], Some("someone@example.com"))
        .prompt
        .unwrap();
    assert_eq!((asked.title.as_str(), asked.next.as_str()), ("Commit", ""));
    assert!(
        run(&repo, "commit", &[], Some("Again"))
            .message
            .unwrap()
            .contains("Again")
    );
    assert!(run(&repo, "commit", &[], None).prompt.unwrap().title == "Commit");
}

#[test]
fn history_tags_and_worktrees_follow_the_repository() {
    let repo = Repo::new();
    Repo::answer_from_viewed_folder();
    repo.write("a.txt", "one\n");
    repo.setup(&["add", "."]);
    repo.setup(&["commit", "-q", "-m", "First"]);
    repo.setup(&["switch", "-q", "-c", "side"]);
    repo.write("b.txt", "two\n");
    repo.setup(&["add", "b.txt"]);
    repo.setup(&["commit", "-q", "-m", "On the side"]);
    repo.setup(&["switch", "-q", "main"]);
    repo.write("a.txt", "one\nmore\n");
    repo.setup(&["commit", "-q", "-am", "Second"]);
    repo.setup(&["merge", "-q", "--no-ff", "--no-edit", "side"]);

    // History: the merge first, in lane 0, with two lines leaving it; refs as badges.
    let history = screen(&repo, "history");
    let rows = &history.sections[0].rows;
    assert_eq!(rows.len(), 4);
    assert!(rows[0].title.starts_with("Merge"), "{:?}", rows[0].title);
    assert!(rows[0].badge.contains("main"), "{:?}", rows[0].badge);
    let cell = rows[0].graph.as_ref().unwrap();
    assert_eq!((cell.lane, cell.below.len()), (0, 2));
    assert!(rows.iter().any(|r| r.graph.as_ref().unwrap().lane == 1));
    assert!(history.toolbar.iter().all(|a| a.id != "history-more"));
    // The commit's page for the Inspector.
    let shown = act_on(&repo, "history", "commit-show", &[&rows[1].id], None);
    assert!(
        shown.diff.as_deref().unwrap_or("").contains("+more"),
        "{shown:?}"
    );
    // A tag from the toolbar, listed with the branches, checked out detached, deleted.
    assert_eq!(
        act_on(&repo, "history", "tag-here", &[&rows[1].id], Some("v1")),
        chita_ext_sdk::CommandResult::nothing()
    );
    let branches = screen(&repo, "branches");
    assert_eq!(rows_of(&branches, "tags"), vec!["v1"]);
    assert_eq!(
        act_on(&repo, "branches", "checkout", &["tag:v1"], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert_eq!(context(&repo).title, "Detached HEAD");
    assert_eq!(
        act_on(&repo, "branches", "checkout", &["local:main"], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert_eq!(
        act_on(&repo, "branches", "tag-delete", &["tag:v1"], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert!(rows_of(&screen(&repo, "branches"), "tags").is_empty());
    // A branch from a commit, and a cherry-pick of the side commit onto it.
    assert_eq!(
        act_on(
            &repo,
            "history",
            "branch-here",
            &[&rows[3].id],
            Some("from-first")
        ),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert_eq!(context(&repo).title, "from-first");
    let side = rows.iter().find(|r| r.title == "On the side").unwrap();
    assert_eq!(
        act_on(&repo, "history", "cherry-pick", &[&side.id], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert!(repo.root.join("b.txt").is_file());
    assert_eq!(
        act_on(&repo, "history", "revert", &[&side.id], None),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert!(!repo.root.join("b.txt").is_file());

    // Worktrees: this one is the main; one added next to the repository, opened by its
    // folder, removed again.
    let trees = screen(&repo, "worktrees");
    assert_eq!(trees.sections[0].rows.len(), 1);
    assert_eq!(trees.sections[0].rows[0].badge, "main worktree");
    assert_eq!(
        run(&repo, "worktree-add", &[], Some("feature/tree")),
        chita_ext_sdk::CommandResult::nothing()
    );
    let trees = screen(&repo, "worktrees");
    assert_eq!(trees.sections[0].rows.len(), 2);
    let added = &trees.sections[0].rows[1];
    assert!(
        added.folder.ends_with("-feature-tree") && std::path::Path::new(&added.folder).is_dir(),
        "{added:?}"
    );
    assert!(added.subtitle.starts_with("feature/tree"));
    assert_eq!(
        act_on(
            &repo,
            "worktrees",
            "worktree-remove",
            &[&added.folder],
            None
        ),
        chita_ext_sdk::CommandResult::nothing()
    );
    assert!(!std::path::Path::new(&added.folder).is_dir());
    assert_eq!(screen(&repo, "worktrees").sections[0].rows.len(), 1);
}
