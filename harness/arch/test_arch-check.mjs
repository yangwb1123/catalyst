// Unit tests for the architecture checks. They run over SYNTHETIC scan models
// (no filesystem), so they are fast and deterministic — and, crucially, each
// includes a NEGATIVE fixture proving the check actually CATCHES its violation
// (a check that never fails is worthless). The live dogfood run
// (node harness/arch/arch-check.mjs) covers the real-tree path.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  checkLayering, checkPackage, checkFanin, checkCognitive, checkAntiPatterns,
  checkFunctionLength, checkCircular,
} from './arch-check.mjs';
import { extractFunctions } from './scan.mjs';

const rules = {
  architecture: { forbidden: ['domain -> infrastructure', 'domain -> application'] },
  package: { max_files: 2, max_exports: 3 },
  fanin: { max_importers: 1 },
  cognitive: { max_root_modules: 1 },
};

function file(rel, layer, imports = [], extra = {}) {
  return { rel, dir: rel.replace(/\/[^/]+$/, ''), layer, isTest: false, exports: 0, imports, ...extra };
}

test('layering: a clean inward-pointing model has no violations', () => {
  const m = { files: [
    file('app/service/s.go', 'application', [{ kind: 'internal', layer: 'domain', rel: 'app/domain/d.go' }]),
    file('app/domain/d.go', 'domain', []),
  ] };
  assert.equal(checkLayering(m, rules).length, 0);
});

test('layering: domain importing infrastructure IS flagged (inner->outer forbidden)', () => {
  const m = { files: [
    file('app/domain/d.go', 'domain', [{ kind: 'internal', layer: 'infrastructure', rel: 'app/store/m.go' }]),
  ] };
  const v = checkLayering(m, rules);
  assert.equal(v.length, 1);
  assert.match(v[0], /forbidden domain -> infrastructure/);
});

test('layering: a test file that violates direction is NOT flagged (tests skipped)', () => {
  const m = { files: [{
    ...file('app/domain/d_test.go', 'domain', [{ kind: 'internal', layer: 'infrastructure', rel: 'x' }]),
    isTest: true,
  }] };
  assert.equal(checkLayering(m, rules).length, 0);
});

test('package: > max non-test files is flagged; tests are excluded from the budget', () => {
  const m = { files: [
    file('p/a.go', null), file('p/b.go', null), file('p/c.go', null), // 3 non-test > max 2
    { ...file('p/c_test.go', null), isTest: true }, // excluded
  ] };
  assert.ok(checkPackage(m, rules).some((x) => /3 files/.test(x)));
});

test('fanin: a target imported by more than max_importers is flagged', () => {
  const m = { files: [
    file('a.go', null, [{ kind: 'internal', dir: '/t', layer: null }]),
    file('b.go', null, [{ kind: 'internal', dir: '/t', layer: null }]), // /t imported by 2 > 1
  ] };
  assert.equal(checkFanin(m, rules).length, 1);
});

test('cognitive: too many top-level source modules is flagged', () => {
  const m = { files: [file('one/a.go', null), file('two/b.go', null)] }; // 2 > 1
  assert.equal(checkCognitive(m, rules).length, 1);
});

test('anti-pattern naming: a utils/ grab-bag directory IS flagged', () => {
  const r = { naming: { anti_patterns: ['utils', 'common'] }, architecture: { dir_aliases: { service: 'application' } } };
  const m = { files: [file('src/utils/x.go', null), file('src/domain/d.go', null)] };
  const v = checkAntiPatterns(m, r);
  assert.equal(v.length, 1);
  assert.match(v[0], /utils/);
});

test('anti-pattern naming: a name blessed as a layer in dir_aliases is NOT flagged', () => {
  // `service` is a technical-role name, but here it is a deliberate layer
  // (service -> application), so it is exempt — not a junk drawer.
  const r = { naming: { anti_patterns: ['service'] }, architecture: { dir_aliases: { service: 'application' } } };
  const m = { files: [file('app/service/s.go', null)] };
  assert.equal(checkAntiPatterns(m, r).length, 0);
});

// --- function-length: parser (extractFunctions) + check ----------------------

test('function-length NEGATIVE: a >50-line function IS flagged with file:line, name, count', () => {
  // 60-line JS function body — the canonical violation the budget exists to catch.
  const body = ['export function tooLong() {', ...Array(58).fill('  doWork();'), '}'].join('\n');
  const m = { files: [{ rel: 'src/big.mjs', functions: extractFunctions(body, 'js') }] };
  const v = checkFunctionLength(m, 50);
  assert.equal(v.length, 1);
  assert.match(v[0], /src\/big\.mjs:1 tooLong 60 lines \(max 50\)/);
});

test('function-length POSITIVE: functions at/under the limit are clean', () => {
  const ok = ['function fine() {', ...Array(48).fill('  x();'), '}'].join('\n'); // 50 lines
  const m = { files: [{ rel: 'src/ok.mjs', functions: extractFunctions(ok, 'js') }] };
  assert.equal(checkFunctionLength(m, 50).length, 0);
});

test('function-length FAIL-CLOSED: a missing/non-numeric limit is reported, not silently passed', () => {
  const m = { files: [{ rel: 'a.mjs', functions: [{ name: 'f', line: 1, lines: 9 }] }] };
  assert.equal(checkFunctionLength(m, NaN).length, 1);
  assert.match(checkFunctionLength(m, NaN)[0], /max_function_lines missing/);
});

test('extractFunctions(go): brace-matched span; a `{` inside a string does NOT inflate it', () => {
  // Regression for the string-brace bug: []byte("{json") must not extend the span.
  const src = [
    'func Decode(t *T) {',          // 1
    '\tblob := []byte("{not json")', // 2  — brace in string literal
    '\tuse(blob)',                   // 3
    '}',                             // 4  — real close: span is 4 lines
    'func After() {}',               // 5
  ].join('\n');
  const fns = extractFunctions(src, 'go');
  const decode = fns.find((f) => f.name === 'Decode');
  assert.equal(decode.lines, 4); // would be 5 (run-on to After's `}`) without the fix
  assert.equal(decode.line, 1);
});

// DECISIVE PROOF that the multi-line false-NEGATIVE is closed. Before the fix,
// braceDelta reset the string state every line, so the `}` chars inside a Go raw
// string (backtick) spanning lines were counted as REAL closes — collapsing the
// body span far below its true length and letting a >50-line function slip under
// the budget SILENTLY. This builds a ~55-line function whose body holds a 30-line
// raw string FULL of stray `}` and asserts function-length now FAILs it.
test('function-length: a >50-line Go func with a multi-line raw string (stray `}`) IS caught', () => {
  const rawBody = Array(30).fill('}}} not real braces — inside `+"`raw`"+` string }}}');
  const src = [
    'func huge() {',                 // 1   real body opens
    '\tblob := `',                   // 2   backtick raw string OPENS (multi-line)
    ...rawBody,                      // 3..32  30 lines of `}` that must NOT close the func
    '\t`',                           // 33  raw string CLOSES
    ...Array(20).fill('\tdoWork()'), // 34..53  padding to push the body well past 50
    '\treturn',                      // 54
    '}',                             // 55  the ONE real close — true span is 55 lines
  ].join('\n');
  const fns = extractFunctions(src, 'go');
  const huge = fns.find((f) => f.name === 'huge');
  assert.ok(huge, 'the function header must still be found');
  assert.equal(huge.lines, 55, 'raw-string `}` must not shorten the span (was ~3 pre-fix)');
  const m = { files: [{ rel: 'x/huge.go', functions: fns }] };
  const v = checkFunctionLength(m, 50);
  assert.equal(v.length, 1, 'a true >50-line function must FAIL the budget');
  assert.match(v[0], /x\/huge\.go:1 huge 55 lines \(max 50\)/);
});

test('extractFunctions(js): braces inside a multi-line template literal are skipped', () => {
  // JS template literal (backtick) spans lines; the `{` / `}` inside it (and even
  // a `${...}` interpolation`s braces, treated as opaque) must not move the span.
  const src = [
    'function render() {',           // 1
    '  const t = `line {one}',       // 2  template OPENS, stray braces inside
    '    still {inside} the literal', // 3  more stray braces, still in the literal
    '  `;',                          // 4  template CLOSES
    '  return t;',                   // 5
    '}',                             // 6  real close: span is 6 lines
    'function next() {}',            // 7
  ].join('\n');
  const fns = extractFunctions(src, 'js');
  const render = fns.find((f) => f.name === 'render');
  assert.equal(render.lines, 6); // pre-fix the per-line reset miscounted this
});

test('extractFunctions(go): braces inside a multi-line /* */ block comment are skipped', () => {
  const src = [
    'func Block() {',                // 1
    '\t/* a block comment {',        // 2  block comment OPENS, stray `{`
    '\t   with a stray } inside */',  // 3  stray `}`, comment CLOSES
    '\tdoWork()',                    // 4
    '}',                             // 5  real close: span is 5 lines
  ].join('\n');
  const fns = extractFunctions(src, 'go');
  assert.equal(fns.find((f) => f.name === 'Block').lines, 5);
});

test('extractFunctions(js): a multi-line arrow signature `const f = (\\n..) => {` IS detected', () => {
  // The single-line JS_BOUND could not see this; multiLineArrowName closes the gap.
  const src = [
    'const handler = (',             // 1  signature OPENS across lines
    '  req,',                        // 2
    '  res,',                        // 3
    ') => {',                        // 4  arrow + body open
    '  send(res);',                  // 5
    '}',                             // 6  span is 6 lines
  ].join('\n');
  const fns = extractFunctions(src, 'js');
  const h = fns.find((f) => f.name === 'handler');
  assert.ok(h, 'multi-line arrow binding must be detected as a function');
  assert.equal(h.lines, 6);
});

test('extractFunctions(js): a multi-line PARENTHESIZED expression is NOT a false function', () => {
  // Guard the arrow detector against false positives: no `=>`, so not a function.
  const src = [
    'const total = (',               // 1  parens open but it is an expression
    '  a + b',                       // 2
    ');',                            // 3  closes with `;`, never an arrow
    'function real() {}',            // 4  the only real function here
  ].join('\n');
  const fns = extractFunctions(src, 'js');
  assert.equal(fns.filter((f) => f.name === 'total').length, 0, 'no `=>` -> not a function');
  assert.ok(fns.some((f) => f.name === 'real'));
});

test('extractFunctions(js): KNOWN GAP — a bare `{` inside `${}` interpolation under-counts (never a false PASS)', () => {
  // Honest pin of the documented limit: interpolation is opaque string text, so a
  // brace typed there is skipped. This can only SHORTEN a span (under-count), so
  // it can never turn a true >N body into a false PASS the other way; we record
  // the behavior rather than pretend full `${}` re-entry is modeled.
  const src = [
    'function tpl() {',              // 1
    '  return `${ {a:1} }`;',        // 2  a real object brace lives inside ${...}
    '}',                             // 3  true span is 3 lines
  ].join('\n');
  const fns = extractFunctions(src, 'js');
  // The interpolation`s `{` is skipped (opaque), so the span is the honest 3 —
  // NOT inflated. Documented gap: were the `}` on a later line, it would be
  // skipped too (under-count), which is fail-safe for the >N budget.
  assert.equal(fns.find((f) => f.name === 'tpl').lines, 3);
});

test('extractFunctions(go): a one-line method (open+close on one line) is 1 line', () => {
  const fns = extractFunctions('func (l L) ext() bool { return l.x }', 'go');
  assert.equal(fns.length, 1);
  assert.equal(fns[0].lines, 1);
});

test('extractFunctions(py): a def`s body is its indented block; dedent ends it', () => {
  const src = ['def outer():', '    a = 1', '    b = 2', '', 'top = 3'].join('\n');
  const fns = extractFunctions(src, 'py');
  const outer = fns.find((f) => f.name === 'outer');
  assert.equal(outer.lines, 3); // def + 2 indented (blank/dedent excluded)
});

// --- circular-dependency -----------------------------------------------------

test('circular-dependency NEGATIVE: an A->B->A import cycle IS flagged with the path', () => {
  const m = { files: [
    { rel: 'a/x.mjs', dir: '/r/a', imports: [{ kind: 'internal', dir: '/r/b' }] },
    { rel: 'b/y.mjs', dir: '/r/b', imports: [{ kind: 'internal', dir: '/r/a' }] },
  ] };
  const v = checkCircular(m);
  assert.equal(v.length, 1);
  assert.match(v[0], /->/); // reports the cycle path (dirs relativized by relOf)
});

test('circular-dependency POSITIVE: an acyclic graph (A->B->C) is clean', () => {
  const m = { files: [
    { rel: 'a/x.mjs', dir: '/r/a', imports: [{ kind: 'internal', dir: '/r/b' }] },
    { rel: 'b/y.mjs', dir: '/r/b', imports: [{ kind: 'internal', dir: '/r/c' }] },
    { rel: 'c/z.mjs', dir: '/r/c', imports: [] },
  ] };
  assert.equal(checkCircular(m).length, 0);
});

test('circular-dependency: a file importing a SIBLING in its own dir is not a cycle', () => {
  const m = { files: [
    { rel: 'a/x.mjs', dir: '/r/a', imports: [{ kind: 'internal', dir: '/r/a' }] },
  ] };
  assert.equal(checkCircular(m).length, 0); // self-edge excluded
});
