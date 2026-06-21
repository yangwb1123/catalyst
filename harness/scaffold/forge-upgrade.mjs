#!/usr/bin/env node
// ForgeOS forge-upgrade — resync an ALREADY-scaffolded project's COPIED governance
// (the 70%) from a ForgeOS SOURCE repo. forge-init stamps a project once; over time
// the SOURCE harness/governance evolves (a hardened acceptance.mjs, a new arch
// check, a fixed scorecard-update) and the project's COPIED copy falls behind. This
// command brings that copy back to byte-identical-with-source — so a NEW mechanism
// reaches an OLD project without re-scaffolding (which would clobber its identity).
//
// ★ SCOPE — what upgrade DOES and DOES NOT fix (be honest, do not overclaim) ★
// There are TWO kinds of drift; upgrade addresses exactly ONE:
//   (A) COPIED harness/asset lag — the project's on-disk copy of acceptance.mjs /
//       arch-check.mjs / the .agent governance assets is older than SOURCE.
//       ► upgrade FIXES this: it resyncs the project's copied 70% to SOURCE bytes.
//   (B) forge-core (Go) BINARY behavior change — e.g. per-model latency/cost logic
//       in command_executor.go / cost.go. The project carries NO forge-core; it runs
//       the HOST's `forge` binary. Nothing in the project tree encodes that behavior.
//       ► upgrade CANNOT fix this. Upgrade your `forge` binary separately (rebuild /
//         reinstall it). This tool does not touch, and cannot change, binary behavior.
//
// ★ THE RED LINE — upgrade NEVER touches project IDENTITY (the 30%) ★
// It mechanically iterates ONLY the two shared manifests (GOVERNANCE_DIRS +
// COPIED_FILES, imported from forge-init — the SAME single source of truth the
// scaffolder copies). It has NO render*/writeGenerated/identity code path at all,
// so PROJECT/ROADMAP/CURRENT_SPRINT/project.yml/README/.gitignore/examples/ — the
// user's task list, mode/lifecycle, prose — are unreachable by construction.
// Overwriting them would be catastrophic data loss; the guarantee is that the code
// literally cannot, reinforced by test #2 (written ∩ GENERATED_FILES = ∅).
//
// FAIL-SAFE: DRY by default — it reports the drift and writes NOTHING. Only an
// explicit --apply mutates the project, and even then it first BACKS UP every file
// it is about to overwrite (.forge/upgrade-backup/<timestamp>/<rel>, git-ignored)
// unless --no-backup. Idempotent: a second --apply against the same SOURCE finds
// everything unchanged and writes (and backs up) nothing.
//
// Usage:
//   node harness/forge-upgrade.mjs --from <forgeos-repo> --target <project> \
//        [--apply] [--no-backup] [--prune]
//   (default is DRY: print the drift report, write nothing)
import {
  readFileSync,
  writeFileSync,
  copyFileSync,
  mkdirSync,
  existsSync,
} from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';

// Shared SINGLE SOURCE OF TRUTH: the same manifests forge-init copies, imported
// (never re-transcribed) so scaffold and upgrade can never disagree on the 70%.
import { GOVERNANCE_DIRS, COPIED_FILES } from './forge-init.mjs';
// Shared PURE projection of a copied governance tree (same __pycache__ skip the
// scaffolder used) — so the set upgrade compares is exactly the set scaffold wrote.
import { enumerateTree } from './scaffold-fs.mjs';

// --- pure core (no writes; unit-testable) ------------------------------------

// manifestProjection(sourceRoot) -> the FLAT list of every relative path the 70%
// covers: each GOVERNANCE_DIR expanded through enumerateTree (its concrete files)
// plus the explicit COPIED_FILES. This — and ONLY this — is what upgrade ever
// considers; identity files are not in either manifest, so they never appear.
export function manifestProjection(sourceRoot) {
  const fromDirs = GOVERNANCE_DIRS.flatMap((d) => enumerateTree(d, sourceRoot));
  return [...fromDirs, ...COPIED_FILES];
}

// byteEqual(a, b): true iff the two files exist and are byte-identical. A missing
// side is NOT equal (the caller distinguishes added/removed before calling). The
// SAME deepEqual-on-bytes test test_forge-init uses for "copied == source".
function byteEqual(aPath, bPath) {
  if (!existsSync(aPath) || !existsSync(bPath)) return false;
  const a = readFileSync(aPath);
  const b = readFileSync(bPath);
  return a.equals(b);
}

// classifyDrift(sourceRoot, targetDir) -> { added, changed, unchanged } over the
// manifest projection, each an array of relative paths (sorted, stable output):
//   added     — in SOURCE, absent in the project (a new harness tool / asset).
//   changed   — present in both but bytes differ (the project's copy lagged).
//   unchanged — byte-identical already (idempotent: a re-run is all-unchanged).
// PURE: only reads; classification is the whole job, the I/O apply path acts on it.
export function classifyDrift(sourceRoot, targetDir) {
  const added = [];
  const changed = [];
  const unchanged = [];
  for (const rel of manifestProjection(sourceRoot)) {
    const dst = join(targetDir, rel);
    if (!existsSync(dst)) added.push(rel);
    else if (byteEqual(join(sourceRoot, rel), dst)) unchanged.push(rel);
    else changed.push(rel);
  }
  return {
    added: added.sort(),
    changed: changed.sort(),
    unchanged: unchanged.sort(),
  };
}

// --- I/O boundary ------------------------------------------------------------

// CLI arg parse. Returns a config object or throws on a usage error. DRY is the
// default (apply false); backups default ON (the safe default), --no-backup opts
// out; --prune is opt-in (this v1 only DISPLAYS removed files, never deletes).
export function parseArgs(argv) {
  const out = { from: null, target: null, apply: false, backup: true, prune: false };
  const takesValue = { '--from': 'from', '--target': 'target' };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--apply') out.apply = true;
    else if (a === '--no-backup') out.backup = false;
    else if (a === '--prune') out.prune = true;
    else if (a in takesValue) {
      const v = argv[++i];
      if (v === undefined || v.startsWith('--')) {
        throw new Error(`flag ${a} requires a value (got ${v ?? 'nothing'})`);
      }
      out[takesValue[a]] = v;
    } else if (a.startsWith('--')) throw new Error(`unknown flag: ${a}`);
    else throw new Error(`unexpected argument: ${a}`);
  }
  if (!out.from) throw new Error('missing --from <forgeos-repo>');
  if (!out.target) throw new Error('missing --target <project>');
  return out;
}

// sourceSha(sourceRoot): the SOURCE repo's HEAD commit (read-only git), so the
// report can honestly stamp "synced from <sha>". Returns a short sha or 'unknown'
// (not a git repo / git absent) — never throws; the upgrade does not depend on it.
function sourceSha(sourceRoot) {
  const r = spawnSync('git', ['-C', sourceRoot, 'rev-parse', '--short', 'HEAD'], {
    encoding: 'utf8',
  });
  return r.status === 0 ? r.stdout.trim() : 'unknown';
}

// removedFiles(sourceRoot, targetDir): manifest-projection paths present in the
// PROJECT but absent in SOURCE — a file SOURCE no longer ships. We only ever
// project paths that exist in SOURCE, so "removed" cannot come from the projection;
// instead it is COPIED_FILES entries whose SOURCE file is gone (a renamed/retired
// tool). Displayed for the operator; deleted only under --prune.
function removedFiles(sourceRoot, targetDir) {
  const out = [];
  for (const rel of COPIED_FILES) {
    if (!existsSync(join(sourceRoot, rel)) && existsSync(join(targetDir, rel))) {
      out.push(rel);
    }
  }
  return out.sort();
}

// backupTimestamp(): a filesystem-safe ISO-ish stamp for the backup dir name
// (colons -> dashes), e.g. 2026-06-21T12-30-00-000Z. Passed in at the boundary.
function backupTimestamp(now) {
  return now.toISOString().replace(/:/g, '-');
}

// applyOne(rel, sourceRoot, targetDir, backupDir): overwrite the project's <rel>
// with SOURCE's bytes, FIRST copying the existing project file (if any) into
// backupDir/<rel> when backupDir is set. Returns true if a backup was taken. The
// backup preserves the user's possibly-modified copy verbatim — zero loss.
function applyOne(rel, sourceRoot, targetDir, backupDir) {
  const dst = join(targetDir, rel);
  let backedUp = false;
  if (backupDir && existsSync(dst)) {
    const bak = join(backupDir, rel);
    mkdirSync(dirname(bak), { recursive: true });
    copyFileSync(dst, bak);
    backedUp = true;
  }
  mkdirSync(dirname(dst), { recursive: true });
  copyFileSync(join(sourceRoot, rel), dst);
  return backedUp;
}

// applyDrift(drift, paths): write every changed + added file from SOURCE into the
// project (changed ones backed up first when backup is on). unchanged is NEVER
// touched — so a re-run that finds all-unchanged writes nothing and backs up
// nothing (idempotent). Returns { written, backedUp } counts for the report.
// Added files have no project copy to back up; changed files do. Only reached
// under --apply (main gates it), and only ever over manifest-projection paths —
// the red line (no identity path) holds structurally.
function applyDrift(drift, { sourceRoot, targetDir, backupDir }) {
  let written = 0;
  let backedUp = 0;
  for (const rel of [...drift.changed, ...drift.added]) {
    if (applyOne(rel, sourceRoot, targetDir, backupDir)) backedUp += 1;
    written += 1;
  }
  return { written, backedUp };
}

// printReport: the drift summary + per-file lines. THE WHOLE output in dry mode,
// the preamble in apply mode, so the operator always sees exactly what will change.
function printReport(drift, removed, { sha, apply }) {
  console.log(`forge-upgrade: ${apply ? 'APPLY' : 'DRY'} — synced from ${sha}`);
  console.log(
    `  changed: ${drift.changed.length}  added: ${drift.added.length}  ` +
      `unchanged: ${drift.unchanged.length}  removed: ${removed.length}`,
  );
  for (const rel of drift.changed) console.log(`  changed: ${rel}`);
  for (const rel of drift.added) console.log(`  added: ${rel}`);
  for (const rel of removed) console.log(`  removed: ${rel} (in project, gone from source)`);
}

// printHonestScope: the standing disclaimer — upgrade resyncs the copied 70%; it
// does NOT change forge-core binary behavior. Printed every run so the operator is
// never misled into thinking a binary-behavior drift was fixed here.
function printHonestScope() {
  console.log('');
  console.log('scope: I resync this project\'s COPIED harness + governance (the 70%).');
  console.log('       I do NOT change forge-core binary behavior — if a per-model');
  console.log('       latency/cost or other Go-runtime change is what you need,');
  console.log('       upgrade your `forge` binary separately (this tool cannot).');
}

// run(cfg, now): the orchestration. Classify drift, print the report + honest
// scope, and — only on --apply — back up + write changed/added. Returns a result
// object (for tests + the exit path). Pure of process.exit so it stays testable.
export function run(cfg, now = new Date()) {
  const sourceRoot = resolve(cfg.from);
  const targetDir = resolve(cfg.target);
  const drift = classifyDrift(sourceRoot, targetDir);
  const removed = removedFiles(sourceRoot, targetDir);
  printReport(drift, removed, { sha: sourceSha(sourceRoot), apply: cfg.apply });

  if (!cfg.apply) {
    console.log('');
    console.log('forge-upgrade: DRY run — nothing written. Re-run with --apply to resync.');
    printHonestScope();
    return { drift, removed, written: 0, backedUp: 0, applied: false };
  }

  const backupDir = cfg.backup
    ? join(targetDir, '.forge', 'upgrade-backup', backupTimestamp(now))
    : null;
  const { written, backedUp } = applyDrift(drift, { sourceRoot, targetDir, backupDir });
  console.log('');
  if (written === 0) {
    console.log('forge-upgrade: APPLIED — already in sync; nothing written, nothing backed up.');
  } else {
    console.log(
      `forge-upgrade: APPLIED — ${written} file(s) resynced` +
        (backedUp > 0 ? `; ${backedUp} overwritten file(s) backed up to ${backupDir}` : ''),
    );
  }
  if (cfg.prune && removed.length > 0) {
    console.log(`  (note: --prune deletion of ${removed.length} removed file(s) is not yet implemented; displayed only)`);
  }
  printHonestScope();
  return { drift, removed, written, backedUp, backupDir, applied: true };
}

function main(argv) {
  let cfg;
  try {
    cfg = parseArgs(argv);
  } catch (err) {
    console.error(`forge-upgrade: ${err.message}`);
    console.error(
      'usage: node harness/forge-upgrade.mjs --from <forgeos-repo> --target <project> ' +
        '[--apply] [--no-backup] [--prune]',
    );
    process.exit(2);
  }
  try {
    run(cfg);
  } catch (err) {
    console.error(`forge-upgrade: ${err.message}`);
    process.exit(1);
  }
  process.exit(0);
}

// Run only when executed directly, not on import (keeps run/classify unit-testable).
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2));
}
