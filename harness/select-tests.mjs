#!/usr/bin/env node
// ForgeOS incremental test selection (ROADMAP direction-5) — a FAST, ADVISORY
// edit-time signal that runs only the suites relevant to the files a git diff
// changed, so a long evolve/dev loop need not re-run the whole suite for a
// few-file change. The win compounds over a 24h multi-iteration run.
//
// ★HONESTY / SAFETY — this is NOT the gate.★ It NEVER replaces `forge accept`,
// which always runs EVERY suite and is the single load-bearing source of truth
// (a governance OS must never let an incremental selector greenlight a regression
// in an unselected suite — the false-PASS class the enforcers exist to prevent).
// This tool is the same shape as the CC PostToolUse accelerator: a fast advisory
// signal, fail-safe toward "run the full gate". So:
//   - a changed file with NO mapped suite is reported as UNMAPPED, and its presence
//     makes the tool recommend a full `forge accept` (it never silently covers it);
//   - the exit code reflects only the SELECTED suites, and the banner states plainly
//     that a clean result here does not substitute for the full gate.
//
// Zero third-party deps (node: builtins + git/go/python3 via the shared kernel
// run()). The file->suite mapping (suitesForChanged) is PURE — `exists` is injected
// — so it is unit-testable without a working tree.
import { existsSync } from 'node:fs';
import { join, relative, dirname, basename, extname } from 'node:path';
import { pathToFileURL } from 'node:url';
import { run, HARNESS_DIR, ROOT } from './acceptance-kernel.mjs';

// A selected suite is a kernel.run() invocation plus a human label. dedupKey keeps
// two changed files that map to the SAME suite from running it twice.
const suite = (label, cmd, args, cwd = ROOT) => ({ label, cmd, args, cwd });

// mapFile: one changed repo-relative path -> the suites that exercise it (possibly
// none). Heuristic and deliberately conservative: when a file's relevant suite
// cannot be named with confidence it returns [], so the caller reports it UNMAPPED
// (-> recommend the full gate) rather than guessing a partial cover. `exists` is
// injected (testability + so a sibling test that isn't present is simply skipped);
// it defaults to a real working-tree check, matching suitesForChanged.
export function mapFile(f, exists = (p) => existsSync(join(ROOT, p))) {
  const rel = f.replace(/\\/g, '/');
  // A changed TEST file always runs itself — the most direct signal.
  if (/(^|\/)test_[^/]*\.mjs$/.test(rel)) return [suite(rel, 'node', ['--test', '--test-reporter=tap', rel])];
  if (/(^|\/)test_[^/]*\.py$/.test(rel)) return [suite(rel, 'python3', [join(ROOT, rel)])];
  // A changed harness MODULE (.mjs/.py) -> its sibling test_<name>, when present.
  if (rel.startsWith('harness/') && (rel.endsWith('.mjs') || rel.endsWith('.py'))) {
    return harnessSibling(rel, exists);
  }
  // A changed Go file -> `go test` on its package dir (run from the module root).
  if (rel.startsWith('forge-core/') && rel.endsWith('.go')) {
    const pkg = './' + (relative('forge-core', dirname(rel)) || '.');
    return [suite(`go test ${pkg}`, 'go', ['test', pkg], join(ROOT, 'forge-core'))];
  }
  // A changed governance asset -> the governance integrity + architecture checks.
  if (rel.startsWith('.agent/') || rel.startsWith('.arch/')) return governanceSuites();
  return [];
}

// harnessSibling: map harness/<dir>/<name>.<ext> to harness/<dir>/test_<name>.<ext>
// when that sibling test exists. A harness file with no sibling test returns []
// (UNMAPPED) — honest: we do not know which suite covers it, so recommend the gate.
function harnessSibling(rel, exists) {
  const dir = dirname(rel);
  const base = basename(rel);
  const ext = extname(base); // .mjs | .py
  const sib = join(dir, `test_${base}`).replace(/\\/g, '/');
  if (!exists(sib)) return [];
  if (ext === '.py') return [suite(sib, 'python3', [join(ROOT, sib)])];
  return [suite(sib, 'node', ['--test', '--test-reporter=tap', sib])];
}

function governanceSuites() {
  return [
    suite('check.py (governance)', 'python3', [join(HARNESS_DIR, 'check.py')]),
    suite('arch-check', 'node', [join(HARNESS_DIR, 'arch', 'arch-check.mjs')]),
  ];
}

// suitesForChanged: PURE reduction of a changed-file list to the DISTINCT suites to
// run plus the UNMAPPED files (which trigger the full-gate recommendation). Order is
// first-seen for a deterministic run sequence.
export function suitesForChanged(files, exists = (p) => existsSync(join(ROOT, p))) {
  const selected = new Map();
  const unmapped = [];
  for (const f of files) {
    const suites = mapFile(f, exists);
    if (suites.length === 0) { unmapped.push(f); continue; }
    for (const s of suites) if (!selected.has(s.label)) selected.set(s.label, s);
  }
  return { selected: [...selected.values()], unmapped };
}

// changedFiles: the tracked+untracked paths git reports against `base` (default the
// working tree vs HEAD). Returns [] on any git failure (not a git repo, etc.), so the
// tool degrades to "nothing selected -> recommend full gate" rather than crashing.
function changedFiles(base) {
  const args = base
    ? ['diff', '--name-only', base]
    : ['diff', '--name-only', 'HEAD'];
  const tracked = run('git', args);
  const untracked = run('git', ['ls-files', '--others', '--exclude-standard']);
  const lines = `${tracked.ok ? tracked.out : ''}\n${untracked.ok ? untracked.out : ''}`
    .split('\n').map((s) => s.trim()).filter(Boolean);
  return [...new Set(lines)];
}

function main() {
  const args = process.argv.slice(2);
  const baseIdx = args.indexOf('--base');
  const base = baseIdx >= 0 ? args[baseIdx + 1] : '';
  // Any non-flag args are treated as an explicit changed-file list (testing / CI).
  // Exclude --base's VALUE arg only when --base is actually present (baseIdx >= 0);
  // otherwise baseIdx+1 == 0 would wrongly drop the first file.
  const explicit = args.filter((a, i) => !a.startsWith('--') && !(baseIdx >= 0 && i === baseIdx + 1));
  const files = explicit.length ? explicit : changedFiles(base);

  console.log('forge-select: INCREMENTAL test signal (ADVISORY — NOT the gate).');
  console.log('  The load-bearing source of truth is `node harness/acceptance.mjs` (forge accept),');
  console.log('  which always runs EVERY suite. A clean result here does NOT substitute for it.');

  if (files.length === 0) {
    console.log('forge-select: no changed files detected — nothing to run. (Run forge accept for the full gate.)');
    process.exit(0);
  }
  const { selected, unmapped } = suitesForChanged(files);
  if (selected.length === 0) {
    console.log(`forge-select: ${files.length} changed file(s), none mapped to a suite — run the FULL gate.`);
    for (const u of unmapped) console.log(`    unmapped: ${u}`);
    process.exit(0);
  }
  console.log(`forge-select: ${files.length} changed file(s) -> ${selected.length} selected suite(s):`);
  let failed = 0;
  for (const s of selected) {
    const r = run(s.cmd, s.args, {}, s.cwd);
    const ok = r.ok;
    if (!ok) failed += 1;
    console.log(`  [${ok ? 'PASS' : 'FAIL'}] ${s.label}`);
    if (!ok) console.log(r.out.split('\n').slice(-8).map((l) => `      ${l}`).join('\n'));
  }
  if (unmapped.length) {
    console.log(`forge-select: ${unmapped.length} changed file(s) had NO mapped suite — run the FULL gate to cover them:`);
    for (const u of unmapped) console.log(`    unmapped: ${u}`);
  }
  console.log(
    failed === 0
      ? `forge-select: ${selected.length} selected suite(s) green${unmapped.length ? ' (but unmapped files remain — full gate still required)' : ''}. Advisory only.`
      : `forge-select: ${failed} selected suite(s) FAILED — fix before the full gate.`,
  );
  // Exit reflects the SELECTED suites only; this is advisory, never the gate.
  process.exit(failed === 0 ? 0 : 1);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
