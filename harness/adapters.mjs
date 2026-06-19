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
import { dirname, join, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseRules, walkSource } from './arch/scan.mjs';

const HARNESS_DIR = dirname(fileURLToPath(import.meta.url));
const ADAPTERS_DIR = join(HARNESS_DIR, 'adapters');

// Source extension -> the ADAPTER language whose <lang>.yml governs it. Note
// this maps onto the adapter file names (go/python/typescript), so every JS/TS
// flavour funnels to the single typescript.yml adapter (its eslint command lints
// .js too), matching the adapters/README "polyglot repo can hit multiple".
const LANG_BY_EXT = new Map([
  ['.go', 'go'],
  ['.mjs', 'typescript'], ['.cjs', 'typescript'], ['.js', 'typescript'],
  ['.jsx', 'typescript'], ['.ts', 'typescript'], ['.tsx', 'typescript'],
  ['.py', 'python'],
]);

// The adapter languages we ship a <lang>.yml for. Used to validate loadAdapter
// input and to keep detectLanguages from inventing names with no adapter.
export const ADAPTER_LANGS = ['go', 'python', 'typescript'];

// --- pure helpers ------------------------------------------------------------

// langForExt: extension (with dot) -> adapter language, or null when unmapped.
// Pure; exported for direct unit testing of the extension mapping.
export function langForExt(ext) {
  return LANG_BY_EXT.get(ext) ?? null;
}

// adapterCommands: pull the {lint,test,coverage} run-strings out of a parsed
// adapter object (the shape parseRules returns for a <lang>.yml). Missing
// commands come back undefined rather than throwing — a partial adapter is the
// caller's problem to interpret, not a crash here. Pure over the parsed object.
export function adapterCommands(parsed) {
  const cmds = parsed?.commands ?? {};
  return {
    lint: cmds.lint?.run,
    test: cmds.test?.run,
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
// "pytest --cov --cov-report=json" -> "pytest", "vitest run --coverage" ->
// "vitest"). Same first-token rule as lintBinary; kept as its own named export
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
  if (/--cov-report[=\s]+json/.test(cmd)) return 'coverage.json';
  if (/--coverage\b/.test(cmd)) return 'coverage';
  return null;
}

// DEFAULT_COVERAGE_THRESHOLD: the line-coverage floor judgeCoverage compares
// against when no per-mode threshold is supplied. References .agent/policies/
// modes.yml's balanced coverage_threshold (60). Per-mode wiring (explorer 0 /
// engineering 80 / production +20) is a follow-up; until then this is the single
// honest default so the framework can render PASS/FAIL once a tool actually runs.
export const DEFAULT_COVERAGE_THRESHOLD = 60;

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
//             "unrecognized arguments" (the --cov plugin absent);
//   - vitest/nyc: "no test files found", "could not find config".
// Conservative: only the clear can-not-run signals match — a real coverage run
// that merely fell below the threshold does NOT match here (it stays a FAIL).
export function coverageUnrunnable(out) {
  const t = out ?? '';
  return /does\s+not\s+contain\s+main\s+module|no\s+required\s+module|build\s+constraints\s+exclude|setup\s+failed|no\s+(?:go|test)\s+files|no\s+tests?\s+(?:ran|found|to\s+run)|file\s+or\s+directory\s+not\s+found|no\s+test\s+files\s+found|unrecognized\s+arguments|could\s+not\s+find\s+(?:a\s+)?config|no\s+configuration/i.test(t);
}

// parseCoveragePercent: best-effort extraction of an overall line-coverage
// percentage from a coverage tool's stdout. Pure; returns a number 0..100 or
// null when no percentage is present. HONESTY: only figures carrying an explicit
// `%` SIGN are trusted — a bare table number (e.g. istanbul's "All files | 91.2
// | 80 | ...") could be a line/branch COUNT, not a percentage, so it is NOT
// parsed (-> null -> judgeCoverage treats it as can-not-determine -> N/A, never a
// guess). Recognizes the common %-signed shapes:
//   - go:      "coverage: 73.4% of statements";
//   - pytest:  the coverage.py "TOTAL ... 85%" summary row;
//   - generic: an "All files: 91.2% ..." style total.
// We take the LAST %-figure on the most specific line so a per-package list
// followed by a total reports the total, not the first package.
export function parseCoveragePercent(out) {
  const t = out ?? '';
  if (typeof t !== 'string' || t.length === 0) return null;
  // Prefer an explicit total/overall line if one exists.
  const totalLine = t.split('\n').reverse().find((l) => /total|all\s+files|^ok\b|coverage:/i.test(l));
  const scan = totalLine ?? t;
  const matches = [...scan.matchAll(/(\d+(?:\.\d+)?)\s*%/g)];
  if (matches.length === 0) return null;
  const pct = Number(matches[matches.length - 1][1]);
  return Number.isFinite(pct) ? pct : null;
}

// judgeCoverage: the PURE per-language coverage decision (no I/O), mirroring
// judgeLint so the honesty / fail-safe branches are unit-testable with NO
// coverage tool installed. Inputs:
//   lang      — adapter language tag (for the detail string);
//   bin       — the coverage binary, or null when the adapter has no coverage cmd;
//   installed — did `<bin> --version` exit 0 (boolean);
//   r         — the coverage command's run result {ok,code,out}, or null if not run;
//   threshold — line-coverage floor to compare against (default 60).
// Order (fail-SAFE): no coverage cmd / tool not installed -> N/A; tool installed
// but could-not-run (coverageUnrunnable) -> N/A; ran but no parseable % ->
// N/A; % >= threshold -> PASS; % < threshold -> FAIL. A MISSING tool is NEVER a
// FAIL and a tool that could not run is NEVER a faked PASS.
export function judgeCoverage(lang, bin, installed, r, threshold = DEFAULT_COVERAGE_THRESHOLD) {
  if (!bin) return { lang, status: 'N-A', detail: `${lang}: adapter has no coverage command` };
  if (!installed) return { lang, status: 'N-A', detail: `${lang}: ${bin} not installed` };
  if (!r) return { lang, status: 'N-A', detail: `${lang}: ${bin} not run` };
  if (coverageUnrunnable(r.out)) {
    return { lang, status: 'N-A', detail: `${lang}: ${bin} installed but could not run here (no module/tests/config) — not a coverage verdict` };
  }
  const pct = parseCoveragePercent(r.out);
  if (pct === null) {
    return { lang, status: 'N-A', detail: `${lang}: ${bin} produced no parseable coverage % (exit ${r.code}) — not a verdict` };
  }
  if (pct >= threshold) {
    return { lang, status: 'PASS', detail: `${lang}: ${bin} ${pct}% >= ${threshold}% threshold` };
  }
  return { lang, status: 'FAIL', detail: `${lang}: ${bin} ${pct}% < ${threshold}% threshold` };
}

// --- I/O boundary ------------------------------------------------------------

// loadAdapter: read harness/adapters/<lang>.yml and return its
// { lint, test, coverage } command strings. Throws on an unknown language (no
// such adapter ships) so a typo surfaces loudly rather than silently yielding an
// all-undefined command map. The file read uses the same minimal YAML reader the
// arch checks use (zero-dep, in-Node).
export function loadAdapter(lang) {
  if (!ADAPTER_LANGS.includes(lang)) {
    throw new Error(`no adapter for language '${lang}' (have: ${ADAPTER_LANGS.join(', ')})`);
  }
  const text = readFileSync(join(ADAPTERS_DIR, `${lang}.yml`), 'utf8');
  return adapterCommands(parseRules(text));
}

// detectLanguages: scan the project tree under `root` and return the SORTED,
// de-duplicated set of adapter languages present, inferred from source-file
// extensions (.go -> go, .mjs/.ts/.js... -> typescript, .py -> python). Reuses
// walkSource from scan.mjs, so it skips node_modules/.git/vendor/etc. the same
// way the arch scan does. Empty array when the tree has no recognized sources.
export function detectLanguages(root) {
  const langs = new Set();
  for (const file of walkSource(root)) {
    const lang = langForExt(extname(file));
    if (lang) langs.add(lang);
  }
  return [...langs].sort();
}
