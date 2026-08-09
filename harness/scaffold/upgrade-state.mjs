// Persisted scaffold-state parsing and retired-file discovery for forge-upgrade.
// The ledger is target-controlled input: keep all path canonicalization and
// current-asset alias defenses together so the mutation orchestrator stays thin.
import {
  lstatSync,
  realpathSync,
} from 'node:fs';
import {
  isAbsolute, join, sep,
} from 'node:path';

import {
  GOVERNANCE_DIRS,
  SCAFFOLD_STATE_FILE,
} from './forge-init.mjs';
import {
  assertNoSymlinkComponents,
  assertSafeRegularFile,
  readFileNoFollow,
} from './scaffold-fs.mjs';

const HISTORICAL_ROOTS = [
  ...GOVERNANCE_DIRS,
  join('.agent', 'agents'),
  join('.agent', 'skills'),
  join('.agent', 'workflows'),
  join('.agent', 'eval'),
  join('.agent', 'routing'),
  join('.agent', 'policies'),
  join('.ai', 'prompts'),
].map((dir) => dir.replace(/\\/g, '/'));

// Accept POSIX or Windows separators so a scaffold can move between hosts, but
// reject every empty/dot/dot-dot segment before converting to the current host.
// Windows removes trailing dots/spaces from ordinary path segments, so reject
// those spellings too: ".. " must never become an unobserved parent traversal.
function canonicalHistoricalPath(rel) {
  if (
    typeof rel !== 'string'
    || rel.length === 0
    || rel.includes('\0')
    || isAbsolute(rel)
  ) return null;
  const portable = rel.replace(/\\/g, '/');
  const segments = portable.split('/');
  if (
    portable.startsWith('/')
    || /^[A-Za-z]:/.test(portable)
    || segments.some((segment) => (
      segment === ''
      || segment === '.'
      || segment === '..'
      || /[ .]$/u.test(segment)
      || segment.includes(':')
    ))
  ) return null;
  const clean = segments.join('/');
  if (clean.startsWith('harness/')) return segments.join(sep);
  if (clean === '.agent/AGENTS.md' || clean === '.arch/rules.yaml') {
    return segments.join(sep);
  }
  if (clean === 'docs/release/README.md') return segments.join(sep);
  if (
    clean === 'docs/design/ai-engineering-os/capability-catalog.v1.yml'
    || clean === 'docs/design/ai-engineering-os/capability-skill-map.v1.yml'
    || clean === 'docs/design/ai-engineering-os/backend-decision-standard.md'
    || clean === 'docs/design/ai-engineering-os/frontend-design-standard.md'
    || clean === 'docs/adr/0042-frontend-design-decision-contract.md'
  ) return segments.join(sep);
  return HISTORICAL_ROOTS.some((dir) => clean.startsWith(`${dir}/`))
    ? segments.join(sep)
    : null;
}

// Conservative keying rejects aliases that case-folding or Windows filesystems
// commonly resolve to one path. Actual identity comparison below covers host-
// specific equivalence, Unicode aliases, and hardlinks that strings cannot model.
function portableAliasKey(rel) {
  return rel.replace(/\\/g, '/').split('/').map(
    (segment) => segment.normalize('NFC').replace(/[ .]+$/u, '').toLowerCase(),
  ).join('/');
}

function existingPathIdentity(path, label) {
  assertNoSymlinkComponents(path, label);
  let st;
  try {
    st = lstatSync(path);
  } catch (err) {
    if (err?.code === 'ENOENT' || err?.code === 'ENOTDIR') return null;
    throw new Error(`cannot safely inspect ${label}: ${err.message}`);
  }
  if (st.isSymbolicLink() || !st.isFile()) {
    throw new Error(`refusing unsafe non-regular path for ${label}: ${path}`);
  }
  let real;
  try {
    real = realpathSync.native(path);
  } catch (err) {
    throw new Error(`cannot safely resolve ${label}: ${err.message}`);
  }
  return { dev: st.dev, ino: st.ino, real };
}

function samePathIdentity(a, b) {
  if (a.real === b.real) return true;
  // Some platforms expose 0/0 when file IDs are unavailable. Do not treat that
  // sentinel as evidence that every pair of files aliases.
  const usable = !(Number(a.dev) === 0 && Number(a.ino) === 0)
    && !(Number(b.dev) === 0 && Number(b.ino) === 0);
  return usable && a.dev === b.dev && a.ino === b.ino;
}

function readScaffoldState(targetDir) {
  const statePath = join(targetDir, SCAFFOLD_STATE_FILE);
  assertNoSymlinkComponents(statePath, SCAFFOLD_STATE_FILE);
  try {
    lstatSync(statePath);
  } catch (err) {
    if (err?.code === 'ENOENT' || err?.code === 'ENOTDIR') return [];
    throw new Error(`cannot safely inspect ${SCAFFOLD_STATE_FILE}: ${err.message}`);
  }
  let parsed;
  try {
    parsed = JSON.parse(readFileNoFollow(statePath, SCAFFOLD_STATE_FILE, 'utf8'));
  } catch (err) {
    throw new Error(`invalid ${SCAFFOLD_STATE_FILE}: ${err.message}`);
  }
  if (!Array.isArray(parsed?.copied)) {
    throw new Error(`invalid ${SCAFFOLD_STATE_FILE}: expected copied[]`);
  }
  const canonical = [];
  const unsafe = [];
  for (const rel of parsed.copied) {
    const clean = canonicalHistoricalPath(rel);
    if (clean === null) unsafe.push(rel);
    else canonical.push(clean);
  }
  if (unsafe.length > 0) {
    throw new Error(`unsafe path(s) in ${SCAFFOLD_STATE_FILE}: ${unsafe.join(', ')}`);
  }
  return [...new Set(canonical)].sort();
}

function currentIdentityEntries(currentPaths, targetDir) {
  return currentPaths.map((rel) => ({
    rel,
    identity: existingPathIdentity(join(targetDir, rel), `current target ${rel}`),
  })).filter(({ identity }) => identity !== null);
}

// A retired path must be ledger-recorded, absent from the new projection, and
// distinct by portable spelling and actual identity from every current asset.
export function removedFilesForProjection(currentPaths, targetDir) {
  const current = new Set(currentPaths);
  const portableCurrent = new Map(
    currentPaths.map((rel) => [portableAliasKey(rel), rel]),
  );
  const identities = currentIdentityEntries(currentPaths, targetDir);
  const retiredPortable = new Map();
  const retiredIdentities = [];
  const removed = [];
  for (const rel of readScaffoldState(targetDir)) {
    if (current.has(rel)) continue;
    const portableKey = portableAliasKey(rel);
    const portableMatch = portableCurrent.get(portableKey);
    if (portableMatch !== undefined) {
      throw new Error(
        `refusing retired scaffold path ${rel}: aliases current governed path ${portableMatch}`,
      );
    }
    const priorPortable = retiredPortable.get(portableKey);
    if (priorPortable !== undefined) {
      throw new Error(
        `refusing retired scaffold path ${rel}: aliases retired path ${priorPortable}`,
      );
    }
    retiredPortable.set(portableKey, rel);
    const destination = join(targetDir, rel);
    const identity = existingPathIdentity(destination, `retired target ${rel}`);
    if (identity === null) continue;
    const actualMatch = identities.find((entry) => samePathIdentity(identity, entry.identity));
    if (actualMatch) {
      throw new Error(
        `refusing retired scaffold path ${rel}: aliases current governed path ${actualMatch.rel}`,
      );
    }
    const retiredMatch = retiredIdentities.find(
      (entry) => samePathIdentity(identity, entry.identity),
    );
    if (retiredMatch) {
      throw new Error(
        `refusing retired scaffold path ${rel}: aliases retired path ${retiredMatch.rel}`,
      );
    }
    assertSafeRegularFile(destination, `retired target ${rel}`);
    retiredIdentities.push({ rel, identity });
    removed.push(rel);
  }
  return removed.sort();
}
