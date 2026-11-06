# Git

A repository as a place in Chita, from the folder you're viewing:

- a **header** in Git's colour with the branch, the upstream with what's ahead and behind,
  what changed, and a merge, rebase, cherry-pick or revert that stopped, with Fetch, Pull,
  Push and Commit… as buttons and a switch between Files, Changes, Branches, History and
  Worktrees;
- **Changes**: conflicts, staged, changed, untracked and stashes, with Stage, Unstage,
  Mark Resolved, Discard (a copy is kept for a week: Undo Discard), Commit…, Amend…,
  Stash All…, and the diff of what you select in the Inspector;
- **Branches** and tags: check out, merge into the current one, rebase onto it, delete,
  new branch, new tag, push and force push with lease; **History** with the commit graph,
  a branch or a tag from a commit, cherry-pick and revert; **Worktrees** and submodules to
  list, add next to the repository, open, update and remove; **Flow** with feature,
  release and hotfix branches by the repository's gitflow conventions, and the commit
  message convention Commit… checks;
- a **Git column** with each item's state ("Modified", "Added (staged)", "Untracked",
  "Ignored", "2 changes" for a folder) and a **Git panel** in the Inspector;
- **.gitignore** from the menu: Ignore, Ignore All Files of This Kind, Ignore on This Mac
  Only, Stop Tracking and Keep the File, Edit .gitignore… with templates to insert;
- **Clone Repository Here…** from an address you type into Chita's own prompt, **Make a
  Repository Here**, and **History of This File**.

Everything is the real `git` on your Mac, run by Chita in the folder you're viewing under
Chita's rules for it: everyday subcommands only, nothing that makes git run something else or
reach outside the folder, and the network only through the repository's own remotes by
name. A password or a token git needs comes to a sheet in Chita; Chita keeps nothing.

Permissions: the folder you're viewing (read and write, for .gitignore), and the tool `git`.

Developed in Chita's repository and mirrored here; this folder is the source the store
builds and signs.

Source: `src/lib.rs` and `src/status.rs` (the status parser, tested on sample output).
