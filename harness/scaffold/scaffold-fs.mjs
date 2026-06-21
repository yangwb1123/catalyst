// ForgeOS scaffold-fs — the shared SOURCE-tree copy/enumeration primitives used
// by BOTH forge-init (scaffold a new project) and forge-upgrade (resync an
// existing project's copied governance). Extracted so the copy semantics live in
// ONE place: the recursive __pycache__-skipping walk that decides what a copied
// governance-asset tree contains is a SINGLE source of truth, not duplicated
// between the scaffolder and the upgrader (where they could silently drift —
// upgrade enumerating a tree differently than scaffold copied it would mean a
// project that can never reach byte-identical-to-source).
//
// Three primitives over a `sourceRoot` (the ForgeOS SOURCE repo) and a `targetDir`:
//   * copyFromSource(rel, ...)  — copy one file, creating parent dirs.
//   * copyTree(relDir, ...)     — recursively copy a whole dir (skips __pycache__).
//   * enumerateTree(relDir, sourceRoot) -> rel[]  — the PURE projection of what
//       copyTree WOULD copy (same __pycache__ skip rule), with NO writes. upgrade
//       expands GOVERNANCE_DIRS through this to know every file a tree contributes.
//
// Zero third-party deps (node: builtins only). This is a SCAFFOLD/UPGRADE-time
// tool, not project runtime governance, so it is on forge-init's HARNESS_NOT_COPIED
// whitelist (a generated project does not scaffold sub-projects).
import { mkdirSync, copyFileSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';

// Copy one file from <sourceRoot>/<relPath> into <targetDir>/<relPath>, creating
// parent dirs. Pushes relPath onto `created` so the caller can report what landed.
export function copyFromSource(relPath, sourceRoot, targetDir, created) {
  const dest = join(targetDir, relPath);
  mkdirSync(dirname(dest), { recursive: true });
  copyFileSync(join(sourceRoot, relPath), dest);
  created.push(relPath);
}

// Recursively copy a whole SOURCE directory tree into the target (verbatim),
// preserving structure. Used for the .agent governance-asset dirs. Skips Python
// bytecode caches so a generated project ships clean source only. The __pycache__
// skip here is THE rule enumerateTree mirrors — keep them in lockstep.
export function copyTree(relDir, sourceRoot, targetDir, created) {
  const srcDir = join(sourceRoot, relDir);
  for (const entry of readdirSync(srcDir, { withFileTypes: true })) {
    if (entry.name === '__pycache__') continue;
    const childRel = join(relDir, entry.name);
    if (entry.isDirectory()) copyTree(childRel, sourceRoot, targetDir, created);
    else copyFromSource(childRel, sourceRoot, targetDir, created);
  }
}

// enumerateTree(relDir, sourceRoot) -> array of relative paths copyTree WOULD copy
// from <sourceRoot>/<relDir>, in directory order, applying the SAME __pycache__
// skip. PURE-ish: it reads the source dir structure but writes NOTHING — the
// read-only twin of copyTree, so forge-upgrade can project GOVERNANCE_DIRS into
// concrete files (to byte-compare each against the target) using the exact set
// the scaffolder would have produced. Single source of truth for "what is in a
// copied governance tree" — if copyTree's skip rule changes, change it once here.
export function enumerateTree(relDir, sourceRoot) {
  const out = [];
  const srcDir = join(sourceRoot, relDir);
  for (const entry of readdirSync(srcDir, { withFileTypes: true })) {
    if (entry.name === '__pycache__') continue;
    const childRel = join(relDir, entry.name);
    if (entry.isDirectory()) out.push(...enumerateTree(childRel, sourceRoot));
    else out.push(childRel);
  }
  return out;
}
