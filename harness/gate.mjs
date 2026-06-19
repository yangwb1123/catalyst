#!/usr/bin/env node
// ForgeOS harness gate (v0) — host-independent constraint enforcement.
// Checks: per-file line cap (code files) + root file count.
// Function-length & circular-deps are language-specific -> later adapters (see ROADMAP).
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, extname, relative } from 'node:path';
import { pathToFileURL } from 'node:url';

const ROOT = process.cwd();
const POLICY_PATH = join(ROOT, 'harness', 'policies.yml');
const CODE_EXTS = new Set(['.ts', '.tsx', '.js', '.mjs', '.cjs', '.jsx', '.py', '.go', '.rs', '.java']);
const SKIP_DIRS = new Set(['node_modules', '.git', 'dist', 'build', '.next', 'coverage', 'vendor']);
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
  const enforce = policy.enforce ?? 'warn';
  // Validate enforce against the allowed set: an unknown value (typo, garbage)
  // must fail-closed, not silently degrade the gate to a no-op.
  if (enforce !== 'warn' && enforce !== 'block') {
    console.error(`forge-gate: enforce must be 'warn' or 'block', got '${enforce}'`);
    process.exit(2);
  }

  const files = walk(ROOT);
  const violations = [...checkFileSizes(files, maxLines), ...checkRootCount(maxRoot)];

  if (violations.length === 0) {
    console.log(`forge-gate: PASS (${files.length} files, <=${maxLines} lines/file, root <=${maxRoot})`);
    process.exit(0);
  }
  console.log(`forge-gate: ${enforce === 'block' ? 'BLOCK' : 'WARN'} - ${violations.length} violation(s):`);
  console.log(violations.join('\n'));
  console.log(
    enforce === 'block'
      ? 'Split oversized files before continuing (skill: refactor-large-file).'
      : 'Advisory only. Set `enforce: block` in harness/policies.yml to enforce.',
  );
  process.exit(enforce === 'block' ? 1 : 0);
}

// Run only when executed directly (e.g. `node harness/gate.mjs`), not on import.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
