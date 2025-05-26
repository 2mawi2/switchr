### 🔎 Library research (Rust ecosystem)

| Capability                        | Recommended crates                                                                                                              | Rationale                                                            |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| **CLI & argument parsing**        | [`clap`](https://crates.io/crates/clap) v4 (derive API)                                                                         | Mature, actively-maintained, auto-generates help/comp-scripts.       |
| **Interactive TUI**               | [`ratatui`](https://crates.io/crates/ratatui) – community fork of *tui-rs* (7 mo old) ([crates.io][1])                          | Widget toolkit + Crossterm backend; supports scroll, lists, borders. |
| **Fuzzy search**                  | [`skim`](https://github.com/skim-rs/skim) (library mode) ([GitHub][2]) or `fuzzy-matcher` crate for lightweight in-proc ranking | Same algorithm as `fzf`; exposes iterator API; can embed or spawn.   |
| **Filesystem scan**               | `walkdir` (recursive), `ignore` (git-aware); both zero-dep & cross-platform                                                     | Efficient DirEntry streaming with predicate filters.                 |
| **Git operations & commit dates** | [`git2`](https://crates.io/crates/git2) bindings to `libgit2` (updated 3 w ago) ([crates.io][3])                                | Avoids spawning subprocesses; safe API for timestamps & clone.       |
| **GitHub API**                    | [`octocrab`](https://github.com/XAMPPRocky/octocrab) ([GitHub][4])                                                              | Typed REST/GraphQL client; handles auth token from `gh auth status`. |
| **JSON / cache serialisation**    | `serde`, `serde_json` (for config) ([crates.io][5]) and `bincode` (for compact caches)                                          | Idiomatic, zero-copy deserialisation.                                |
| **User dirs**                     | `directories` crate for `~/.config` & `~/.cache` paths ([crates.io][6])                                                         | Correct XDG handling on Linux & macOS.                               |
| **Concurrency**                   | `rayon` (thread-pool) for parallel directory scans                                                                              | CPU-bound; no runtime dependency.                                    |
| **Error handling**                | `anyhow`, `thiserror`                                                                                                           | Consistent UX with `--verbose` backtraces.                           |

---

### 🛠 High-level implementation approach

1. **Crate layout**

   ```
   sw/
   ├── src/
   │   ├── main.rs            # clap entry-point & subcommands
   │   ├── config.rs          # Serde model + load/save
   │   ├── models.rs          # Project struct (name, path, last_modified, source)
   │   ├── cache.rs           # TTL cache via bincode
   │   ├── scanner/
   │   │   ├── local.rs       # walkdir + metadata times
   │   │   ├── cursor.rs      # parse workspaceStorage JSON
   │   │   └── github.rs      # octocrab queries
   │   ├── fuzzy.rs           # thin wrapper around fuzzy-matcher or skim
   │   ├── tui.rs             # ratatui UI; async channel with search results
   │   └── opener.rs          # launch editor, clone GitHub if needed
   └── Cargo.toml
   ```

2. **Command flow**

   * **`sw --interactive`**
     *Spawner thread*: preload cached projects → spawn scanner threads (rayon) if cache stale → send updates over channel → `tui.rs` renders list, applies fuzzy rank on keystroke.
     *On select*: `opener::open_project`, which resolves clone if `source == Remote`.

   * **`sw --list`**
     Quickly dumps formatted table; used by shell completion and fzf.

   * **`sw --fzf`**
     Pipes list → external `fzf` binary; captures selection.

   * **`sw --setup`**
     Runs a guided dialog using `dialoguer` crate, or `interactive-setup` subcommand inside TUI.

3. **Caching strategy**

   | File                | Codec   | TTL   | Contents                        |
   | ------------------- | ------- | ----- | ------------------------------- |
   | `sw_projects.cache` | bincode | 5 min | `Vec<Project>` (local + cursor) |
   | `sw_github.cache`   | bincode | 5 min | `Vec<Project>` (GitHub only)    |

4. **Parallel discovery**

   ```mermaid
   graph TD
     start(Startup) --> cache{Cache valid?}
     cache -- yes --> serve(Serve cached vector)
     cache -- no --> rayon[Scan tasks in rayon pool]
     rayon --> aggregate{merge + sort}
     aggregate --> saveCache(Save .cache) --> serve
   ```

5. **Unit & integration tests**

   * Cargo features `--no-default-features` for offline tests (GitHub mocked).
   * Use `assert_cmd` & `tempfile` for CLI regression tests.
   * Git operations mocked via `git2::Repository::init_bare` in temp dirs.

6. **Distribution**

   * CI cross-compiles macOS (aarch64, x86\_64) & Linux (musl).
   * Homebrew + cargo-install instructions replicate current `install.sh`.

7. **Migration**

   * Users keep existing `~/.config/sw/config.json`; `config.rs` reads identical schema.
   * Provide a *one-time* shim Python script that exec-replaces itself with the Rust binary if present.

---

### 📄 Product Requirements Document

I’ve drafted a complete PRD capturing every feature and non-functional guarantee of the current Python version.
Feel free to open it in the side-panel and comment or request changes!

* [crates.io](https://crates.io/crates/ratatui/?utm_source=chatgpt.com)
* [GitHub](https://github.com/skim-rs/skim?utm_source=chatgpt.com)
* [GitHub](https://github.com/XAMPPRocky/octocrab?utm_source=chatgpt.com)

[1]: https://crates.io/crates/ratatui/?utm_source=chatgpt.com "ratatui - crates.io: Rust Package Registry"
[2]: https://github.com/skim-rs/skim?utm_source=chatgpt.com "skim-rs/skim: Fuzzy Finder in rust! - GitHub"
[3]: https://crates.io/crates/git2?utm_source=chatgpt.com "git2 - crates.io: Rust Package Registry"
[4]: https://github.com/XAMPPRocky/octocrab?utm_source=chatgpt.com "XAMPPRocky/octocrab: A modern, extensible GitHub API Client for ..."
[5]: https://crates.io/crates/serde_json?utm_source=chatgpt.com "serde_json - crates.io: Rust Package Registry"
[6]: https://crates.io/crates/directories?utm_source=chatgpt.com "directories - crates.io: Rust Package Registry"



Ensure to create a justfile (concise and as short as possible) that can be used to build the project. and run the tests.