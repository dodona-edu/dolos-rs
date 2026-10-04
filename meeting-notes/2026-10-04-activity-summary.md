# dolos-rs — activity summary

**Repository:** [dodona-edu/dolos-rs](https://github.com/dodona-edu/dolos-rs) · **Window:** 20 Sep – 4 Oct 2026 (2 weeks)

> **Headline:** no human-authored PR was merged to `main` in these two weeks. All substantive work (~20 commits) sits on a 3-deep stacked PR chain that has been waiting on review since 10 September — 24 days.

---

## 1. Merged pull requests

Both merges to `main` were dependency bumps; no feature or fix work landed.

| PR | Title | Author | Merged |
|----|-------|--------|--------|
| [#87](https://github.com/dodona-edu/dolos-rs/pull/87) | Bump `tree-sitter-modelica` (`ee57ebf` → `aa3ea6b`) | dependabot[bot] | 1 Oct |
| [#82](https://github.com/dodona-edu/dolos-rs/pull/82) | Bump cargo-dependencies: `rand` 0.10.2→0.10.3, `clap` 4.6.6→4.6.7 | dependabot[bot] | 28 Sep |

### Closed without merging

| PR | Title | Outcome |
|----|-------|---------|
| [#78](https://github.com/dodona-edu/dolos-rs/pull/78) | refactor: Validate the input of `analyze`, drop the `IgnoredPositions` / `IgnoreMask` wrappers (+1305/−570, 24 files) | Closed 28 Sep. Not lost — the commits were folded into the stacked branches (`refactor/split-dolos-core`, `feature/export-analysis-data`); the branch had gone conflicted against its base. |
| [#84](https://github.com/dodona-edu/dolos-rs/pull/84) | fix: Do the winnowing hash arithmetic in `u64` (draft) | Opened and closed the same day (28 Sep), together with issue #83. |

---

## 2. Closed issues

| Issue | Title | Resolution |
|-------|-------|------------|
| [#83](https://github.com/dodona-edu/dolos-rs/issues/83) | Winnowing hash arithmetic overflows on a 32-bit `usize` | **Closed as not planned** (28 Sep). `RollingHash`/`hash_token` compute in `usize`; intermediate products reach ~2^47 and would overflow silently on `wasm32`. Latent only: today just `dolos-core` compiles to wasm, and the tokenizer stays native. Becomes real the moment tokenization runs on a 32-bit target. |

That is the only issue closed in the window.

---

## 3. Open PRs awaiting review

A single stacked chain — `#75` → `#74` → `#73` → `main`. All three opened **10 Sep**, reviewer **@rien** requested, **waiting 24 days**. CI is green and mergeable on all three; the only reviews on record are the author's own automated code-review passes. Nothing can merge until #73 does.

| PR | Title | Base | Size | Closes | Waiting |
|----|-------|------|------|--------|---------|
| [#73](https://github.com/dodona-edu/dolos-rs/pull/73) | refactor: Split `dolos-lib` into `dolos-core` | `main` | +979/−494, 36 files, 16 commits | #46 | 24 d |
| [#74](https://github.com/dodona-edu/dolos-rs/pull/74) | feat: Export the analysis data in the report | #73 | +619/−276, 13 files | #47 | 24 d |
| [#75](https://github.com/dodona-edu/dolos-rs/pull/75) | feat: Analyze a pair in the browser via WASM | #74 | — | #48 | 24 d |

Roughly 1,600 added lines queued behind one review, and the chain keeps growing (last pushes 1 Oct).

---

## 4. Notable commits, releases, CI

**Commits on `main`:** 2, both dependabot. Everything else landed on the feature branches.

**Work on the stacked branches (14 Sep – 1 Oct), all by Michiel Lachaert:**
- **WASM binding reworked** — `Interval` became an object with `start`/`end`, `options` is now required, errors surface as `io::Error`; the smoke test was replaced by a real TypeScript suite in `dolos-core/tests-typescript` that type-checks against the generated `.d.ts` and runs under `node --test`.
- **Analysis input validation** — `analyze` takes ignored ranges directly (`Option<&VecBitmap>`), the contract moved out of `debug_assert!` into named validation rules, and `InputError` collapsed into `std::io::Error`.
- **Two real defects fixed along the way** — `record_match` split matches on one sequence's ignored positions only, and which side it read came from a hash map, so runs were non-deterministic when the two sides were ignored at different offsets; and a run over fewer than two files panicked in `collect_child_maps` instead of erroring.
- **Perf** — shared ignore-free runs now found in one pass (walk the OR of both windows) instead of intersecting two run lists.
- **CI/tooling** — Node 24 pinned for the TypeScript job, `wasm32-unknown-unknown` lint job added, pre-commit hook now lints the cfg-gated wasm module (a lint error there used to pass the hook and fail CI), package metadata and licences added to all three crates.

**Releases:** none. The repository has never published a release or tag.

**CI:** 38 workflow runs since 20 Sep — 34 success, 3 cancelled (superseded pushes), 1 failure. No `Continuous Integration` failure on `main`. The single failure is the **Dependabot security-update job on `main`** (28 Sep), covering `failure`, `idna` and `serde_yaml` — Dependabot could not produce those security updates. Worth a look: `serde_yaml` is unmaintained and `idna` carries a known advisory.

---

## 5. For discussion

1. **Review is the bottleneck, and it is the only one.** Three PRs, one reviewer, 24 days, ~1,600 lines. Two weeks of output is invisible on `main`. Worth agreeing a review cadence, or landing #73 (a behaviour-preserving split, green and mergeable) on its own so the stack can unwind incrementally.
2. **Stacking is costing rework.** #78's 1,305 lines had to be closed and folded back into the stack after its base moved; branches are rebased repeatedly while they wait. The longer the chain waits, the more of this there is.
3. **A known performance regression is sitting untouched.** [#81](https://github.com/dodona-edu/dolos-rs/issues/81) — writing `pairs.csv` is 17% slower since #68 — open since 17 Sep with no activity. Decide whether it blocks anything.
4. **#83 was closed as not planned.** Fine while the tokenizer is native-only, but it is a prerequisite if tokenization is ever meant to run in the browser — which is the direction #75 points. Worth an explicit decision rather than leaving it closed.
5. **Dependabot security updates are failing.** One failed run on 28 Sep for three crates; nobody has triaged it.
6. **No releases yet.** If anything downstream is meant to consume `dolos-core` (the npm package `wasm-pack` builds, for instance), there is nothing tagged to consume.
