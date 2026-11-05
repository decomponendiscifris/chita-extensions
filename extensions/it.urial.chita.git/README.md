# Git

Shows what changed in a repository, from the folder you're viewing in Chita:

- a **Git column** with each item's state ("Modified", "Added (staged)", "Untracked",
  "Conflict", "2 changes" for a folder);
- a **Git panel** in the Inspector: the branch, the upstream with what's ahead and behind,
  the changes, the last commit; nothing outside a repository;
- commands: **Stage**, **Unstage**, **Commit…** (asks for the message), **Show Changes**,
  **Show History**, **Fetch**, **Pull**, **Push**.

Everything is the real `git` on your Mac, run by Chita in the folder you're viewing under
Chita's rules for it: everyday subcommands only, nothing that makes git run something else or
reach outside the folder, and the network only through the repository's own remotes by
name. A password or a token git needs comes to a sheet in Chita; Chita keeps nothing.

Permissions: the folder you're viewing (read), and the tool `git`.

Developed in Chita's repository and mirrored here; this folder is the source the store
builds and signs.
