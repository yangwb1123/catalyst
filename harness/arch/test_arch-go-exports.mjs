// Go-specific architecture scanner regressions kept separate from the general
// architecture-check suite so each harness test file stays within its own size
// policy while Node's recursive test discovery still runs every assertion.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { checkFunctionLength } from './arch-check.mjs';
import { countGoExports, extractFunctions } from './scan.mjs';

// --- countGoExports: grouped const/var/type blocks (false-negative fix) -------
// Before the fix countGoExports only matched single-line decls; every exported
// identifier inside a grouped `const (...)` / `var (...)` / `type (...)` block was
// counted as ZERO, so a vocabulary package could carry an unbounded exported enum
// while the god-package budget saw almost nothing (e.g. internal/mode: 23 real
// exports counted as 7). These pin the corrected counting.

test('countGoExports FALSE-NEGATIVE FIX: exported idents in a grouped const block ARE counted', () => {
  const src = [
    'const (',
    '\tGateLint     = "lint"',           // exported
    '\tGateTest     GateName = "test"',  // exported, with a type before `=`
    '\tunexported   = "x"',              // lowercase -> NOT counted
    '\t_            = iota',             // blank ident -> NOT counted
    ')',
    '',
    'const Single = 1',                  // single-line still counts (regression)
  ].join('\n');
  // 2 grouped exports (GateLint, GateTest) + 1 single-line (Single) = 3.
  assert.equal(countGoExports(src), 3);
});

test('countGoExports: a multi-name grouped spec counts each exported name; closing `)` ends the block', () => {
  const src = [
    'var (',
    '\tA, B = 1, 2',                     // two exported names on one spec line
    '\tc    = 3',                        // unexported
    ')',
    'func notCounted() {}',              // lowercase func after the block -> 0
    'func Exported() {}',                // exported func -> 1
  ].join('\n');
  assert.equal(countGoExports(src), 3); // A, B, Exported
});

test('countGoExports: a multi-line CALL value in a block does NOT mis-close it (no under-count)', () => {
  // The call`s own-line `)` previously matched the block-close regex and dropped
  // `Other` — an UNDER-count (the unsafe false-PASS direction for a god-package gate).
  // Nesting-depth tracking keeps the block open until the real close.
  const src = [
    'var (',
    '\tPattern = mk(',                   // value spans lines; opens a paren
    '\t\t"a",',
    '\t)',                               // closes the CALL, not the block
    '\tOther = 2',                       // must still be counted
    ')',
  ].join('\n');
  assert.equal(countGoExports(src), 2); // Pattern, Other
});

test('countGoExports: struct/interface FIELDS inside a type(...) block are NOT counted as exports', () => {
  // The fields live inside `{}` (one nesting level deeper than the block), so they
  // are not package-level exports — without brace tracking they would over-count.
  const src = [
    'type (',
    '\tServer struct {',
    '\t\tHost string',                   // a struct field, NOT a package export
    '\t\tPort int',
    '\t}',
    '\tOther int',
    ')',
  ].join('\n');
  assert.equal(countGoExports(src), 2); // Server, Other
});

test('countGoExports: a `//` comment with UNBALANCED ()/{} does NOT mis-level the block', () => {
  // The comment carries an unclosed `(` AND `{` (net +2 delimiters, no closers). If
  // depth counted them, the block would never return to level 1 — it would LEAK past
  // its `)`, dropping Alpha/Beta AND the later single-line Gamma (the unsafe
  // under-count). Comment-awareness keeps the depth honest. This fixture GUARDS the
  // `//`-break: revert it and the comment's +2 makes the count collapse (0, not 3).
  const src = [
    'const (',
    '\t// note: a lone foo( and an open brace { live only in this comment',
    '\tAlpha = 1',
    '\tBeta  = 2',
    ')',
    'const Gamma = 3',                        // a single-line export AFTER the block
  ].join('\n');
  assert.equal(countGoExports(src), 3); // Alpha, Beta, Gamma — none lost to comment delimiters
});

// --- function-length: Go generic free function (false-negative fix) ----------

test('extractFunctions(go) FALSE-NEGATIVE FIX: a generic free function `func Map[T any]` IS extracted', () => {
  // The header regex required the name be followed by `<` or `(`; a Go generic
  // free function writes `func Map[T any](...)` (name then `[`), so it matched
  // NOTHING and a >50-line generic body bypassed the budget. Go has no `func Name<`
  // form, so the prior `[<(]` was a typo. A generic METHOD must still work too.
  const body = Array(58).fill('\twork()').join('\n');
  const src = [
    'func Map[T any, U any](xs []T, f func(T) U) []U {', // 1
    body,                                                // 2..59
    '\treturn nil',                                      // 60
    '}',                                                 // 61 -> 61-line body
    '',
    'func (s *Stack[T]) Push(x T) { s.xs = append(s.xs, x) }', // generic method, 1 line
  ].join('\n');
  const fns = extractFunctions(src, 'go');
  const mapFn = fns.find((f) => f.name === 'Map');
  assert.ok(mapFn, 'the generic free function MUST now be extracted (was invisible pre-fix)');
  assert.equal(mapFn.lines, 61, 'its true 61-line span is reported');
  assert.ok(fns.find((f) => f.name === 'Push'), 'a generic method is still extracted (regression)');
  // and the budget now FAILs it where before zero functions => silent PASS.
  const v = checkFunctionLength({ files: [{ rel: 'x/gen.go', functions: fns }] }, 50);
  assert.equal(v.length, 1, 'the >50-line generic function now FAILs the budget');
  assert.match(v[0], /x\/gen\.go:1 Map 61 lines \(max 50\)/);
});
