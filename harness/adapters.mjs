// ForgeOS harness adapters — turns the declarative per-language command maps in
// harness/adapters/<lang>.yml into an EXECUTABLE framework.
//
// Until now those adapters were pure declarations: nothing read them, so the
// {lint,test,coverage} commands they ship were never run (a fresh reviewer
// flagged this as "no consumer"). This module is the consumer: it loads an
// adapter and detects which languages a project is written in, so the
// acceptance gate's lint criterion can shell out the right linter per language.
//
// Zero third-party deps (node: builtins only). The YAML the adapters use is the
// same 2-space-indent / `key: value` / bare-`key:` / `- item` shape the arch
// rules use, so we reuse the in-Node parseRules from harness/arch/scan.mjs
// rather than shelling out to PyYAML/yaml2json — keeping the path zero-dep.
//
// Boundary discipline (mirrors scan.mjs): the parsing/classification helpers are
// PURE (take their inputs explicitly, no I/O) so they are unit-testable without
// the filesystem; only loadAdapter and detectLanguages touch disk.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseRules } from './arch/scan.mjs';
import { ADAPTER_LANGS } from './adapters/detection.mjs';
export {
  ADAPTER_LANGS,
  detectLanguages,
  langForExt,
  walkAdapterSources,
} from './adapters/detection.mjs';

const HARNESS_DIR = dirname(fileURLToPath(import.meta.url));
const ADAPTERS_DIR = join(HARNESS_DIR, 'adapters');

// App test-RUNNER kind (acceptance.mjs inferRunner: node/python/go) -> the adapter
// language whose <lang>.yml `test:` command governs it. node's *.test.mjs funnels
// to the typescript adapter (the one LANG_BY_EXT maps .mjs/.ts/.js onto), so the
// app-test selector and detectLanguages agree on the language.
export const ADAPTER_LANG_BY_RUNNER = { node: 'typescript', python: 'python', go: 'go' };

// --- pure helpers ------------------------------------------------------------

// adapterCommands: pull executable run-strings out of a parsed command map.
// adapter object (the shape parseRules returns for a <lang>.yml). Missing
// commands come back undefined rather than throwing — a partial adapter is the
// caller's problem to interpret, not a crash here. Pure over the parsed object.
export function adapterCommands(parsed) {
  const cmds = parsed?.commands ?? {};
  return {
    lint: cmds.lint?.run,
    test: cmds.test?.run,
    typecheck: cmds.typecheck?.run,
    build: cmds.build?.run,
    coverage: cmds.coverage?.run,
  };
}

// lintBinary: the executable name a lint command invokes — its first
// whitespace-delimited token (e.g. "eslint . --max-warnings=0" -> "eslint",
// "golangci-lint run ./..." -> "golangci-lint"). Pure; null when there is no
// lint command. This is what we probe with `<bin> --version` to decide whether
// the linter is INSTALLED before ever running the (heavier) full lint command.
export function lintBinary(lintCmd) {
  if (!lintCmd || typeof lintCmd !== 'string') return null;
  const first = lintCmd.trim().split(/\s+/)[0];
  return first || null;
}

// coverageBinary: the executable a coverage command invokes — its first token
// (e.g. "go test -coverprofile=coverage.out ./..." -> "go",
// "python3 -I -B ... --cov" -> "python3", "vitest run --coverage" -> "vitest").
// Same first-token rule as lintBinary; kept as its own named export
// so the coverage probe reads as a coverage probe (and so a future divergence —
// e.g. a coverage command that is not first-token-addressable — has one place to
// change). Pure; null when there is no coverage command.
export function coverageBinary(coverageCmd) {
  return lintBinary(coverageCmd);
}

// versionProbeArgs: the args that make a tool print its version and exit 0 when
// it is installed — the cheap "is this on PATH" check, used BEFORE running the
// heavier coverage command. Most tools accept `--version`, but `go`'s version
// check is the subcommand `go version` (`go --version` exits 2: "flag provided
// but not defined: -version" — which would dishonestly read as "go not
// installed"). Pure data so the per-tool mapping is unit-testable without
// spawning. Default `['--version']`; `go` -> `['version']`.
export function versionProbeArgs(bin) {
  if (bin === 'go') return ['version'];
  return ['--version'];
}

// coverageArtifact: the report file/dir a coverage command WRITES into the
// working tree, so the probe can avoid polluting the repo (e.g. `go test
// -coverprofile=coverage.out ./...` drops a `coverage.out` even when the run
// fails). Pure; returns a repo-root-relative path, or null when the command
// writes no well-known artifact. Derived from the command string so it tracks
// the adapter YAML, not a hardcoded list:
//   - go:     "-coverprofile=<path>"           -> <path>   (e.g. coverage.out);
//   - pytest: "--cov-report=json"              -> coverage.json (coverage.py default);
//   - vitest: "--coverage"                     -> coverage  (the c8/istanbul dir).
// Conservative: only these explicit, adapter-shipped shapes match; anything else
// -> null (the probe then leaves the tree untouched).
export function coverageArtifact(coverageCmd) {
  const cmd = typeof coverageCmd === 'string' ? coverageCmd : '';
  if (!cmd) return null;
  const profile = cmd.match(/-coverprofile=(\S+)/);
  if (profile) return profile[1];
  const pytestJson = cmd.match(/--cov-report(?:=|\s+)json(?::(\S+))?/);
  if (pytestJson) return pytestJson[1] ?? 'coverage.json';
  if (/--coverage\b/.test(cmd)) return 'coverage';
  return null;
}

export function coverageArtifacts(coverageCmd) {
  const cmd = typeof coverageCmd === 'string' ? coverageCmd : '';
  const artifacts = /(?:^|\s)--cov(?=[=\s]|$)/.test(cmd) ? ['.coverage'] : [];
  const primary = coverageArtifact(coverageCmd);
  if (primary && !artifacts.includes(primary)) artifacts.push(primary);
  return artifacts;
}

// --- app test selection (declaration-driven, with honest fallback) -----------
//
// The adapters ship a `test:` command per language, but the acceptance gate's APP
// tests (examples/<app>/) used to run a HARDCODED runner per detected extension
// (node --test / python -m unittest / go test), ignoring the adapter's declared
// test command entirely (a reviewer flagged the same "no consumer" gap lint and
// coverage already closed). appTestPlan makes the app-test runner DECLARATION-
// DRIVEN: prefer the adapter `test:` command, falling back to the hardcoded runner
// only when that command's runner does not fit the app's on-disk layout — and
// saying WHY (honesty: never silently substitute a different runner, never claim a
// pass the declared command did not produce). PURE (no I/O): acceptance.mjs feeds
// it the adapter command + the test-dir file list and shells the plan it returns.

// runner-binary -> the test-file basename suffixes that runner DISCOVERS, used to
// decide whether an adapter `test:` command fits an app (its tests must match the
// runner's conventions). Mirrors arch/scan.mjs isTestFile but keyed by runner.
//   - go test:   *_test.go;  node: *.test.mjs/.js (node:test's discovery);
//   - vitest:    *.test/.spec .ts/.js — NOTE not .mjs, so a node:test *.test.mjs
//                app does NOT fit vitest;  pytest/unittest: test_*.py / *_test.py.
// A trailing '_' marks a prefix rule (test_), else it is a suffix.
const RUNNER_TEST_SUFFIXES = new Map([
  ['go', ['_test.go']],
  ['node', ['.test.mjs', '.test.js']],
  ['vitest', ['.test.ts', '.test.js', '.spec.ts', '.spec.js']],
  ['pytest', ['_test.py', 'test_']],
]);

// Hardcoded FALLBACK command per runner kind (the pre-adapter behavior), used when
// the adapter command does not fit. cwd null => repo root; go's -C runs from the
// app's own module dir (a nested module: own go.mod, no root go.work — `go test
// ./examples/..` from the non-module repo root fails). node's pattern is resolved
// by acceptance.mjs (it needs the fail-closed `# tests N>0` count), so node carries
// no command here. Returns {cmd, args, cwd} or null for node.
function fallbackCommand(runner, appName) {
  if (runner === 'python') return { cmd: 'python3', args: ['-m', 'unittest', 'discover', '-s', `examples/${appName}/test`], cwd: null };
  if (runner === 'go') return { cmd: 'go', args: ['-C', `examples/${appName}`, 'test', './...'], cwd: null };
  return null; // node -> acceptance.mjs's counting runner
}

// testCmdMatchesLayout: does the adapter `test:` command's runner DISCOVER the
// app's actual test files? Pure over (runner binary, [test file basenames]). True
// when the runner's known suffixes match a file (the command fits); false when the
// runner is unknown OR no file matches (e.g. adapter `vitest run` vs node:test
// *.test.mjs). `go test ./...` discovers by PACKAGE, so a Go app's *_test.go
// presence is the right "this runner fits" signal too.
export function testCmdMatchesLayout(runner, testFiles) {
  const suffixes = RUNNER_TEST_SUFFIXES.get(runner);
  if (!suffixes) return false;
  return (testFiles ?? []).some((f) => {
    const base = String(f).split(/[\\/]/).pop();
    return suffixes.some((s) => (s.endsWith('_') ? base.startsWith(s) : base.endsWith(s)));
  });
}

// appTestPlan: the PURE decision for HOW to run one app's tests. Inputs:
//   adapterTestCmd — the adapter's `test:` run-string (loadAdapter(lang).test);
//   testFiles      — the app's test-dir file basenames (the layout to fit);
//   appName        — examples/<appName> (for cwd + fallback command paths);
//   runner         — the inferRunner kind (node/python/go) for fallback selection.
// Returns {cmd, args, cwd, tag, countCheck}:
//   - adapter path  when the command fits — its split cmd/args; cwd = the app dir
//     for a relative-path command (`go test ./...`), else null; tag "adapter: …".
//   - fallback path otherwise — the hardcoded runner's command; tag "<runner>
//     fallback: <reason>". countCheck is true ONLY for node (acceptance.mjs then
//     enforces the fail-closed `# tests N>0` count; go/python judge on exit code).
// HONESTY: applicability is a real layout check — when the adapter runner can't
// discover the app's tests we fall back and SAY SO, never run-and-pretend.
export function appTestPlan(adapterTestCmd, testFiles, appName, runner) {
  const [acmd, ...aargs] = String(adapterTestCmd ?? '').trim().split(/\s+/).filter(Boolean);
  const reason = !acmd ? 'adapter has no test command' : `adapter test runner '${acmd}' does not fit app layout`;
  const fits = Boolean(acmd) && testCmdMatchesLayout(acmd, testFiles);
  if (fits) {
    const relativeToApp = aargs.some((a) => a.startsWith('./') || a === '.' || a.endsWith('/...'));
    return { cmd: acmd, args: aargs, cwd: relativeToApp ? `examples/${appName}` : null, tag: `adapter: ${[acmd, ...aargs].join(' ')}`, countCheck: false };
  }
  const fb = fallbackCommand(runner, appName);
  if (!fb) return { cmd: null, args: [], cwd: null, tag: `node fallback: ${reason}`, countCheck: true };
  return { ...fb, tag: `${runner} fallback: ${reason}`, countCheck: false };
}

// DEFAULT_COVERAGE_THRESHOLD: the line-coverage floor judgeCoverage compares
// against when no per-mode threshold can be resolved. References .agent/policies/
// modes.yml's balanced coverage_threshold (60). This is the fail-SAFE fallback:
// when project.yml / modes.yml is missing a field or fails to parse,
// resolveCoverageThreshold returns this single honest default rather than
// guessing — so a misconfigured project still gets a sane PASS/FAIL boundary.
export const DEFAULT_COVERAGE_THRESHOLD = 60;

// COVERAGE_THRESHOLD_CAP: the hard ceiling on a resolved threshold. modes.yml
// states production's +20 coverage_delta "封顶 95 / cap 95"; clamp() enforces it
// so e.g. engineering(80) + production(+20) lands at 95, not 100.
export const COVERAGE_THRESHOLD_CAP = 95;

// clamp: bound n into [lo, hi]. Pure.
function clamp(n, lo, hi) {
  return Math.min(hi, Math.max(lo, n));
}

// computeCoverageThreshold: the PURE mode×lifecycle resolution (no I/O), so the
// arithmetic + fail-safe branches are unit-testable without the filesystem.
// Inputs are the two already-parsed yml objects (parseRules shape) + the project's
// chosen mode/lifecycle strings. Resolution:
//   base  = modes.modes[mode].harness.coverage_threshold   (explorer 0 / balanced
//           60 / engineering 80);
//   delta = modes.lifecycle_modifiers[lifecycle].coverage_delta  (idea 0 / growth
//           +10 / production +20);
//   threshold = clamp(base + delta, 0, 95).
// FAIL-SAFE (honesty): if mode/lifecycle is absent, or either looked-up value is
// not a finite number, fall back to DEFAULT_COVERAGE_THRESHOLD — never a guessed
// or partial figure. NOTE modes.yml writes the deltas as `+10`/`+20`; the minimal
// YAML reader keeps a leading-`+` token as the STRING "+10" (its number coercion
// is `-?\d+` only), so we Number()-coerce both operands here — Number("+10")===10,
// Number(0)===0, Number("warn")===NaN -> fallback.
export function computeCoverageThreshold(modes, mode, lifecycle) {
  const base = Number(modes?.modes?.[mode]?.harness?.coverage_threshold);
  const delta = Number(modes?.lifecycle_modifiers?.[lifecycle]?.coverage_delta);
  if (!Number.isFinite(base) || !Number.isFinite(delta)) return DEFAULT_COVERAGE_THRESHOLD;
  return clamp(base + delta, 0, COVERAGE_THRESHOLD_CAP);
}

// resolveCoverageThreshold: the I/O boundary for computeCoverageThreshold. Reads
// <root>/.agent/project.yml for {mode, lifecycle}, then <root>/.agent/policies/
// modes.yml for the base+delta, and returns the resolved line-coverage floor.
// This is the wire from the central knob (mode×lifecycle in project.yml) into the
// acceptance coverage criterion — replacing the hardcoded 60 that ignored both.
// FAIL-SAFE: ANY failure to read/parse either file (missing file, unreadable,
// malformed) -> DEFAULT_COVERAGE_THRESHOLD (60). Same for a missing field, via
// computeCoverageThreshold. So a project without a .agent/ still gets the honest
// default and the gate stays backward-compatible.
export function resolveCoverageThreshold(root) {
  let modes;
  let project;
  try {
    project = parseRules(readFileSync(join(root, '.agent', 'project.yml'), 'utf8'));
    modes = parseRules(readFileSync(join(root, '.agent', 'policies', 'modes.yml'), 'utf8'));
  } catch {
    return DEFAULT_COVERAGE_THRESHOLD;
  }
  return computeCoverageThreshold(modes, project?.mode, project?.lifecycle);
}

// --- enforce strictness: mode×lifecycle resolution (central knob -> warn/block) -
//
// The gap this closes (the coverage pair's sibling on the OTHER harness knob): the
// gate read its warn|block strictness from policies.yml's single GLOBAL `enforce`,
// ignoring the project's mode (explorer warn / engineering block) and the lifecycle
// FLOOR (production's enforce_floor: block, which must override even a loose mode's
// warn — "explorer+production 也必须过全闸门"). These resolve the strictness from
// the same mode×lifecycle knob resolveCoverageThreshold reads, with the identical
// fail-safe shape.

// ENFORCE_LEVELS: enforce values ordered by STRICTNESS (index = how strict). block
// is stricter than warn. enforceRank/stricterEnforce use this so "take the stricter
// of base and floor" is a max over indices — and a production floor of block wins
// over any mode's warn. An unknown token (typo/garbage) ranks -1 (less strict than
// warn) so it can never spuriously WIN a max and silently tighten the gate; the
// fail-safe in computeEnforce rejects such inputs to the policies fallback instead.
export const ENFORCE_LEVELS = ['warn', 'block'];

// enforceRank: strictness index of an enforce token (warn 0 / block 1), or -1 when
// it is not a known level. Pure.
export function enforceRank(level) {
  return ENFORCE_LEVELS.indexOf(level);
}

// stricterEnforce: the stricter of two enforce levels (the one with the higher
// rank). Pure; this is where "production floor=block overrides mode warn" lives —
// stricterEnforce('warn','block') === 'block'. Returns null when NEITHER input is a
// known level (so the caller fails safe rather than inventing a verdict).
export function stricterEnforce(a, b) {
  const ra = enforceRank(a);
  const rb = enforceRank(b);
  if (ra < 0 && rb < 0) return null;
  return rb > ra ? b : a;
}

// computeEnforce: the PURE mode×lifecycle resolution of the gate's warn|block
// strictness (no I/O), so the "take stricter" + fail-safe branches are unit-testable
// without the filesystem. Inputs: the parsed modes.yml object (parseRules shape),
// the project's chosen mode/lifecycle strings, and the policies.yml `enforce` as the
// conservative FALLBACK (gate.mjs owns that default — currently 'block'). Resolution:
//   base  = modes.modes[mode].harness.enforce             (explorer warn / balanced
//           warn / engineering block);
//   floor = modes.lifecycle_modifiers[lifecycle].enforce_floor  (idea/growth warn /
//           production block);  — NOT every lifecycle declares a floor (mvp has none).
//   enforce = stricter(base, floor)  — block wins over warn, so production's block
//             floor overrides a loose mode's warn (the safety override).
// FAIL-SAFE (honesty / conservative-not-looser): when base is absent/unknown we use
// the floor alone; when the floor is absent we use the base alone; when NEITHER is a
// known level we fall back to `fallback` (the policies.yml enforce). The result is
// always validated to a known level — an unknown stricter() outcome can't happen, but
// a non-warn/block fallback (garbage policies.yml) is rejected back to 'block' so the
// gate never silently degrades to a no-op. So a misconfigured project still BLOCKS.
export function computeEnforce(modes, mode, lifecycle, fallback) {
  const base = modes?.modes?.[mode]?.harness?.enforce;
  const floor = modes?.lifecycle_modifiers?.[lifecycle]?.enforce_floor;
  const resolved = stricterEnforce(base, floor);
  // resolved is null only when BOTH base and floor are unknown -> use the fallback;
  // then guarantee a known level (a garbage policies.yml enforce -> conservative block).
  const out = resolved ?? fallback;
  return enforceRank(out) >= 0 ? out : 'block';
}

// resolveEnforce: the I/O boundary for computeEnforce — the wire from the central
// knob (mode×lifecycle in project.yml) into the gate's warn|block strictness,
// replacing the gate's direct read of policies.yml's GLOBAL enforce. Reads
// <root>/.agent/project.yml for {mode, lifecycle} and <root>/.agent/policies/
// modes.yml for the base+floor; `fallback` is the policies.yml enforce gate.mjs
// already parsed (the conservative default when the central knob is unavailable).
// FAIL-SAFE: ANY failure to read/parse either file (missing/unreadable/malformed)
// -> the fallback (so a project without a .agent/ keeps the gate's policies default
// and stays backward-compatible). The honesty/safety guarantees live in
// computeEnforce: production -> block override, and never looser than the fallback's
// intent (block stays block; an unknown fallback hardens to block).
export function resolveEnforce(root, fallback) {
  let modes;
  let project;
  try {
    project = parseRules(readFileSync(join(root, '.agent', 'project.yml'), 'utf8'));
    modes = parseRules(readFileSync(join(root, '.agent', 'policies', 'modes.yml'), 'utf8'));
  } catch {
    return enforceRank(fallback) >= 0 ? fallback : 'block';
  }
  return computeEnforce(modes, project?.mode, project?.lifecycle, fallback);
}

// --- file-size cap: mode×lifecycle resolution (central knob -> max_file_lines) -
//
// The THIRD harness knob's sibling to the coverage + enforce pairs above. The gate
// read its per-file line cap from policies.yml's single GLOBAL max_file_lines,
// ignoring the project's mode (explorer declares `max_file_lines: 800` to tolerate
// prototypes; balanced/engineering 500) AND the lifecycle veto (production must
// tighten a loose mode back down — "explorer+production 也必须过全闸门"). These
// resolve the cap from the same mode×lifecycle knob, with the identical fail-safe.

// computeMaxFileLines: the PURE mode×lifecycle resolution of the per-file line cap
// (no I/O), so the clamp + fail-safe branches are unit-testable. Inputs: the parsed
// modes.yml object (parseRules shape), the project's mode/lifecycle, and the
// policies.yml max_file_lines as the conservative FALLBACK (gate.mjs owns it).
//   base    = modes.modes[mode].harness.max_file_lines  (explorer 800 / balanced
//             500 / engineering 500; cto declares none -> fallback);
//   ceiling = modes.lifecycle_modifiers[lifecycle].max_file_lines  (production 500;
//             other lifecycles declare none).
//   cap = min(base, ceiling) when a ceiling exists, else base.
// TIGHTEN-ONLY (the central-knob invariant): file-size is stricter when SMALLER, so
// a lifecycle ceiling can only LOWER the cap (production clamps explorer's 800 to
// 500), never raise a stricter base. FAIL-SAFE: a missing/non-positive base falls
// back to `fallback` (so cto / an absent field keeps the gate's default); a
// missing/garbage ceiling is simply ignored (base passes through).
export function computeMaxFileLines(modes, mode, lifecycle, fallback) {
  const base = Number(modes?.modes?.[mode]?.harness?.max_file_lines);
  const eff = Number.isFinite(base) && base > 0 ? base : fallback;
  const ceiling = Number(modes?.lifecycle_modifiers?.[lifecycle]?.max_file_lines);
  if (Number.isFinite(ceiling) && ceiling > 0) return Math.min(eff, ceiling);
  return eff;
}

// resolveMaxFileLines: the I/O boundary for computeMaxFileLines — the wire from the
// central knob (mode×lifecycle in project.yml) into the gate's per-file line cap,
// replacing gate.mjs's direct read of policies.yml's GLOBAL max_file_lines. Reads
// project.yml for {mode, lifecycle} and modes.yml for the base+ceiling; `fallback`
// is the already-validated positive integer gate.mjs parsed from policies.yml.
// FAIL-SAFE: ANY failure to read/parse either file (missing/unreadable/malformed)
// -> the fallback, so a project without a .agent/ keeps the policies default and the
// gate stays backward-compatible (this repo's engineering×mvp still resolves to 500).
export function resolveMaxFileLines(root, fallback) {
  let modes;
  let project;
  try {
    project = parseRules(readFileSync(join(root, '.agent', 'project.yml'), 'utf8'));
    modes = parseRules(readFileSync(join(root, '.agent', 'policies', 'modes.yml'), 'utf8'));
  } catch {
    return fallback;
  }
  return computeMaxFileLines(modes, project?.mode, project?.lifecycle, fallback);
}

// coverageUnrunnable: did a coverage command fail because the tool could not
// actually RUN here — no module / no tests / unconfigured — as opposed to
// producing a real (below-threshold or failing-test) coverage result? Such a
// non-result is NOT a verdict on the code, so judgeCoverage maps it to N/A
// rather than FAIL (the coverage analogue of lint's `unconfigured`).
//
// Detected generically from the tools' own can-not-run wording:
//   - go:     "does not contain main module", "no Go files", "no test files",
//             "setup failed", "no required module", "build constraints exclude";
//   - pytest: "no tests ran", "ERROR: file or directory not found",
//             "unrecognized arguments" or "No module named pytest";
//   - vitest/nyc: "no test files found", "could not find config".
// Conservative: only the clear can-not-run signals match — a real coverage run
// that merely fell below the threshold does NOT match here (it stays a FAIL).
export function coverageUnrunnable(out) {
  const t = out ?? '';
  return /does\s+not\s+contain\s+main\s+module|no\s+required\s+module|build\s+constraints\s+exclude|setup\s+failed|no\s+(?:go|test)\s+files|no\s+tests?\s+(?:ran|found|to\s+run)|file\s+or\s+directory\s+not\s+found|no\s+test\s+files\s+found|unrecognized\s+arguments|could\s+not\s+find\s+(?:a\s+)?config|no\s+configuration|no\s+module\s+named\s+['"]?pytest/i.test(t);
}

// parseCoveragePercent extracts an overall line-coverage percentage. It trusts
// only `%`-signed figures: bare table numbers could be counts, so no guess is
// made (null -> judgeCoverage maps can-not-determine to N/A). Common shapes:
//   - go:      "coverage: 73.4% of statements";
//   - pytest:  the coverage.py "TOTAL ... 85%" summary row;
//   - generic: an "All files: 91.2% ..." style total.
// We take the LAST %-figure on the most specific line so a per-package list
// followed by a total reports the total, not the first package.
export function parseCoveragePercent(out) {
  const t = out ?? '';
  if (typeof t !== 'string' || t.length === 0) return null;
  const lines = t.split('\n');
  // A real aggregate outranks package-local figures. Go's `go test ./...`
  // emits one `ok ... coverage:` line per package, never an overall line; only
  // `go tool cover -func=<profile>` emits the authoritative `total:` row.
  const totalLine = lines.reverse().find((line) => /\btotal\b|\boverall\b|all\s+files/i.test(line));
  if (!totalLine) {
    const packageLines = t.split('\n').filter((line) => /\bcoverage:\s*\d+(?:\.\d+)?\s*%/i.test(line));
    if (packageLines.length > 1) return null;
  }
  const scan = totalLine ?? t;
  const matches = [...scan.matchAll(/(\d+(?:\.\d+)?)\s*%/g)];
  if (matches.length === 0) return null;
  const pct = Number(matches[matches.length - 1][1]);
  return Number.isFinite(pct) ? pct : null;
}

// Pure per-language decision: missing/unrunnable -> N/A; invalid report or a
// failing completed run -> FAIL; otherwise compare its aggregate with threshold.
// A missing tool is never a failure and an exited test failure is never a pass.
export function judgeCoverage(lang, bin, installed, r, threshold = DEFAULT_COVERAGE_THRESHOLD) {
  if (!bin) return { lang, status: 'N-A', detail: `${lang}: coverage command/tool is not configured` };
  if (!installed) return { lang, status: 'N-A', detail: `${lang}: ${bin} not installed` };
  if (!r) return { lang, status: 'N-A', detail: `${lang}: ${bin} not run` };
  const machineReport = lang === 'python' || lang === 'typescript';
  const pct = machineReport ? r.coveragePercent : parseCoveragePercent(r.out);
  const validMachinePercent = machineReport && typeof pct === 'number' && Number.isFinite(pct) && pct >= 0 && pct <= 100;
  const withoutGoNoTestPackages = String(r.out ?? '').replace(/no\s+test\s+files/ig, '');
  const mixedGoPackages = lang === 'go' && pct !== null
    && coverageUnrunnable(r.out) && !coverageUnrunnable(withoutGoNoTestPackages);
  if (coverageUnrunnable(r.out) && !mixedGoPackages && !validMachinePercent) {
    return { lang, status: 'N-A', detail: `${lang}: ${bin} installed but could not run here (no module/tests/config) — not a coverage verdict` };
  }
  if (machineReport && !validMachinePercent) {
    return { lang, status: 'FAIL', detail: `${lang}: ${bin} machine coverage report invalid or missing (${r.coverageError ?? 'no valid percentage'})` };
  }
  if (!r.ok) {
    return { lang, status: 'FAIL', detail: `${lang}: ${bin} coverage command failed (exit ${r.code})` };
  }
  if (pct === null) {
    return { lang, status: 'N-A', detail: `${lang}: ${bin} produced no parseable coverage % (exit ${r.code}) — not a verdict` };
  }
  if (pct >= threshold) {
    return { lang, status: 'PASS', detail: `${lang}: ${bin} ${pct}% >= ${threshold}% threshold` };
  }
  return { lang, status: 'FAIL', detail: `${lang}: ${bin} ${pct}% < ${threshold}% threshold` };
}

// --- I/O boundary ------------------------------------------------------------
// loadAdapterDocument: parse one adapter without choosing a Java toolchain.
export function loadAdapterDocument(lang) {
  if (!ADAPTER_LANGS.includes(lang)) {
    throw new Error(`no adapter for language '${lang}' (have: ${ADAPTER_LANGS.join(', ')})`);
  }
  const text = readFileSync(join(ADAPTERS_DIR, `${lang}.yml`), 'utf8');
  return parseRules(text);
}

// loadAdapter: read harness/adapters/<lang>.yml and return its command strings.
// Java callers pass maven|gradle so the selected wrapper remains explicit. An
// omitted profile reads the common top-level command map. Throws on an unknown
// such adapter ships) so a typo surfaces loudly rather than silently yielding an
// all-undefined command map. The file read uses the same minimal YAML reader the
// arch checks use (zero-dep, in-Node).
export function loadAdapter(lang, profile = null) {
  const parsed = loadAdapterDocument(lang);
  const commandMap = profile ? parsed?.toolchains?.[profile] : parsed;
  if (profile && !commandMap) {
    throw new Error(`adapter '${lang}' has no toolchain profile '${profile}'`);
  }
  return adapterCommands(commandMap);
}
