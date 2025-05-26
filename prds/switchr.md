# Project Switcher – Rust Rewrite

## 1  Purpose

Provide a single‑binary command‑line tool, **`sw`**, that lets developers switch to projects instantly by typing part of the project name. It discovers projects from local folders, recent editor workspaces, and GitHub, ranks them with fuzzy search, and opens the chosen project in the user's preferred editor.

## 2  Scope & Goals

* **Parity**: Match all capabilities of the current Python implementation.
* **Performance**: Cold‑start in < 200 ms with cached data; interactive keystroke latency < 30 ms.
* **Zero‑setup binary**: Distribute via `cargo install sw` or a pre‑built binary—no Python or virtualenv.

## 3  Stakeholders

| Role            | Interest                                   |
| --------------- | ------------------------------------------ |
| Individual devs | Instant context‑switching across projects. |
| Team leads      | Consistent workflow across machines.       |
| OSS maintainers | Simple contribution model in Rust.         |

## 4  Functional Requirements

|  ID       | Requirement                                                                                                                                                   |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **FR‑01** | `sw` SHALL support these CLI flags: `--help`, `--list`, `--interactive` (default), `--fzf`, `--refresh`, `--setup`, and `<project‑name>` positional argument. |
| **FR‑02** | In *interactive* mode, the tool SHALL display a real‑time fuzzy‑search prompt with ranked completions for at least the 10 best matches.                       |
| **FR‑03** | The tool SHALL discover *local* projects by scanning all directories configured in *project\_dirs*. Only directories containing a `.git` folder are considered projects. |
| **FR‑04** | The tool SHALL discover *Cursor* workspaces by reading workspace JSON files under `~/Library/Application Support/Cursor/User/workspaceStorage`. All valid workspaces are included regardless of Git status. |
| **FR‑05** | The tool SHALL discover *GitHub* repositories by using the `gh` CLI tool to query the GitHub API. If `gh` is not installed, a warning SHALL be displayed. |
| **FR‑06** | GitHub authentication SHALL be handled exclusively through `gh auth login`. If the user is not authenticated, the tool SHALL prompt to run `gh auth login`. |
| **FR‑07** | The tool SHALL automatically run `gh auth login` if the user confirms when GitHub access is requested but authentication is missing. |
| **FR‑08** | The tool SHALL deduplicate projects to avoid showing the same repository as both GitHub and Local. Local Git repositories take precedence over remote GitHub entries. |
| **FR‑09** | For each project, the tool SHALL capture the latest activity timestamp (commit time, index mtime, or directory mtime) when available. |
| **FR‑10** | Projects SHALL be sorted descending by `last_modified`; alphabetically when timestamps are equal / missing. |
| **FR‑11** | Selecting a GitHub project SHALL clone it under `~/Documents/git/<repo>` *if it is not already cloned* and then open it. |
| **FR‑12** | Upon opening a project, `sw` SHALL launch `editor_command <project_path>`. |
| **FR‑13** | The *setup* wizard SHALL prompt for (or auto‑detect) `editor_command`, `project_dirs`, and GitHub username, saving them to `~/.config/sw/config.json`. |
| **FR‑14** | A *refresh* command SHALL invalidate caches and rebuild the project list. |
| **FR‑15** | A *list* command SHALL output project names with formatted metadata for shell completion. |
| **FR‑16** | A *fzf* mode SHALL pipe the project list through the system `fzf` binary and open the selected project. |
| **FR‑17** | The installer SHALL generate Bash, Zsh, and Fish completion scripts. |

## 5  Non‑Functional Requirements

|  ID       | Requirement                                                                                            |
| ---------- | ------------------------------------------------------------------------------------------------------ |
| **NFR‑01** | Cold‑start **< 200 ms** when cache is valid; **< 5 s** when rebuilding.                                |
| **NFR‑02** | Memory footprint **< 25 MB** resident after startup.                                                   |
| **NFR‑03** | Works on macOS (>= 11) and Linux (glibc >= 2.31).                                                      |
| **NFR‑04** | Concurrency‑safe cache writes; no data loss on interruption.                                           |
| **NFR‑05** | ≥ 90 % unit‑test line coverage; CI runs on push & PR.                                                  |
| **NFR‑06** | The codebase SHALL follow Rust 2021 edition and `cargo fmt`/`clippy` with zero warnings.               |
| **NFR‑07** | No runtime panics on invalid user input or missing external binaries; graceful error messages instead. |

## 6  Data & Config

* **Config file**: `~/.config/sw/config.json` (Serde‑serialized).
* **Cache files**:

  * Projects: `~/.local/cache/sw_projects.cache` (bincode‑encoded Vec<Project>). TTL default = 5 minutes.
  * GitHub repos: `~/.local/cache/sw_github.cache` (same format).

## 7  External Interfaces

| Service / Binary       | Interaction                                            |
| ---------------------- | ------------------------------------------------------ |
| `git` CLI OR `libgit2` | Fetch last commit timestamp & clone repos when needed. |
| `gh` CLI               | GitHub authentication and repository listing via API.  |
| `fzf` binary           | Optional fuzzy picker in `--fzf` mode.                 |

## 8  Open Design Questions

1. Use **`git2`** bindings vs. spawn `git` CLI for commit dates/cloning?
2. Should GitHub queries be cached separately from local projects?
3. How to handle `gh` CLI version compatibility across different installations?

## 9  Success Metrics

* P50 startup time from telemetry logs meets **NFR‑01**.
* ≥ 500 downloads on crates.io in first month.
* CI maintains green master; merge ‑> release via GitHub Actions.

## 10 Out‑of‑Scope

* Multi‑user shared caches.
* Windows support (nice‑to‑have future).
* GUI front‑ends.

---

*Last updated 2025‑05‑26*
