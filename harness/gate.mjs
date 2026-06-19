#!/usr/bin/env node
// ForgeOS harness gate (v0) — host-independent constraint enforcement.
// Checks: per-file line cap (code files) + root file count.
// Function-length & circular-deps are language-specific -> later adapters (see ROADMAP).
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, extname, relative } from 'node:path';
import { pathToFileURL } from 'node:url';
// resolveEnforce wires the gate's warn|block strictness to the CENTRAL KNOB
// (mode×lifecycle in .agent/project.yml × modes.yml) instead of policies.yml's
// single global `enforce`. Same I/O-boundary + fail-safe shape as the coverage
// pair next to it; zero-dep (it reuses scan.mjs's in-Node YAML reader).
import { resolveEnforce } from './adapters.mjs';

const ROOT = process.cwd();
const POLICY_PATH = join(ROOT, 'harness', 'policies.yml');
const CODE_EXTS = new Set(['.ts', '.tsx', '.js', '.mjs', '.cjs', '.jsx', '.py', '.go', '.rs', '.java']);
const SKIP_DIRS = new Set(['node_modules', '.git', 'dist', 'build', '.next', 'coverage', 'vendor', '.forge']);
// A single line longer than this is a God-file even with few/no newlines
// (e.g. a minified bundle): the line-count cap alone would miss it.
const MAX_LINE_LEN = 2000;

export function parsePolicies(text) {
  const out = {};
  for (const line of text.split('\n')) {
    const m = line.split('#')[0].match(/^\s*([a-z_]+)\s*:\s*(\S+)\s*$/i);
    if (!m) continue;
    // Strip surrounding single/double quotes so a quoted value (`enforce: 'block'`)
    // classifies the same as its bare form — a quoted '500' must stay numeric and
    // a quoted 'warn' must not be mistaken for garbage that disables the gate.
    const value = m[2].replace(/^['"]|['"]$/g, '');
    out[m[1]] = /^\d+$/.test(value) ? Number(value) : value;
  }
  return out;
}

// Resolve a numeric policy: require a positive integer, else fail-closed.
// A garbage cap (e.g. '500abc') must NOT silently fall back and disable the gate.
function numericPolicy(value, fallback, key) {
  if (value === undefined) return fallback;
  if (typeof value === 'number' && Number.isInteger(value) && value > 0) return value;
  console.error(`forge-gate: ${key} must be a positive integer, got '${value}'`);
  process.exit(2);
}

function walk(dir, acc = []) {
  for (const name of readdirSync(dir)) {
    if (SKIP_DIRS.has(name)) continue;
    const full = join(dir, name);
    // statSync (not lstat) so symlinked dirs still traverse; skip entries that
    // throw — a broken symlink or permission error must not crash the gate.
    let st;
    try {
      st = statSync(full);
    } catch {
      continue;
    }
    if (st.isDirectory()) walk(full, acc);
    else acc.push(full);
  }
  return acc;
}

export function checkFileSizes(files, maxLines) {
  const out = [];
  for (const f of files) {
    if (!CODE_EXTS.has(extname(f))) continue;
    let text;
    try {
      text = readFileSync(f, 'utf8');
    } catch {
      continue; // unreadable (permission/broken) — skip rather than crash
    }
    const lines = text.split('\n');
    if (lines.length > maxLines) {
      out.push(`  ${relative(ROOT, f)}: ${lines.length} lines (max ${maxLines})`);
    }
    // Byte/line-length cap: a minified single-line God-file has few newlines but
    // a huge longest line — flag it regardless of line count.
    const longest = lines.reduce((max, l) => (l.length > max ? l.length : max), 0);
    if (longest > MAX_LINE_LEN) {
      out.push(`  ${relative(ROOT, f)}: longest line ${longest} chars (max ${MAX_LINE_LEN})`);
    }
  }
  return out;
}

export function checkRootCount(maxRoot) {
  const rootFiles = readdirSync(ROOT).filter((n) => {
    if (SKIP_DIRS.has(n)) return false;
    try {
      return statSync(join(ROOT, n)).isFile();
    } catch {
      return false; // broken symlink / permission — does not count toward root
    }
  });
  return rootFiles.length > maxRoot
    ? [`  root has ${rootFiles.length} files (max ${maxRoot})`]
    : [];
}

function main() {
  let policy;
  try {
    policy = parsePolicies(readFileSync(POLICY_PATH, 'utf8'));
  } catch {
    console.error(`forge-gate: cannot read ${POLICY_PATH}`);
    process.exit(2);
  }
  const maxLines = numericPolicy(policy.max_file_lines, 500, 'max_file_lines');
  const maxRoot = numericPolicy(policy.max_root_files, 15, 'max_root_files');
  // policies.yml's enforce is now the FALLBACK only — the live value comes from the
  // central knob (mode×lifecycle). Validate it first so a garbage policies.yml still
  // fails-closed loudly rather than feeding junk into the fail-safe path.
  const policyEnforce = policy.enforce ?? 'warn';
  if (policyEnforce !== 'warn' && policyEnforce !== 'block') {
    console.error(`forge-gate: enforce must be 'warn' or 'block', got '${policyEnforce}'`);
    process.exit(2);
  }
  // Resolve warn|block from .agent/project.yml × modes.yml: mode base (explorer warn
  // / engineering block) made stricter by the lifecycle floor (production -> block,
  // overriding a loose mode's warn). Missing .agent/field -> policyEnforce fallback.
  const enforce = resolveEnforce(ROOT, policyEnforce);

  const files = walk(ROOT);
  const violations = [...checkFileSizes(files, maxLines), ...checkRootCount(maxRoot)];

  // HONESTY: violations are ALWAYS reported (the file list + count below), in BOTH
  // warn and block — warn never pretends a clean tree. The ONLY difference is the
  // exit code: block -> 1 (stops the pipeline), warn -> 0 (advisory, speed first).
  if (violations.length === 0) {
    console.log(`forge-gate: PASS (${files.length} files, <=${maxLines} lines/file, root <=${maxRoot})`);
    process.exit(0);
  }
  console.log(`forge-gate: ${enforce === 'block' ? 'BLOCK' : 'WARN'} - ${violations.length} violation(s):`);
  console.log(violations.join('\n'));
  // Name the source so the strictness is auditable: enforce came from mode×lifecycle
  // (the central knob), not a bare policies.yml flag.
  console.log(
    enforce === 'block'
      ? 'Split oversized files before continuing (skill: refactor-large-file). [enforce=block from mode×lifecycle]'
      : 'Advisory only (exit 0); set a stricter mode/lifecycle (or production) to block. [enforce=warn from mode×lifecycle]',
  );
  process.exit(enforce === 'block' ? 1 : 0);
}

// Run only when executed directly (e.g. `node harness/gate.mjs`), not on import.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
