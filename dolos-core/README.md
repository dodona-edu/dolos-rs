# dolos-core

The computational core of [Dolos](https://dolos.ugent.be), a source code plagiarism detection tool.

dolos-core compares sequences of symbols pairwise. In Dolos, each sequence holds the fingerprints of one file. The analysis finds the maximal exact matches between every pair of sequences and cuts them at ignored positions. From these matches it derives the similarity metrics of each pair.

The crate has no dependencies. It builds for Rust and, with the `wasm` feature, for WebAssembly.

## JavaScript

The WebAssembly package is published to npm as `@dodona/dolos-core`, from version 2.0.0. Versions 1.x are the earlier TypeScript core of Dolos.

```sh
npm install @dodona/dolos-core
```

The package loads its `.wasm` file in `init()`. Call it once, and await it before the first call into the package. The package works with any bundler and without one.

For exactly two sequences, `analyzePair` returns the metrics and the matches as plain data:

```ts
import init, { analyzePair } from "@dodona/dolos-core";

await init();

// `ignored` holds the positions to leave out, as half-open `[start, end]` intervals.
const { metrics, matches } = analyzePair(
  { fingerprints: [1, 2, 3, 4, 5], ignored: [] },
  { fingerprints: [7, 2, 3, 4, 8], ignored: [] },
  { minMatchLength: 2 },
);

console.log(metrics.similarity); // 0.6
console.log(matches); // [{ leftStart: 1, rightStart: 1, length: 3 }]
```

For many sequences, `analyze` compares all of them pairwise. The result stays in WebAssembly memory and converts one pair per call. Release it with `free()`, or declare it with `using`:

```ts
import init, { analyze } from "@dodona/dolos-core";

await init();

// One list of ignored intervals per sequence, or `null` to ignore nothing.
using analysis = analyze([[1, 2, 3], [1, 2, 4], [9, 2, 3]], null, {
  minMatchLength: 1,
  keepMatches: true,
});

console.log(analysis.metrics(0, 1).longestMatch); // 2
console.log(analysis.matches(0, 2)); // [{ leftStart: 1, rightStart: 1, length: 2 }]
```

The fingerprints and the ignored intervals of a file are in `files.csv` of a Dolos report made with `--include-analysis-data`.

## Rust

```toml
[dependencies]
dolos-core = { git = "https://github.com/dodona-edu/dolos-rs" }
```

```rust
use dolos_core::{AnalysisOptions, analyze};

let sequences = vec![vec![1, 2, 3, 4, 5], vec![7, 2, 3, 4, 8]];
let options = AnalysisOptions { min_match_length: 2, keep_matches: true };
let result = analyze(&sequences, None, &options)?;

assert_eq!(result.metrics()[(0, 1)].similarity, 0.6);
```

## License

Licensed under the [MIT license](./LICENSE).
