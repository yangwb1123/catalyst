// Unit tests for the architecture checks. They run over SYNTHETIC scan models
// (no filesystem), so they are fast and deterministic — and, crucially, each
// includes a NEGATIVE fixture proving the check actually CATCHES its violation
// (a check that never fails is worthless). The live dogfood run
// (node harness/arch/arch-check.mjs) covers the real-tree path.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  checkLayering, checkPackage, checkFanin, checkCognitive, checkAntiPatterns,
  checkFunctionLength, checkCircular,
} from './arch-check.mjs';
import {
  extractFunctions, extractImports, scan, walkSource,
} from './scan.mjs';

const rules = {
  architecture: {
    forbidden: [
      'domain -> infrastructure',
      'domain -> application',
      'application -> infrastructure',
      'infrastructure -> interfaces',
    ],
  },
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

test('layering: application importing infrastructure IS flagged', () => {
  const m = { files: [
    file('app/application/a.go', 'application', [
      { kind: 'internal', layer: 'infrastructure', rel: 'app/infrastructure/db.go' },
    ]),
  ] };
  const v = checkLayering(m, rules);
  assert.equal(v.length, 1);
  assert.match(v[0], /forbidden application -> infrastructure/);
});

test('layering: infrastructure importing an interface IS flagged', () => {
  const m = { files: [
    file('app/infrastructure/db.go', 'infrastructure', [
      { kind: 'internal', layer: 'interfaces', rel: 'app/interfaces/http.go' },
    ]),
  ] };
  const v = checkLayering(m, rules);
  assert.equal(v.length, 1);
  assert.match(v[0], /forbidden infrastructure -> interfaces/);
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

test('fanin: test-file importers are excluded (coupling is a production concern)', () => {
  const m = { files: [
    file('a.go', null, [{ kind: 'internal', dir: '/t', layer: null }]),
    { ...file('a_test.go', null, [{ kind: 'internal', dir: '/t', layer: null }]), isTest: true },
  ] };
  // /t has 1 production importer (a.go); the test importer is not counted, so max 1 holds.
  assert.equal(checkFanin(m, rules).length, 0);
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

test('architecture walk skips Rust target output but keeps sibling source', () => {
  const root = mkdtempSync(join(tmpdir(), 'forge-arch-target-'));
  try {
    mkdirSync(join(root, 'target', 'debug'), { recursive: true });
    mkdirSync(join(root, 'src'));
    writeFileSync(join(root, 'target', 'debug', 'generated.rs'), 'fn generated() {}\n');
    writeFileSync(join(root, 'src', 'main.js'), 'export const live = true;\n');
    assert.deepEqual(walkSource(root), [join(root, 'src', 'main.js')]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('Rust source and Cargo crate imports participate in layering checks', () => {
  const root = mkdtempSync(join(tmpdir(), 'forge-arch-rust-'));
  try {
    const app = join(root, 'crates', 'application');
    const infra = join(root, 'crates', 'infrastructure');
    mkdirSync(join(app, 'src'), { recursive: true });
    mkdirSync(join(infra, 'src'), { recursive: true });
    writeFileSync(join(app, 'Cargo.toml'), '[package]\nname = "sample-application"\n');
    writeFileSync(join(infra, 'Cargo.toml'), '[package]\nname = "sample-infrastructure"\n');
    writeFileSync(join(app, 'src', 'lib.rs'), 'use sample_infrastructure::Store;\npub fn run() {}\n');
    writeFileSync(join(infra, 'src', 'lib.rs'), 'pub struct Store;\n');
    const rustRules = {
      ...rules,
      architecture: {
        ...rules.architecture,
        dir_aliases: { application: 'application', infrastructure: 'infrastructure' },
      },
    };
    const model = scan(root, rustRules);
    assert.equal(model.files.filter((entry) => entry.lang === 'rust').length, 2);
    assert.match(checkLayering(model, rustRules)[0], /forbidden application -> infrastructure/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
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

// --- import extraction: re-export + dynamic forms (false-negative fixes) -----
// These pin the gap where barrel/index re-exports and dynamic import() were
// INVISIBLE to extractImports — so a layering violation or import cycle routed
// THROUGH a re-export silently bypassed checkLayering/checkCircular. Each fixture
// asserts the spec is now RETURNED (it was absent / [] before the fix).

test('extractImports(js) FALSE-NEGATIVE FIX: `export ... from` re-exports ARE captured', () => {
  // `export { a } from`, `export * from`, and `export * as ns from` were all
  // invisible before; a re-export is a real import-graph edge and must be seen.
  const src = [
    'export { a, b } from "./named.mjs";',
    'export * from "./star.mjs";',
    'export * as ns from "./starns.mjs";',
    'export const local = 1;',          // NOT an edge (no `from`)
    'export default function f() {}',    // NOT an edge (no `from`)
  ].join('\n');
  const got = extractImports(src, 'js');
  assert.deepEqual(
    got.sort(),
    ['./named.mjs', './star.mjs', './starns.mjs'],
    'all three re-export forms captured; bare `export` declarations add no edge',
  );
});

test('extractImports(js) FALSE-NEGATIVE FIX: dynamic `import(...)` IS captured', () => {
  // `await import('x')` and `import('x').then(...)` are real (lazy) edges; before
  // the fix only static `import x from` was seen, so dynamic imports were missed.
  const src = [
    'const m = await import("./lazy.mjs");',
    'import("./then.mjs").then((x) => x.run());',
    'import staticDefault from "./static.mjs";', // static still works (regression)
  ].join('\n');
  const got = extractImports(src, 'js');
  assert.deepEqual(
    got.sort(),
    ['./lazy.mjs', './static.mjs', './then.mjs'],
    'both dynamic import() calls captured alongside the static import',
  );
});

test('checkLayering: a forbidden edge routed THROUGH `export ... from` IS now flagged', () => {
  // The end-to-end consequence of the fix: a domain barrel re-exporting from
  // infrastructure is a forbidden inner->outer dependency. Before extractImports
  // saw `export-from`, this edge never reached the model and the violation
  // slipped through as a false PASS. Build the import record the scan now yields.
  const m = { files: [
    file('app/domain/index.mjs', 'domain', [
      { kind: 'internal', layer: 'infrastructure', rel: 'app/store/db.mjs', spec: './store/db.mjs' },
    ]),
  ] };
  const v = checkLayering(m, rules);
  assert.equal(v.length, 1, 're-exported forbidden edge must FAIL layering');
  assert.match(v[0], /forbidden domain -> infrastructure/);
});

// --- function-length: Python async def (false-negative fix) ------------------

test('extractFunctions(py) FALSE-NEGATIVE FIX: an `async def` body IS counted (was invisible)', () => {
  // FastAPI/asyncio coroutines use `async def`; the old `^(\s*)def` regex never
  // matched them, so a 60-line async coroutine yielded ZERO functions and could
  // never trip the 50-line budget. A sibling plain `def` must still be counted
  // (regression). This is the decisive pin: pre-fix `big` was simply absent.
  const src = [
    'async def big():',                  // 1
    ...Array(58).fill('    work()'),     // 2..59
    '    return 1',                      // 60  -> a true 60-line body
    '',                                  // 61  blank gap
    'def small():',                      // 62
    '    return 2',                      // 63  -> 2 lines (regression guard)
  ].join('\n');
  const fns = extractFunctions(src, 'py');
  const big = fns.find((f) => f.name === 'big');
  const small = fns.find((f) => f.name === 'small');
  assert.ok(big, 'the async def MUST now be extracted (was invisible pre-fix)');
  assert.equal(big.lines, 60, 'its true 60-line span is reported');
  assert.equal(big.line, 1);
  assert.ok(small && small.lines === 2, 'a plain `def` is still counted correctly');
  // and the budget now FAILs it where before zero functions => silent PASS.
  const v = checkFunctionLength({ files: [{ rel: 'svc/api.py', functions: fns }] }, 50);
  assert.equal(v.length, 1, 'the >50-line async coroutine now FAILs the budget');
  assert.match(v[0], /svc\/api\.py:1 big 60 lines \(max 50\)/);
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
