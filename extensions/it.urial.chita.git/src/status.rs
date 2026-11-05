//! `git status --porcelain=v2 -z --branch` read into what the column and the panel show.
//! Pure: no git here.

/// A repository's state as `git status` reports it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    pub branch: Option<Branch>,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Branch {
    /// The branch, or "(detached)".
    pub head: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// No commits yet.
    pub initial: bool,
}

/// One changed, untracked, ignored or conflicted path, relative to the repository's root
/// (an untracked folder ends in `/`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// `index` is the change staged (`.` for none), `worktree` the one not staged.
    Tracked {
        index: char,
        worktree: char,
        renamed_from: Option<String>,
    },
    Unmerged,
    Untracked,
    Ignored,
}

impl Status {
    /// Reads the `-z` output: records end in NUL; a rename's original path follows as one
    /// more NUL-ended field.
    pub fn parse(output: &[u8]) -> Status {
        let text = String::from_utf8_lossy(output);
        let mut fields = text.split('\0').filter(|f| !f.is_empty());
        let mut status = Status::default();
        let mut branch = Branch::default();
        let mut has_branch = false;
        while let Some(record) = fields.next() {
            let mut parts = record.splitn(2, ' ');
            let tag = parts.next().unwrap_or_default();
            let rest = parts.next().unwrap_or_default();
            match tag {
                "#" => {
                    has_branch = true;
                    let (key, value) = rest.split_once(' ').unwrap_or((rest, ""));
                    match key {
                        "branch.oid" => branch.initial = value == "(initial)",
                        "branch.head" => branch.head = value.to_owned(),
                        "branch.upstream" => branch.upstream = Some(value.to_owned()),
                        "branch.ab" => {
                            for part in value.split(' ') {
                                if let Some(n) = part.strip_prefix('+') {
                                    branch.ahead = n.parse().unwrap_or(0);
                                } else if let Some(n) = part.strip_prefix('-') {
                                    branch.behind = n.parse().unwrap_or(0);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                "1" | "2" => {
                    // XY sub mH mI mW hH hI [Xscore] path
                    let n = if tag == "1" { 8 } else { 9 };
                    let mut words = rest.splitn(n, ' ');
                    let xy = words.next().unwrap_or_default();
                    let path = words.nth(n - 2).unwrap_or_default().to_owned();
                    let mut chars = xy.chars();
                    let index = chars.next().unwrap_or('.');
                    let worktree = chars.next().unwrap_or('.');
                    let renamed_from = if tag == "2" {
                        fields.next().map(str::to_owned)
                    } else {
                        None
                    };
                    status.changes.push(Change {
                        path,
                        kind: Kind::Tracked {
                            index,
                            worktree,
                            renamed_from,
                        },
                    });
                }
                "u" => {
                    // XY sub m1 m2 m3 mW h1 h2 h3 path
                    let path = rest.splitn(10, ' ').nth(9).unwrap_or_default().to_owned();
                    status.changes.push(Change {
                        path,
                        kind: Kind::Unmerged,
                    });
                }
                "?" => status.changes.push(Change {
                    path: rest.to_owned(),
                    kind: Kind::Untracked,
                }),
                "!" => status.changes.push(Change {
                    path: rest.to_owned(),
                    kind: Kind::Ignored,
                }),
                _ => {}
            }
        }
        if has_branch {
            status.branch = Some(branch);
        }
        status
    }

    /// The text for each of `paths` (full paths inside `folder`, which is `prefix` below the
    /// repository's root, `""` at it): the item's own state, or for a folder how much changed
    /// inside it; empty for what's unchanged or outside the folder.
    pub fn column(&self, folder: &str, prefix: &str, paths: &[String]) -> Vec<String> {
        let folder = folder.trim_end_matches('/');
        paths
            .iter()
            .map(|path| {
                let Some(name) = path.strip_prefix(folder).and_then(|r| r.strip_prefix('/')) else {
                    return String::new();
                };
                let relative = format!("{prefix}{name}");
                if let Some(change) = self.changes.iter().find(|c| c.path == relative) {
                    return change.text();
                }
                let inside = format!("{relative}/");
                if self
                    .changes
                    .iter()
                    .any(|c| c.path == inside && c.kind == Kind::Untracked)
                {
                    return "Untracked".into();
                }
                if self
                    .changes
                    .iter()
                    .any(|c| c.path == inside && c.kind == Kind::Ignored)
                {
                    return "Ignored".into();
                }
                let count = self
                    .changes
                    .iter()
                    .filter(|c| c.path.starts_with(&inside) && c.kind != Kind::Ignored)
                    .count();
                match count {
                    0 => String::new(),
                    1 => "1 change".into(),
                    n => format!("{n} changes"),
                }
            })
            .collect()
    }

    /// Staged, not staged, untracked and conflicted, in words ("2 staged, 1 modified").
    pub fn summary(&self) -> String {
        let (mut staged, mut unstaged, mut untracked, mut conflicts) = (0, 0, 0, 0);
        for change in &self.changes {
            match &change.kind {
                Kind::Tracked {
                    index, worktree, ..
                } => {
                    if *index != '.' {
                        staged += 1;
                    }
                    if *worktree != '.' {
                        unstaged += 1;
                    }
                }
                Kind::Unmerged => conflicts += 1,
                Kind::Untracked => untracked += 1,
                Kind::Ignored => {}
            }
        }
        let mut parts = Vec::new();
        if conflicts > 0 {
            parts.push(format!("{conflicts} in conflict"));
        }
        if staged > 0 {
            parts.push(format!("{staged} staged"));
        }
        if unstaged > 0 {
            parts.push(format!("{unstaged} modified"));
        }
        if untracked > 0 {
            parts.push(format!("{untracked} untracked"));
        }
        if parts.is_empty() {
            "None".into()
        } else {
            parts.join(", ")
        }
    }
}

impl Change {
    /// "Modified", "Added (staged)", "Modified, partly staged", "Untracked"…
    pub fn text(&self) -> String {
        match &self.kind {
            Kind::Untracked => "Untracked".into(),
            Kind::Ignored => "Ignored".into(),
            Kind::Unmerged => "Conflict".into(),
            Kind::Tracked {
                index, worktree, ..
            } => match (*index != '.', *worktree != '.') {
                (false, false) => String::new(),
                (true, false) => format!("{} (staged)", word(*index)),
                (false, true) => word(*worktree).to_owned(),
                (true, true) => format!("{}, partly staged", word(*worktree)),
            },
        }
    }
}

impl Change {
    /// What's staged of it ("Added"), when something is.
    pub fn staged_word(&self) -> Option<&'static str> {
        match &self.kind {
            Kind::Tracked { index, .. } if *index != '.' => Some(word(*index)),
            _ => None,
        }
    }

    /// What's changed and not staged ("Modified"), when something is.
    pub fn unstaged_word(&self) -> Option<&'static str> {
        match &self.kind {
            Kind::Tracked { worktree, .. } if *worktree != '.' => Some(word(*worktree)),
            _ => None,
        }
    }
}

fn word(code: char) -> &'static str {
    match code {
        'M' => "Modified",
        'A' => "Added",
        'D' => "Deleted",
        'R' => "Renamed",
        'C' => "Copied",
        'T' => "Type changed",
        'U' => "Conflict",
        _ => "Changed",
    }
}

impl Branch {
    /// "origin/main, 2 ahead, 1 behind", or "None".
    pub fn upstream_text(&self) -> String {
        let Some(upstream) = &self.upstream else {
            return "None".into();
        };
        let mut text = upstream.clone();
        if self.ahead > 0 {
            text.push_str(&format!(", {} ahead", self.ahead));
        }
        if self.behind > 0 {
            text.push_str(&format!(", {} behind", self.behind));
        }
        if self.ahead == 0 && self.behind == 0 {
            text.push_str(", up to date");
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"# branch.oid 1234\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -1\0\
1 .M N... 100644 100644 100644 abc def src/lib.rs\0\
1 A. N... 000000 100644 100644 000 abc new file.txt\0\
1 MM N... 100644 100644 100644 a b both.txt\0\
2 R. N... 100644 100644 100644 a b R100 renamed.txt\0old.txt\0\
u UU N... 100644 100644 100644 100644 a b c conflict.txt\0\
? untracked.txt\0? newdir/\0! ignored.txt\0";

    #[test]
    fn the_branch_and_every_change_are_read() {
        let status = Status::parse(SAMPLE);
        let branch = status.branch.as_ref().unwrap();
        assert_eq!(branch.head, "main");
        assert_eq!(branch.upstream.as_deref(), Some("origin/main"));
        assert_eq!((branch.ahead, branch.behind), (2, 1));
        assert!(!branch.initial);
        assert_eq!(branch.upstream_text(), "origin/main, 2 ahead, 1 behind");
        let texts: Vec<(String, String)> = status
            .changes
            .iter()
            .map(|c| (c.path.clone(), c.text()))
            .collect();
        assert_eq!(
            texts,
            vec![
                ("src/lib.rs".into(), "Modified".into()),
                ("new file.txt".into(), "Added (staged)".into()),
                ("both.txt".into(), "Modified, partly staged".into()),
                ("renamed.txt".into(), "Renamed (staged)".into()),
                ("conflict.txt".into(), "Conflict".into()),
                ("untracked.txt".into(), "Untracked".into()),
                ("newdir/".into(), "Untracked".into()),
                ("ignored.txt".into(), "Ignored".into()),
            ]
        );
        assert_eq!(
            status.changes[3].kind,
            Kind::Tracked {
                index: 'R',
                worktree: '.',
                renamed_from: Some("old.txt".into())
            }
        );
        assert_eq!(
            status.summary(),
            "1 in conflict, 3 staged, 2 modified, 2 untracked"
        );
    }

    #[test]
    fn the_column_follows_the_folder_and_the_prefix() {
        let status = Status::parse(SAMPLE);
        // At the root.
        let paths: Vec<String> = ["src", "new file.txt", "newdir", "clean.txt", "/elsewhere/x"]
            .iter()
            .map(|n| {
                if n.starts_with('/') {
                    (*n).to_owned()
                } else {
                    format!("/repo/{n}")
                }
            })
            .collect();
        assert_eq!(
            status.column("/repo", "", &paths),
            vec!["1 change", "Added (staged)", "Untracked", "", ""]
        );
        // In a subfolder: the prefix is what git's paths start with.
        assert_eq!(
            status.column("/repo/src/", "src/", &["/repo/src/lib.rs".to_owned()]),
            vec!["Modified"]
        );
        // Two changes below a folder.
        let two = Status::parse(b"1 .M N... 1 1 1 a b d/a\0? d/b\0! d/c\0");
        assert_eq!(
            two.column("/r", "", &["/r/d".to_owned()]),
            vec!["2 changes"]
        );
    }

    #[test]
    fn an_empty_repository_and_no_branch() {
        let status = Status::parse(b"# branch.oid (initial)\0# branch.head main\0");
        assert!(status.branch.as_ref().unwrap().initial);
        assert_eq!(status.branch.as_ref().unwrap().upstream_text(), "None");
        assert_eq!(status.summary(), "None");
        assert_eq!(Status::parse(b"").branch, None);
        let detached = Status::parse(b"# branch.oid 1\0# branch.head (detached)\0");
        assert_eq!(detached.branch.unwrap().head, "(detached)");
    }
}
