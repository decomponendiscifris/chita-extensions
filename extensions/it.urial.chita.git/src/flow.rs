//! Flow: feature, release and hotfix branches by the gitflow conventions the repository
//! is set up with (`gitflow.branch.*`, `gitflow.prefix.*`, as git-flow and git-flow-next
//! keep them), and the commit message convention (`chita.commits`). Pure: the git runs
//! are in `lib.rs`.

use chita_ext_sdk::{Action, Screen, ScreenRow, Section};

/// The flow's branches and prefixes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conventions {
    /// Releases land here ("main").
    pub master: String,
    /// Features land here ("develop").
    pub develop: String,
    pub feature: String,
    pub release: String,
    pub hotfix: String,
    /// Before a release's tag ("v").
    pub versiontag: String,
    /// Set up in the repository's configuration (else these are defaults).
    pub configured: bool,
}

impl Conventions {
    /// From `git config --get-regexp ^gitflow\.` output ("gitflow.branch.master main"),
    /// with `main_branch` as the default master when nothing is set.
    pub fn parse(output: &str, main_branch: &str) -> Conventions {
        let mut c = Conventions {
            master: main_branch.to_owned(),
            develop: "develop".into(),
            feature: "feature/".into(),
            release: "release/".into(),
            hotfix: "hotfix/".into(),
            versiontag: String::new(),
            configured: false,
        };
        for line in output.lines() {
            let (key, value) = line.split_once(' ').unwrap_or((line, ""));
            let value = value.trim().to_owned();
            match key.trim() {
                "gitflow.branch.master" => c.master = value,
                "gitflow.branch.develop" => c.develop = value,
                "gitflow.prefix.feature" => c.feature = value,
                "gitflow.prefix.release" => c.release = value,
                "gitflow.prefix.hotfix" => c.hotfix = value,
                "gitflow.prefix.versiontag" => c.versiontag = value,
                _ => continue,
            }
            c.configured = true;
        }
        c
    }

    /// The configuration lines that set these up.
    pub fn settings(&self) -> Vec<(&'static str, String)> {
        vec![
            ("gitflow.branch.master", self.master.clone()),
            ("gitflow.branch.develop", self.develop.clone()),
            ("gitflow.prefix.feature", self.feature.clone()),
            ("gitflow.prefix.release", self.release.clone()),
            ("gitflow.prefix.hotfix", self.hotfix.clone()),
            ("gitflow.prefix.versiontag", self.versiontag.clone()),
        ]
    }

    /// Which kind of branch `name` is, and its short name.
    pub fn kind_of<'a>(&self, name: &'a str) -> Option<(Kind, &'a str)> {
        if let Some(rest) = name.strip_prefix(&self.feature) {
            Some((Kind::Feature, rest))
        } else if let Some(rest) = name.strip_prefix(&self.release) {
            Some((Kind::Release, rest))
        } else if let Some(rest) = name.strip_prefix(&self.hotfix) {
            Some((Kind::Hotfix, rest))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Feature,
    Release,
    Hotfix,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Feature => "feature",
            Kind::Release => "release",
            Kind::Hotfix => "hotfix",
        }
    }

    pub fn prefix(self, c: &Conventions) -> &str {
        match self {
            Kind::Feature => &c.feature,
            Kind::Release => &c.release,
            Kind::Hotfix => &c.hotfix,
        }
    }

    /// Where it starts from.
    pub fn base(self, c: &Conventions) -> &str {
        match self {
            Kind::Feature | Kind::Release => &c.develop,
            Kind::Hotfix => &c.master,
        }
    }
}

/// The commit message convention the repository asks for (`chita.commits`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommitStyle {
    /// Anything goes.
    #[default]
    Plain,
    /// A short imperative subject: capitalised, no trailing period, at most 72 characters.
    Imperative,
    /// Conventional Commits: `type(scope): subject`.
    Conventional,
}

impl CommitStyle {
    pub fn parse(value: &str) -> CommitStyle {
        match value.trim() {
            "imperative" => CommitStyle::Imperative,
            "conventional" => CommitStyle::Conventional,
            _ => CommitStyle::Plain,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            CommitStyle::Plain => "plain",
            CommitStyle::Imperative => "imperative",
            CommitStyle::Conventional => "conventional",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            CommitStyle::Plain => "No convention",
            CommitStyle::Imperative => "Imperative subject",
            CommitStyle::Conventional => "Conventional Commits",
        }
    }

    /// What's wrong with `message` under this style, in a sentence; `None` when it's fine.
    pub fn check(self, message: &str) -> Option<String> {
        let subject = message.lines().next().unwrap_or("").trim();
        if subject.is_empty() {
            return Some("The first line is the subject; it's empty.".into());
        }
        match self {
            CommitStyle::Plain => None,
            CommitStyle::Imperative => {
                if subject.chars().count() > 72 {
                    return Some(format!(
                        "The subject is {} characters; keep it to 72, and put the rest after an empty line.",
                        subject.chars().count()
                    ));
                }
                if subject.ends_with('.') {
                    return Some("The subject ends with a period; leave it out.".into());
                }
                if subject.chars().next().is_some_and(|c| c.is_lowercase()) {
                    return Some("The subject starts with a small letter; capitalise it.".into());
                }
                let first = subject
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_lowercase();
                if (first.ends_with("ed") && first.len() > 3 && first != "embed" && first != "shed")
                    || (first.ends_with("ing") && first.len() > 4)
                    || first == "fixes"
                    || first == "adds"
                    || first == "changes"
                    || first == "updates"
                {
                    return Some(format!(
                        "The subject starts with \u{201C}{}\u{201D}; say what the commit does, as an order: \u{201C}Fix\u{201D}, \u{201C}Add\u{201D}, \u{201C}Change\u{201D}.",
                        subject.split_whitespace().next().unwrap_or("")
                    ));
                }
                None
            }
            CommitStyle::Conventional => {
                let Some((head, rest)) = subject.split_once(':') else {
                    return Some("Conventional Commits: \u{201C}type(scope): subject\u{201D}, as in \u{201C}fix(parser): handle empty input\u{201D}.".into());
                };
                let kind = head.trim_end_matches('!');
                let kind = kind.split_once('(').map(|(k, _)| k).unwrap_or(kind);
                const TYPES: &[&str] = &[
                    "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci",
                    "chore", "revert",
                ];
                if !TYPES.contains(&kind) {
                    return Some(format!(
                        "\u{201C}{kind}\u{201D} isn't a Conventional Commits type: {}.",
                        TYPES.join(", ")
                    ));
                }
                if rest.trim().is_empty() {
                    return Some("Nothing after the colon: say what changed.".into());
                }
                None
            }
        }
    }
}

/// A flow branch as the screen lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowBranch {
    pub kind: Kind,
    pub name: String,
    pub short: String,
    pub current: bool,
    pub upstream: String,
    pub when: String,
    pub subject: String,
}

/// The Flow screen.
pub fn flow(
    c: &Conventions,
    branches: &[FlowBranch],
    style: CommitStyle,
    develop_exists: bool,
) -> Screen {
    let mut screen = Screen::new()
        .empty("No feature, release or hotfix branches yet: start one from the toolbar.")
        .toolbar(
            Action::new("flow-start-feature", "Start Feature\u{2026}")
                .symbol("plus")
                .prominent(),
        )
        .toolbar(Action::new("flow-start-release", "Start Release\u{2026}").symbol("shippingbox"))
        .toolbar(Action::new("flow-start-hotfix", "Start Hotfix\u{2026}").symbol("bandage"));
    if !c.configured || !develop_exists {
        screen = screen.toolbar(Action::new("flow-setup", "Set Up Flow").symbol("gearshape"));
    }
    for (kind, title) in [
        (Kind::Feature, "Features"),
        (Kind::Release, "Releases"),
        (Kind::Hotfix, "Hotfixes"),
    ] {
        let mut section = Section::new(kind.word(), title);
        for b in branches.iter().filter(|b| b.kind == kind) {
            let mut parts = vec![b.name.clone()];
            if !b.upstream.is_empty() {
                parts.push(format!("published as {}", b.upstream));
            }
            if !b.when.is_empty() {
                parts.push(format!("{} \u{00B7} {}", b.when, b.subject));
            }
            let finish_text = match kind {
                Kind::Feature => format!(
                    "Finish the feature {}? It's merged into {} and its branch deleted.",
                    b.short, c.develop
                ),
                Kind::Release => format!(
                    "Finish the release {}? It's merged into {}, tagged {}{}, merged back into {} and its branch deleted.",
                    b.short, c.master, c.versiontag, b.short, c.develop
                ),
                Kind::Hotfix => format!(
                    "Finish the hotfix {}? It's merged into {}, tagged {}{}, merged into {} and its branch deleted.",
                    b.short, c.master, c.versiontag, b.short, c.develop
                ),
            };
            let mut row = ScreenRow::new(format!("local:{}", b.name), b.short.clone())
                .subtitle(parts.join(" \u{00B7} "))
                .symbol(
                    match kind {
                        Kind::Feature => "leaf",
                        Kind::Release => "shippingbox",
                        Kind::Hotfix => "bandage",
                    },
                    if b.current { "green" } else { "gray" },
                )
                .action(
                    Action::new("flow-finish", "Finish")
                        .symbol("checkmark.circle")
                        .destructive(finish_text),
                )
                .action(Action::new("flow-publish", "Publish").symbol("arrow.up.to.line"));
            if b.current {
                row = row.badge("current");
            } else {
                row = row
                    .action(Action::new("checkout", "Check Out").symbol("arrow.right.circle"))
                    .primary("checkout");
            }
            section = section.row(row);
        }
        if !section.rows.is_empty() {
            screen = screen.section(section);
        }
    }
    let mut conventions = Section::new("conventions", "Conventions");
    conventions = conventions.row(
        ScreenRow::new(
            "branches",
            format!("Releases on {}, work on {}", c.master, c.develop),
        )
        .subtitle(format!(
            "Prefixes {}, {}, {}; tags {}",
            c.feature,
            c.release,
            c.hotfix,
            if c.versiontag.is_empty() {
                "without a prefix".to_owned()
            } else {
                format!("with \u{201C}{}\u{201D}", c.versiontag)
            }
        ))
        .symbol("arrow.triangle.branch", "accent")
        .badge(if c.configured { "set up" } else { "defaults" }),
    );
    conventions = conventions.row(
        ScreenRow::new("commits", format!("Commit messages: {}", style.title().to_lowercase()))
            .subtitle(match style {
                CommitStyle::Plain => "Commit… takes any message.",
                CommitStyle::Imperative => "A capitalised imperative subject, no trailing period, 72 characters at most; Commit… asks again when it isn't.",
                CommitStyle::Conventional => "type(scope): subject, with the usual types; Commit… asks again when it isn't.",
            })
            .symbol("text.quote", "accent")
            .action(Action::new("flow-style-imperative", "Imperative Subject").symbol("text.cursor"))
            .action(Action::new("flow-style-conventional", "Conventional Commits").symbol("text.badge.checkmark"))
            .action(Action::new("flow-style-plain", "No Convention").symbol("xmark.circle")),
    );
    screen.section(conventions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conventions_come_from_the_configuration_with_defaults() {
        let c = Conventions::parse(
            "gitflow.branch.master main\ngitflow.prefix.versiontag v\n",
            "master",
        );
        assert!(
            c.configured && c.master == "main" && c.develop == "develop" && c.versiontag == "v"
        );
        assert_eq!(c.kind_of("feature/x"), Some((Kind::Feature, "x")));
        assert_eq!(c.kind_of("hotfix/1.0.1"), Some((Kind::Hotfix, "1.0.1")));
        assert_eq!(c.kind_of("main"), None);
        let d = Conventions::parse("", "trunk");
        assert!(!d.configured && d.master == "trunk");
        assert_eq!(Kind::Hotfix.base(&d), "trunk");
        assert_eq!(Kind::Feature.base(&d), "develop");
        assert_eq!(d.settings().len(), 6);
    }

    #[test]
    fn commit_styles_check_the_subject() {
        let i = CommitStyle::Imperative;
        assert_eq!(i.check("Fix the parser on empty input"), None);
        assert!(i.check("fix it").unwrap().contains("small letter"));
        assert!(i.check("Fixed it").unwrap().contains("Fixed"));
        assert!(i.check("Adding things").unwrap().contains("Adding"));
        assert!(i.check("Fix it.").unwrap().contains("period"));
        assert!(i.check(&"A".repeat(80)).unwrap().contains("80 characters"));
        assert!(i.check("").unwrap().contains("empty"));
        let c = CommitStyle::Conventional;
        assert_eq!(c.check("fix(parser): handle empty input"), None);
        assert_eq!(c.check("feat!: drop the old API"), None);
        assert!(c.check("Fix the parser").unwrap().contains("type(scope)"));
        assert!(
            c.check("bug: x")
                .unwrap()
                .contains("isn't a Conventional Commits type")
        );
        assert!(c.check("fix:").unwrap().contains("Nothing after"));
        assert_eq!(CommitStyle::Plain.check("whatever"), None);
        assert_eq!(
            CommitStyle::parse("conventional"),
            CommitStyle::Conventional
        );
        assert_eq!(CommitStyle::parse("x"), CommitStyle::Plain);
    }

    #[test]
    fn the_flow_screen_lists_branches_by_kind() {
        let c = Conventions::parse("gitflow.branch.master main\n", "main");
        let branches = vec![
            FlowBranch {
                kind: Kind::Feature,
                name: "feature/a".into(),
                short: "a".into(),
                current: true,
                upstream: String::new(),
                when: "now".into(),
                subject: "S".into(),
            },
            FlowBranch {
                kind: Kind::Release,
                name: "release/1.0".into(),
                short: "1.0".into(),
                current: false,
                upstream: "origin/release/1.0".into(),
                when: "now".into(),
                subject: "S".into(),
            },
        ];
        let screen = flow(&c, &branches, CommitStyle::Imperative, true);
        assert_eq!(
            screen
                .sections
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            vec!["feature", "release", "conventions"]
        );
        assert_eq!(screen.sections[0].rows[0].badge, "current");
        assert!(
            screen.sections[1].rows[0]
                .subtitle
                .contains("published as origin/release/1.0")
        );
        assert!(
            screen.sections[1].rows[0]
                .actions
                .iter()
                .any(|a| a.id == "checkout")
        );
        assert!(screen.toolbar.iter().all(|a| a.id != "flow-setup"));
        let unset = flow(
            &Conventions::parse("", "main"),
            &[],
            CommitStyle::Plain,
            false,
        );
        assert!(unset.toolbar.iter().any(|a| a.id == "flow-setup"));
        assert_eq!(unset.sections[0].rows[0].badge, "defaults");
    }
}
