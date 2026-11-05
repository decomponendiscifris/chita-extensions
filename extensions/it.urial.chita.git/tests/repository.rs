//! The extension against a real repository: the real git runs in a folder made for the
//! test, in the viewed folder as Chita runs it (Chita's own rules on arguments are tested
//! in Chita; the app's tests drive this extension through them too).

use chita_ext_sdk::{Answer, Ask, ColumnArgument, CommandArgument, PanelArgument};
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

    /// Setup steps run git directly (what a developer would have done already).
    fn setup(&self, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.root)
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
            _ => Answer::Refused {
                reason: "files aren't granted".into(),
            },
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
    })
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
