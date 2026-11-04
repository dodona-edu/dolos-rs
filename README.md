# dolos-rs

A Rust implementation of the [Dolos](https://github.com/dodona-edu/dolos) source code plagiarism detection CLI.

> **Note:** This project is intended to eventually replace the existing Node.js CLI in the main Dolos repository.

## Workspace crates

- **[`dolos-cli`](./dolos-cli/)** — the Rust CLI (the `dolos` binary) for running similarity analyses on source code files.
- **[`dolos-lib`](./dolos-lib/)** — the library (`dolos`) implementing tokenization, winnowing, the reader and the report. The per-language feature flags live here.
- **[`dolos-core`](./dolos-core/)** — the pure computational core: the generalized suffix tree, the maximal-exact-match analysis and the pairwise similarity metrics. It has no dependencies, and it also builds for WebAssembly.
- **[`tree-sitter-grammars`](./tree-sitter-grammars/)** — a unified crate that bundles tree-sitter grammar bindings for 29 programming languages behind a single ergonomic API. See its [README](./tree-sitter-grammars/README.md) for details.

## Building

Requires a recent stable Rust toolchain. To build the CLI (which bundles support for all languages):

```sh
cargo build -p dolos-cli
```

The per-language feature flags are defined on `dolos-lib`. To depend on the
library with only specific languages, enable the corresponding features:

```sh
# In your Cargo.toml
dolos-lib = { path = "../dolos-lib", default-features = false, features = ["lang-python", "lang-javascript"] }
```

## Usage

```sh
# Analyze a directory of files
dolos run path/to/files/

# Analyze specific files
dolos run file1.py file2.py file3.py

# Output results as CSV
dolos run --output-format csv --output-destination ./results/ path/to/files/
```

## WebAssembly

`dolos-core` builds for the browser, so a frontend can run the analysis itself.
The binding is behind the opt-in `wasm` feature, so a normal build pulls none of
its dependencies.

The package is published to npm as
[`@dodona/dolos-core`](https://www.npmjs.com/package/@dodona/dolos-core), from
version 2.0.0. Versions 1.x are the earlier TypeScript core.

```sh
npm install @dodona/dolos-core
```

To build the package yourself:

```sh
rustup target add wasm32-unknown-unknown
wasm-pack build dolos-core --target web --scope dodona --release -- --features wasm
```

This writes the npm package `@dodona/dolos-core` to `dolos-core/pkg/`, which
git ignores. A local build takes the version of the crate. The publish
workflow sets the npm version.

The package loads its `.wasm` file in `init()`. Call it once, and await it
before the first call into the package:

```ts
import init, { analyze } from "@dodona/dolos-core";

await init();

// `ignored` holds the positions to leave out, one list of half-open
// `[start, end]` intervals per sequence, the same shape as the ignored
// intervals in the CSV report. Pass `null` to ignore nothing.
using analysis = analyze(fingerprints, ignored, { minMatchLength: 5, keepMatches: true });

const metrics = analysis.metrics(0, 1);
const matches = analysis.matches(0, 1);
```

`Analysis` holds the result in WebAssembly memory and converts one pair per call,
so an analysis over hundreds of files stays cheap. Release it with `free()`, or
declare it with `using` as above.

For exactly two sequences, `analyzePair` returns the metrics and the matches as
plain data, so there is nothing to release:

```ts
import init, { analyzePair } from "@dodona/dolos-core";

await init();

const { metrics, matches } = analyzePair(
  { fingerprints: leftFingerprints, ignored: leftIgnored },
  { fingerprints: rightFingerprints, ignored: rightIgnored },
  { minMatchLength: 5 },
);
```

The package works with any bundler and without one. Vite needs no plugin and no
config change, also in a worker.

## Who made this?

Dolos is an active research project by [Team Dolos](https://dolos.ugent.be/) at Ghent University. If you use this software for your research, please cite:

- Maertens et al. (2024) SoftwareX [doi:10.1016/j.softx.2024.101755](https://doi.org/10.1016/j.softx.2024.101755)

## AI disclosure

Parts of the code and documentation in this repository were written with the help of an AI coding assistant, [Claude Code](https://claude.com/claude-code).
A human maintainer reviews these changes before they are merged.

## License

Licensed under the [MIT license](./LICENSE).
