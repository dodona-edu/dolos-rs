# dolos-rs — activity summary

**Window:** 2026-09-27 → 2026-10-11 (2 weeks) · **Repo:** `dodona-edu/dolos-rs`

**One-line read:** quiet two weeks — 1 substantive merge, 2 dependency bumps, 1 perf issue closed.
The real story is the **three-PR stacked chain (#73 → #74 → #75) that has been waiting on review for 31 days**.

---

## 1. Merged pull requests (3)

| PR | Title | Author | Merged |
|----|-------|--------|--------|
| [#88](https://github.com/dodona-edu/dolos-rs/pull/88) | perf: Write the CSV tables with serde | milachae | Oct 7 |
| [#87](https://github.com/dodona-edu/dolos-rs/pull/87) | Bump tree-sitter-modelica (`ee57ebf` → `aa3ea6b`) | dependabot | Oct 1 |
| [#82](https://github.com/dodona-edu/dolos-rs/pull/82) | Bump cargo-dependencies group (rand 0.10.2→0.10.3, clap 4.6.6→4.6.7) | dependabot | Sep 28 |

Only #88 is real engineering work: the CSV view now writes through `csv::Writer::serialize` with one row
struct per file. On `bellen-en-blokken-2023-2024` (10 304 files, 53 M pairs) a run went **45.5 s → 36.6 s**.
Follow-up commit in the same PR fixes the header being dropped for empty tables (`fragments.csv`, `pairs.csv`).

**Closed without merging:**
[#84](https://github.com/dodona-edu/dolos-rs/pull/84) *fix: Do the winnowing hash arithmetic in u64* —
opened and closed within 5 minutes on Sep 28, still a draft.

---

## 2. Closed issues (2)

| Issue | Title | Resolution |
|-------|-------|------------|
| [#81](https://github.com/dodona-edu/dolos-rs/issues/81) | Writing `pairs.csv` is 17% slower since #68 | ✅ **Completed** — fixed by #88 |
| [#83](https://github.com/dodona-edu/dolos-rs/issues/83) | Winnowing hash arithmetic overflows on a 32-bit usize | ⚠️ **Closed as not planned** — PR #84 closed with it |

- **#81** was a thorough regression hunt (perf-stat instruction counts, symbol-level profiles, four measured
  fix variants). Root cause: #68 replaced serde serialization with `Column<T> { value: fn(&T) -> String }`,
  turning 4 heap allocations per row into 10 and dropping the `itoa`/`ryu` fast paths — multiplied by 53 M rows.
- **#83** is a latent `usize` overflow in `RollingHash`/`hash_token`; harmless today because only `dolos-core`
  compiles to wasm, but it bites the moment the tokenizer runs on a 32-bit target. **Deliberately deferred** —
  worth confirming that was the intent, since a working fix (14 lines, 1 file) was already written.

**New issues opened in the window (both still open):**
- [#89](https://github.com/dodona-edu/dolos-rs/issues/89) Print CLI errors without the Debug format (bug, Oct 7)
- [#86](https://github.com/dodona-edu/dolos-rs/issues/86) Filter input files by language (bug, Sep 30)

---

## 3. Open PRs awaiting review

| PR | Title | Base | Waiting | Reviewer |
|----|-------|------|---------|----------|
| [#73](https://github.com/dodona-edu/dolos-rs/pull/73) | refactor: Split dolos-lib into dolos-core | `main` | **31 days** (since Sep 10) | rien |
| [#74](https://github.com/dodona-edu/dolos-rs/pull/74) | feat: Export the analysis data in the report | `#73` | **31 days** | rien |
| [#75](https://github.com/dodona-edu/dolos-rs/pull/75) | feat: Analyze a pair in the browser via WASM | `#74` | **31 days** | rien |
| [#90](https://github.com/dodona-edu/dolos-rs/pull/90) | Bump cargo-dependencies group with 3 updates (tokio, tree-sitter-haskell, tree-sitter-swift) | `main` | 4 days | — (dependabot, CI green) |

**#73/#74/#75 are a stack**: each targets the previous one, so nothing lands until #73 is reviewed.
All three are green on CI and were still being rebased/polished on Oct 7, so they are review-ready, not stale.
The only review on #73 is a self-review ("🤖 Code review of the split", Sep 10) — no external review yet.
These three PRs correspond to open issues #46, #47 and #45/#48.

---

## 4. Commits, releases, CI

- **Commits on `main`:** 3 in two weeks (= the 3 merged PRs). Head is `2bf6432`.
- **Releases:** none — the repo has never cut a release. Still "Dolos CLI in Rust 🚧".
- **CI (`Continuous Integration`, ci.yml):** green on every run in the window, including `main` pushes and all
  three stacked PRs. The `cancelled` runs on Oct 1 / Oct 7 are concurrency cancellations from rapid re-pushes,
  not failures.
- **One failure:** the Dependabot updates job `cargo in /. for failure, failure, idna, serde_yaml`
  ([run 37590570813](https://github.com/dodona-edu/dolos-rs/actions/runs/37590570813), Oct 7) failed.
  That is Dependabot's own resolver on security advisories for `failure`, `idna`, `serde_yaml` — these are
  unmaintained/advisory-flagged transitive crates Dependabot can't bump automatically. Not the project's CI,
  but it means those advisories are **silently not being handled**.
- Copilot code review ran on #88 and passed.

---

## 5. Patterns / things worth raising in the promotor meeting

1. **The review bottleneck is the headline.** Three weeks of work (the dolos-core split, the report data export,
   the WASM pair analysis) has been queued on one reviewer for a month. Ask for a review slot, or agree to
   unstack #73 and merge it on its own so #74/#75 stop being hostage to it.
2. **Work has shifted to perf archaeology.** #81 and #88 are both regression recovery against the output-writer
   rewrite in #68 — real value (8.9 s on a 10 k-file run), but it is undoing a recent refactor rather than
   moving the WASM/browser goal forward. The remaining ~1.2 s from #61/#63 is still unrecovered.
3. **A known memory regression is unticketed.** #81's closing notes flag peak RSS doubling from 3.35–3.77 GB
   (April benchmark) to **7.49 GB**, traced to `a5f1b32` changing `Report` to an owned `Vec<Pair>`
   (~4.7 GB of `Pair` structs + 106 M `Rc` clone/drop at 53 M pairs). Unmeasured and no issue filed — that
   likely matters more than the remaining CSV milliseconds.
4. **32-bit correctness is parked while the roadmap goes to WASM.** #83 was closed not-planned, but #75 is
   exactly the browser/wasm32 work. Worth a decision: fix now with #84, or write down the condition under
   which it must be fixed.
5. **Dependabot advisory failures need a human.** `failure`, `idna`, `serde_yaml` can't be auto-bumped;
   nobody is currently assigned to resolve them.
6. **Claude-authored artifacts are landing with an explicit "not reviewed by a human yet" banner**
   (#81, #84). Fine as a workflow, but worth an explicit norm before one of these is cited as a finding.
