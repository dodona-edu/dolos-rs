// Runtime check of the WebAssembly boundary itself. The Rust unit tests cover
// the logic; this covers what only shows up once the module is loaded.
//
// Build first:
//   wasm-pack build dolos-core --target nodejs --out-dir pkg-node --release -- --features wasm

import assert from "node:assert/strict";
import { analyze } from "../pkg-node/dolos_core.js";

const symbols = (text) => [...text].map((c) => c.charCodeAt(0));

// A pair with matches kept.
{
  using analysis = analyze([symbols("ABCDE"), symbols("ABCDE")], null, {
    minMatchLength: 2,
    keepMatches: true,
  });

  assert.equal(analysis.sequenceCount, 2);
  assert.equal(analysis.pairCount, 1);
  assert.equal(analysis.hasMatches, true);

  assert.deepEqual(analysis.metrics(0, 1), {
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
}

// Ignored intervals split a match and leave the position out of the total.
{
  using analysis = analyze(
    [symbols("ABCDE"), symbols("ABCDE")],
    [[[2, 3]], [[2, 3]]],
    { minMatchLength: 1, keepMatches: true },
  );

  assert.equal(analysis.metrics(0, 1).totalLeft, 4);
}

// Options are optional, and matches are absent unless the run keeps them.
{
  using analysis = analyze([symbols("AB"), symbols("AB")], null);

  assert.equal(analysis.hasMatches, false);
  assert.equal(analysis.matches(0, 1), undefined, "absent matches must be undefined, not null");
}

// A rejected input throws a catchable Error instead of trapping the module.
// `validate_input` itself is covered by the Rust tests; what only the boundary can show
// is that the throw is catchable, and that a value above 2^32 does not survive
// the conversion to `usize` on wasm32.
{
  assert.throws(() => analyze([[1], [4294967295]], null), Error, "reserved symbol");
  assert.throws(() => analyze([[1], [2 ** 32]], null), Error, "value above 2^32");
  // `[]` covers no sequence. `null` is how a caller ignores nothing.
  assert.throws(() => analyze([symbols("AB"), symbols("AB")], []), /cover 0 sequences/);

  using analysis = analyze([symbols("AB"), symbols("AB")], null);

  // The module still works after the rejections, so neither of them trapped it.
  assert.equal(analysis.metrics(0, 1).longestMatch, 2);

  // Indices that name no pair throw instead of reaching PairArray.
  assert.throws(() => analysis.metrics(1, 1), /does not form a pair with itself/);
  assert.throws(() => analysis.metrics(0, 5), /out of range/);
}

console.log("smoke: ok");
