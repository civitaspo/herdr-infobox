# Local Git collection

Info reads the current worktree, including edits from other people or agents. It does not attribute every change to the selected session. Unstaged compares the index to the worktree. Staged compares HEAD to the index, including an unborn HEAD. Untracked reads one selected regular file or symlink target without following the symlink. Branch compares the merge-base of an explicit base and HEAD to HEAD. Conflicts use a labeled combined diff.

Commands disable optional index locks, pagers, color, external diff drivers, text conversion, and fsmonitor. Arguments use literal pathspecs. Collection never stages, checks out, fetches, commits, or pushes. Commands have a five-second limit and two-MiB output limit. Metadata changes trigger one retry. Continued changes produce `Changed while reading`. Metadata checking cannot guarantee an atomic filesystem snapshot.

Repository identity uses the canonical Git common directory. Linked worktrees retain separate Git directories and roots. Remote URLs do not merge independent checkouts. Missing paths resolve from the closest existing parent, so newly proposed files can identify their repository.

GitHub remote selection checks an explicit `infobox.remote`, the branch upstream remote, `origin`, then a sole remaining remote. Ambiguous remotes produce no link. SSH aliases and GitHub Enterprise hosts require explicit mappings. The collector reads these Git config keys without writing them. Users can configure a repository themselves.

```sh
git config infobox.remote work
git config --add infobox.github-host 'work-alias=github.example.com'
git config --add infobox.github-host 'github.example.com=github.example.com'
```

Each mapping has `remote-host=web-host` form. An optional HTTPS port is allowed on the web host. Credentials, paths, queries, and fragments are rejected there. Repository URLs remove credentials and one trailing `.git`. No GitHub token or network request is needed. A generated link does not prove that a branch or commit has been pushed.

Annotation exports the exact retained patch, comparison commits, worktree, session, timestamp, and hash. A refresh never overwrites a previous snapshot. Export publishes a completed read-only file atomically and checks existing contents before reuse. Non-UTF-8 patches are saved as exact `.patch` files with a diagnostic; the Markdown reviewer requires UTF-8. Raw path bytes remain separate from escaped display paths. Actual non-UTF-8 filename creation requires Linux verification because this macOS filesystem rejected it; NUL-delimited parser fixtures cover those bytes.

Copy review requires registered Annotate Full 0.6.0 with plannotator-tui 0.9.4. Info rechecks the target pane identity and opens its own review pane. That pane verifies the snapshot hash and removes delivery environment variables before launching the reviewer. Send is unavailable because the reviewed upstream implementation does not compare native sessions immediately before delivery. OSC 52 clipboard forwarding and live pane behavior still require manual verification.
