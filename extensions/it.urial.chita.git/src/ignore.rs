//! `.gitignore` kept from the file list: patterns for items, for a kind of file, for a
//! folder; lines added to what's there without repeating one. Pure.

/// The pattern that ignores one item named `name` in the folder the file is in (`/` first,
/// so it matches there only; a folder ends in `/`).
pub fn pattern_for(name: &str, is_folder: bool) -> String {
    let name = name.trim_end_matches('/');
    let escaped = name
        .replace('\\', "\\\\")
        .replace('*', "\\*")
        .replace('?', "\\?")
        .replace('[', "\\[")
        .replace('#', "\\#")
        .replace('!', "\\!");
    if is_folder {
        format!("/{escaped}/")
    } else {
        format!("/{escaped}")
    }
}

/// The pattern that ignores every file with `name`'s extension, anywhere below; `None`
/// when it has none.
pub fn pattern_for_kind(name: &str) -> Option<String> {
    let name = name.trim_end_matches('/');
    let (stem, ext) = name.rsplit_once('.')?;
    if stem.is_empty() || ext.is_empty() || ext.contains('/') {
        return None;
    }
    Some(format!("*.{ext}"))
}

/// `text` with `lines` added at the end (each once; a heading line before the first when
/// the file isn't empty and has none of them yet).
pub fn with_lines(text: &str, lines: &[String]) -> String {
    let present: Vec<&str> = text.lines().map(str::trim).collect();
    let new: Vec<&String> = lines
        .iter()
        .filter(|l| !present.contains(&l.as_str()))
        .collect();
    if new.is_empty() {
        return text.to_owned();
    }
    let mut out = text.to_owned();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() && !out.ends_with("\n\n") && !present.contains(&"# Ignored from Chita") {
        out.push_str("\n# Ignored from Chita\n");
    }
    for line in new {
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_and_lines() {
        assert_eq!(pattern_for("build", true), "/build/");
        assert_eq!(pattern_for("a b.txt", false), "/a b.txt");
        assert_eq!(pattern_for("#1!.log", false), "/\\#1\\!.log");
        assert_eq!(pattern_for_kind("x.log"), Some("*.log".into()));
        assert_eq!(pattern_for_kind("Makefile"), None);
        assert_eq!(pattern_for_kind(".env"), None);
        let once = with_lines("node_modules/\n", &["/build/".into(), "*.log".into()]);
        assert_eq!(
            once,
            "node_modules/\n\n# Ignored from Chita\n/build/\n*.log\n"
        );
        assert_eq!(with_lines(&once, &["*.log".into()]), once);
        assert_eq!(
            with_lines(&once, &["/dist/".into()]),
            format!("{once}/dist/\n")
        );
        assert_eq!(with_lines("", &["/x".into()]), "/x\n");
    }
}
