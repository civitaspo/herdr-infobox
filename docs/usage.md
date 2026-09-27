# Read a session in Info

Open Info from Herdr's Toggle Info action, or run `herdr-infobox ui --session SESSION`. A manual session picker is available when automatic identity cannot be established. Pinning keeps the chosen session visible while you focus another agent.

| Key | Action |
| --- | --- |
| `s` | Open the session picker |
| `Tab`, `Shift-Tab` | Select repositories, references, or plans |
| `j`, `k`, arrows | Select an item or scroll detail |
| `Enter` | Open the selected detail or URL |
| `d` | Open the selected worktree's changed-file list |
| `1`, `2`, `3` | Select unstaged, staged, or untracked scope |
| `r` | Refresh the selected Git view |
| `a` | Export the retained diff and open copy-only annotation |
| `p` | Pin or unpin the current session |
| `/` | Filter the current section |
| `c` | Collapse or expand the current section |
| `o` | Open a repository URL, reference URL, or Plan source file |
| `y` | Copy a worktree path, reference URL, or Plan body |
| `Esc` | Return from detail |
| `q` | Close the terminal UI |

The diff describes the worktree's current state, including changes made by other people or agents. It does not attribute changes to the selected session. Use the CLI's `diff --scope branch --base REF` for committed changes relative to a resolved merge base. Conflicts remain explicit rather than appearing as an ordinary two-way patch.

Reference relations distinguish search results, requested URLs, successful fetches, citations, and manual additions. A missing title stays unavailable. Plan approval does not mean execution started. Task completion does not mark the Plan complete.

When annotate is unavailable, `snapshot export --session SESSION --worktree ID --scope unstaged` still produces an immutable export. Non-UTF-8 patches produce a raw `.patch` export instead of an invalid Markdown document. `snapshot list` shows retained exports. Purge only an explicit full hash after reviewing whether its annotation history is still needed.
