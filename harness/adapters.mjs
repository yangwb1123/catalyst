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
