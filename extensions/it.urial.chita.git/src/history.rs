//! The History screen: commits as `git log` lists them, laid out in lanes for the graph
//! Chita draws, with their refs as badges. Tags and worktrees as git lists them. Pure.

use chita_ext_sdk::{Action, GraphCell, Screen, ScreenRow, Section};

/// A commit as `git log --format=%H%x1f%h%x1f%P%x1f%an%x1f%cr%x1f%D%x1f%s` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub hash: String,
    pub short: String,
    pub parents: Vec<String>,
    pub author: String,
    pub when: String,
    /// "HEAD -> main, origin/main, tag: v1" as git decorates it.
    pub refs: String,
    pub subject: String,
}

impl Commit {
    pub fn parse(output: &str) -> Vec<Commit> {
        output
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\u{1f}').collect();
                if parts.len() < 7 {
                    return None;
                }
                Some(Commit {
                    hash: parts[0].trim().to_owned(),
                    short: parts[1].trim().to_owned(),
                    parents: parts[2].split_whitespace().map(str::to_owned).collect(),
                    author: parts[3].trim().to_owned(),
                    when: parts[4].trim().to_owned(),
                    refs: parts[5].trim().to_owned(),
                    subject: parts[6..].join("\u{1f}").trim().to_owned(),
                })
            })
            .collect()
    }

    /// The refs without "HEAD -> " and "tag: ", for a badge.
    pub fn badge(&self) -> String {
        self.refs
            .split(", ")
            .filter(|r| !r.is_empty())
            .map(|r| r.trim_start_matches("HEAD -> ").trim_start_matches("tag: "))
            .collect::<Vec<_>>()
            .join(" \u{00B7} ")
    }
}

/// Lays commits out in lanes, newest first, as `git log --graph` does: a commit sits in
/// the lane that waits for it (or a free one); each parent's line goes to the lane
/// already waiting for that parent, else the first parent keeps a lane and the others
/// take free ones; other lanes pass straight through. Each cell says where the lines
/// come from above and go below.
pub fn layout(commits: &[Commit]) -> Vec<GraphCell> {
    // Each open lane holds the hash it waits for.
    let mut lanes: Vec<Option<String>> = Vec::new();
    let mut cells = Vec::with_capacity(commits.len());
    let mut above: Vec<(u32, u32)> = Vec::new();
    let waiting = |lanes: &[Option<String>], hash: &str| {
        lanes.iter().position(|l| l.as_deref() == Some(hash))
    };
    let free = |lanes: &mut Vec<Option<String>>| {
        lanes.iter().position(Option::is_none).unwrap_or_else(|| {
            lanes.push(None);
            lanes.len() - 1
        })
    };
    for commit in commits {
        let lane = match waiting(&lanes, &commit.hash) {
            Some(i) => i,
            None => free(&mut lanes),
        };
        // Lines arriving from the row above; lanes waiting for this commit, other than
        // its own, bend into it and end.
        let mut arriving = above.clone();
        for (i, l) in lanes.iter_mut().enumerate() {
            if l.as_deref() == Some(&commit.hash) {
                *l = None;
                if i != lane {
                    arriving.push((i as u32, lane as u32));
                }
            }
        }
        // Where the lines go next. Lanes open before this commit's parents pass straight
        // through; the ones its parents open start here.
        let existing: Vec<bool> = lanes.iter().map(Option::is_some).collect();
        let mut below: Vec<(u32, u32)> = Vec::new();
        for (k, parent) in commit.parents.iter().enumerate() {
            let target = match waiting(&lanes, parent) {
                Some(i) => i,
                None if k == 0 => {
                    lanes[lane] = Some(parent.clone());
                    lane
                }
                None => {
                    let j = free(&mut lanes);
                    lanes[j] = Some(parent.clone());
                    j
                }
            };
            if !below.contains(&(lane as u32, target as u32)) {
                below.push((lane as u32, target as u32));
            }
        }
        // Every other open lane passes straight through.
        for (i, l) in lanes.iter().enumerate() {
            if l.is_some()
                && existing.get(i).copied().unwrap_or(false)
                && !below.iter().any(|(from, _)| *from as usize == i)
            {
                below.push((i as u32, i as u32));
            }
        }
        while lanes.last().is_some_and(Option::is_none) {
            lanes.pop();
        }
        // The lines arriving at the next row start where they ended here.
        above.clear();
        for (_, to) in &below {
            if !above.contains(&(*to, *to)) {
                above.push((*to, *to));
            }
        }
        cells.push(GraphCell {
            lane: lane as u32,
            above: arriving,
            below,
        });
    }
    cells
}

/// The History screen. `more` says whether there are older commits to load.
pub fn history(commits: &[Commit], more: bool) -> Screen {
    let cells = layout(commits);
    let mut screen = Screen::new()
        .inspector("commit-show")
        .empty("No commits yet.");
    if more {
        screen = screen.toolbar(
            Action::new("history-more", "Show Older Commits").symbol("arrow.down.to.line"),
        );
    }
    screen = screen.toolbar(Action::new("tag-new", "New Tag\u{2026}").symbol("tag"));
    let mut section = Section::new("commits", "Commits");
    for (commit, cell) in commits.iter().zip(cells) {
        let mut row = ScreenRow::new(commit.hash.clone(), commit.subject.clone())
            .subtitle(format!(
                "{} \u{00B7} {} \u{00B7} {}",
                commit.short, commit.author, commit.when
            ))
            .graph(cell)
            .action(
                Action::new("branch-here", "New Branch Here\u{2026}")
                    .symbol("arrow.triangle.branch"),
            )
            .action(
                Action::new("checkout-commit", "Check Out This Commit")
                    .symbol("arrow.right.circle"),
            )
            .action(Action::new("cherry-pick", "Cherry-Pick onto Current").symbol("arrow.down.doc"))
            .action(
                Action::new("revert", "Revert")
                    .symbol("arrow.uturn.backward")
                    .destructive("Make a new commit that undoes this one?"),
            )
            .action(Action::new("tag-here", "Tag\u{2026}").symbol("tag"));
        let badge = commit.badge();
        if !badge.is_empty() {
            row = row.badge(badge);
        }
        section = section.row(row);
    }
    screen.section(section)
}

/// A worktree as `git worktree list --porcelain` reports it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Worktree {
    pub path: String,
    pub head: String,
    /// "main" for refs/heads/main; "" when detached.
    pub branch: String,
    pub main: bool,
}

impl Worktree {
    pub fn parse(output: &str) -> Vec<Worktree> {
        let mut list = Vec::new();
        let mut current: Option<Worktree> = None;
        let mut first = true;
        for line in output.lines() {
            if let Some(path) = line.strip_prefix("worktree ") {
                if let Some(w) = current.take() {
                    list.push(w);
                }
                current = Some(Worktree {
                    path: path.to_owned(),
                    main: first,
                    ..Default::default()
                });
                first = false;
            } else if let Some(w) = current.as_mut() {
                if let Some(head) = line.strip_prefix("HEAD ") {
                    w.head = head.chars().take(7).collect();
                } else if let Some(branch) = line.strip_prefix("branch ") {
                    w.branch = branch.trim_start_matches("refs/heads/").to_owned();
                }
            }
        }
        if let Some(w) = current {
            list.push(w);
        }
        list
    }
}

/// The Worktrees screen.
pub fn worktrees(list: &[Worktree]) -> Screen {
    let screen = Screen::new()
        .empty("No worktrees besides this one.")
        .toolbar(
            Action::new("worktree-add", "Add Worktree\u{2026}")
                .symbol("plus")
                .prominent(),
        );
    let mut section = Section::new("worktrees", "Worktrees");
    for w in list {
        let name = w.path.rsplit('/').next().unwrap_or(&w.path).to_owned();
        let mut row = ScreenRow::new(w.path.clone(), name)
            .subtitle(format!(
                "{} \u{00B7} {}",
                if w.branch.is_empty() {
                    format!("detached at {}", w.head)
                } else {
                    w.branch.clone()
                },
                w.path
            ))
            .symbol(
                if w.main { "house" } else { "square.on.square" },
                if w.main { "accent" } else { "gray" },
            )
            .folder(w.path.clone());
        if w.main {
            row = row.badge("main worktree");
        } else {
            row = row.action(
                Action::new("worktree-remove", "Remove")
                    .symbol("trash")
                    .destructive("Remove this worktree? Its folder is deleted; the branch stays."),
            );
        }
        section = section.row(row);
    }
    screen.section(section)
}

/// A submodule as `git submodule status` reports it: a mark (" " in place, "-" not
/// initialised, "+" at another commit, "U" with conflicts), the commit, the path, and
/// what `describe` says in parentheses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Submodule {
    pub path: String,
    pub mark: char,
    pub commit: String,
    pub describe: String,
}

impl Submodule {
    pub fn parse(output: &str) -> Vec<Submodule> {
        output
            .lines()
            .filter_map(|line| {
                let mut chars = line.chars();
                let mark = chars.next()?;
                let rest = chars.as_str();
                let mut parts = rest.splitn(2, ' ');
                let commit = parts.next()?.chars().take(7).collect();
                let tail = parts.next()?.trim();
                let (path, describe) = match tail.split_once(" (") {
                    Some((p, d)) => (p.trim().to_owned(), d.trim_end_matches(')').to_owned()),
                    None => (tail.to_owned(), String::new()),
                };
                Some(Submodule {
                    path,
                    mark,
                    commit,
                    describe,
                })
            })
            .collect()
    }

    pub fn state(&self) -> &'static str {
        match self.mark {
            '-' => "not set up",
            '+' => "at another commit than recorded",
            'U' => "in conflict",
            _ => "as recorded",
        }
    }
}

/// The Submodules section of the Worktrees screen; `root` is the repository's root.
pub fn submodules(list: &[Submodule], root: &str) -> Section {
    let root = root.trim_end_matches('/');
    let mut section = Section::new("submodules", "Submodules");
    for m in list {
        let mut subtitle = m.state().to_owned();
        if !m.describe.is_empty() {
            subtitle.push_str(&format!(" \u{00B7} {}", m.describe));
        }
        if !m.commit.is_empty() {
            subtitle.push_str(&format!(" \u{00B7} {}", m.commit));
        }
        let (color, symbol) = match m.mark {
            '-' => ("gray", "square.dashed"),
            '+' => ("orange", "square.stack.3d.up"),
            'U' => ("red", "exclamationmark.triangle.fill"),
            _ => ("blue", "square.stack.3d.up"),
        };
        section = section.row(
            ScreenRow::new(format!("{root}/{}", m.path), m.path.clone())
                .subtitle(subtitle)
                .symbol(symbol, color)
                .folder(format!("{root}/{}", m.path))
                .action(Action::new("submodule-update", "Update").symbol("arrow.down.circle")),
        );
    }
    section
}

/// A tag as `for-each-ref refs/tags` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    pub when: String,
    pub subject: String,
}

impl Tag {
    /// Reads tab-separated `%(refname:short)`, `%(creatordate:relative)`, `%(subject)`.
    pub fn parse(output: &str) -> Vec<Tag> {
        output
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.splitn(3, '\t').collect();
                if parts.is_empty() || parts[0].trim().is_empty() {
                    return None;
                }
                Some(Tag {
                    name: parts[0].trim().to_owned(),
                    when: parts
                        .get(1)
                        .map(|p| p.trim().to_owned())
                        .unwrap_or_default(),
                    subject: parts
                        .get(2)
                        .map(|p| p.trim().to_owned())
                        .unwrap_or_default(),
                })
            })
            .collect()
    }
}

/// The Tags section of the Branches screen.
pub fn tags(list: &[Tag]) -> Section {
    let mut section = Section::new("tags", "Tags");
    for tag in list {
        let subtitle = [tag.when.as_str(), tag.subject.as_str()]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" \u{00B7} ");
        section = section.row(
            ScreenRow::new(format!("tag:{}", tag.name), tag.name.clone())
                .subtitle(subtitle)
                .symbol("tag", "gray")
                .action(Action::new("checkout", "Check Out").symbol("arrow.right.circle"))
                .action(
                    Action::new("tag-delete", "Delete")
                        .symbol("trash")
                        .destructive(format!(
                            "Delete the tag {}? If it was pushed, it stays on the remote.",
                            tag.name
                        )),
                )
                .primary("checkout"),
        );
    }
    section
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(hash: &str, parents: &[&str], subject: &str) -> Commit {
        Commit {
            hash: hash.into(),
            short: hash[..1].into(),
            parents: parents.iter().map(|p| (*p).to_owned()).collect(),
            author: "A".into(),
            when: "now".into(),
            refs: String::new(),
            subject: subject.into(),
        }
    }

    #[test]
    fn a_line_of_commits_stays_in_one_lane() {
        let commits = [
            commit("c", &["b"], "C"),
            commit("b", &["a"], "B"),
            commit("a", &[], "A"),
        ];
        let cells = layout(&commits);
        assert!(cells.iter().all(|c| c.lane == 0));
        assert_eq!(cells[0].below, vec![(0, 0)]);
        assert_eq!(cells[1].above, vec![(0, 0)]);
        assert!(cells[2].below.is_empty());
    }

    #[test]
    fn a_merge_opens_a_lane_and_the_branch_closes_it() {
        // m merges f into d; f branched from a; d's parent is a.
        //   m
        //   |\
        //   d f
        //   |/
        //   a
        let commits = [
            commit("m", &["d", "f"], "Merge"),
            commit("d", &["a"], "D"),
            commit("f", &["a"], "F"),
            commit("a", &[], "A"),
        ];
        let cells = layout(&commits);
        assert_eq!(cells[0].lane, 0);
        assert_eq!(cells[0].below, vec![(0, 0), (0, 1)]);
        assert_eq!(cells[1].lane, 0);
        assert_eq!(cells[1].above, vec![(0, 0), (1, 1)]);
        assert_eq!(cells[1].below, vec![(0, 0), (1, 1)]);
        assert_eq!(cells[2].lane, 1);
        // f's parent a is already awaited in lane 0: its line bends there, and lane 0 goes on.
        assert_eq!(cells[2].below, vec![(1, 0), (0, 0)]);
        assert_eq!(cells[3].lane, 0);
        assert_eq!(cells[3].above, vec![(0, 0)]);
        assert!(cells[3].below.is_empty());
    }

    #[test]
    fn commits_tags_and_worktrees_are_read_from_gits_lines() {
        let commits = Commit::parse(
            "abc\u{1f}a\u{1f}def ghi\u{1f}Ann\u{1f}2 days ago\u{1f}HEAD -> main, tag: v1, origin/main\u{1f}Merge it\n",
        );
        assert_eq!(commits[0].parents, vec!["def", "ghi"]);
        assert_eq!(commits[0].badge(), "main \u{00B7} v1 \u{00B7} origin/main");
        let screen = history(&commits, true);
        assert_eq!(screen.toolbar[0].id, "history-more");
        assert_eq!(
            screen.sections[0].rows[0].badge,
            "main \u{00B7} v1 \u{00B7} origin/main"
        );
        assert!(screen.sections[0].rows[0].graph.is_some());

        let trees = Worktree::parse(
            "worktree /r\nHEAD 0123456789\nbranch refs/heads/main\n\nworktree /r-feature\nHEAD 89abcdef0\nbranch refs/heads/feature\n\nworktree /r-detached\nHEAD 1111111aa\ndetached\n",
        );
        assert_eq!(trees.len(), 3);
        assert!(trees[0].main && trees[0].branch == "main");
        assert_eq!(
            (trees[2].branch.as_str(), trees[2].head.as_str()),
            ("", "1111111")
        );
        let screen = worktrees(&trees);
        assert_eq!(screen.sections[0].rows[0].badge, "main worktree");
        assert_eq!(screen.sections[0].rows[1].folder, "/r-feature");
        assert_eq!(screen.sections[0].rows[1].actions[0].id, "worktree-remove");

        let modules = Submodule::parse(
            " 0123456789abcdef libs/one (v1.2-3-g0123456)\n-fedcba9876543210 libs/two\n+aaaaaaa libs/three (heads/main)\n",
        );
        assert_eq!(modules.len(), 3);
        assert_eq!(
            (
                modules[0].path.as_str(),
                modules[0].mark,
                modules[0].describe.as_str()
            ),
            ("libs/one", ' ', "v1.2-3-g0123456")
        );
        assert_eq!(
            (modules[1].state(), modules[2].state()),
            ("not set up", "at another commit than recorded")
        );
        let section = submodules(&modules, "/r/");
        assert_eq!(section.rows[1].folder, "/r/libs/two");
        let tags = Tag::parse("v1\t3 days ago\tFirst release\nv2\t\t\n");
        assert_eq!(tags.len(), 2);
        let section = super::tags(&tags);
        assert_eq!(section.rows[0].id, "tag:v1");
        assert_eq!(
            section.rows[0].subtitle,
            "3 days ago \u{00B7} First release"
        );
    }
}
