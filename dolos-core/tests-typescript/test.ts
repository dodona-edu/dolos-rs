// Check of the WebAssembly boundary itself. The Rust unit tests cover the
// logic; this covers what only shows up once the module is loaded.
//
// Build first:
//   wasm-pack build dolos-core --target nodejs --out-dir pkg-node --release -- --features wasm

import assert from "node:assert/strict";
import { test } from "node:test";
import {
  analyze,
  type AnalysisOptions,
  type Interval,
  type Match,
  type PairMetrics,
} from "../pkg-node/dolos_core.js";

// One symbol per code unit of `text`.
const symbols = (text: string): number[] => [...text].map((c) => c.charCodeAt(0));

const kept: AnalysisOptions = { minMatchLength: 1, keepMatches: true };
const dropped: AnalysisOptions = { minMatchLength: 1, keepMatches: false };

test("a pair reports its counts, metrics and matches", () => {
  using analysis = analyze([symbols("ABCDE"), symbols("ABCDE")], null, {
    minMatchLength: 2,
    keepMatches: true,
  });

  assert.equal(analysis.sequenceCount, 2);
  assert.equal(analysis.pairCount, 1);
  assert.equal(analysis.hasMatches, true);

  const metrics: PairMetrics = analysis.metrics(0, 1);
  assert.deepEqual(metrics, {
    similarity: 1,
    totalLeft: 5,
    totalRight: 5,
    overlapLeft: 5,
    overlapRight: 5,
    longestMatch: 5,
  });
  assert.deepEqual(analysis.matches(0, 1), [
    { leftStart: 0, rightStart: 0, length: 5 },
  ]);
});

test("an ignored interval splits a match and leaves its position out of the total", () => {
  const ignored: Interval[][] = [[{ start: 2, end: 3 }], [{ start: 2, end: 3 }]];

  using analysis = analyze([symbols("ABCDE"), symbols("ABCDE")], ignored, kept);

  assert.deepEqual(analysis.matches(0, 1), [
    { leftStart: 0, rightStart: 0, length: 2 },
    { leftStart: 3, rightStart: 3, length: 2 },
  ]);
  assert.equal(analysis.metrics(0, 1).totalLeft, 4);
});

test("matches are absent, not null, unless the run keeps them", () => {
  using analysis = analyze([symbols("AB"), symbols("AB")], null, dropped);

  assert.equal(analysis.hasMatches, false);
  assert.equal(analysis.matches(0, 1), undefined);
});

// `validate_input` itself is covered by the Rust tests. What only the boundary
// can show is that the throw is catchable, and that a value above 2^32 does not
// survive the conversion to `usize` on wasm32.
test("unusable input throws a catchable Error instead of trapping the module", () => {
  assert.throws(() => analyze([[1], [4294967295]], null, kept), /reserved end-of-sequence symbol/);
  assert.throws(() => analyze([[1], [2 ** 32]], null, kept), /expected usize/);
  // `[]` covers no sequence. `null` is how a caller ignores nothing.
  assert.throws(() => analyze([symbols("AB"), symbols("AB")], [], kept), /cover 0 sequences/);

  // The module still works after the rejections, so none of them trapped it.
  using analysis = analyze([symbols("AB"), symbols("AB")], null, kept);
  assert.equal(analysis.metrics(0, 1).longestMatch, 2);
});

test("indices that name no pair throw instead of reaching PairArray", () => {
  using analysis = analyze([symbols("AB"), symbols("AB")], null, kept);

  assert.throws(() => analysis.metrics(1, 1), /does not form a pair with itself/);
  assert.throws(() => analysis.metrics(0, 2), /out of range/);
});
