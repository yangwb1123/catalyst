// Immutable source capture and target classification for forge-upgrade.
import { lstatSync } from 'node:fs';
import { join, resolve } from 'node:path';

import {
  PROJECT_INSTANCE_FILES, copiedProjection,
} from './forge-init.mjs';
import { snapshotFileNoFollow } from './scaffold-fs.mjs';
import { scaffoldOwnedFiles } from './upgrade-state.mjs';
import {
  assertParentBoundary, captureParentBoundary, closeParentBoundary,
} from './upgrade-path-boundary.mjs';

function lexicalExists(path) {
  try {
    lstatSync(path);
    return true;
  } catch (error) {
    if (error?.code === 'ENOENT' || error?.code === 'ENOTDIR') return false;
    throw new Error(`cannot safely inspect ${path}: ${error.message}`);
  }
}

function sourceSession(sourceRoot) {
  return captureParentBoundary(
    sourceRoot, join(sourceRoot, '.forge-upgrade-source-session'),
    'upgrade source session',
  );
}

function freezePinnedSource(sourceRoot, projection, afterSourceRead) {
  const session = sourceSession(sourceRoot);
  try {
    const root = session.components[0];
    const sourceView = join(session.descriptorRoot, String(root.fd));
    const selected = projection ?? copiedProjection(sourceView);
    assertParentBoundary(session);
    const files = new Map();
    const paths = [...new Set([...selected, ...PROJECT_INSTANCE_FILES])];
    for (const [index, rel] of paths.entries()) {
      files.set(rel, snapshotFileNoFollow(join(sourceView, rel), `source ${rel}`));
      afterSourceRead?.({ index, rel, total: paths.length });
      assertParentBoundary(session);
    }
    return Object.freeze({
      files, projection: Object.freeze([...selected]),
    });
  } finally {
    closeParentBoundary(session);
  }
}

export function freezeSourceProjection(
  sourceRoot, projection = null, afterSourceRead = null,
) {
  return freezePinnedSource(resolve(sourceRoot), projection, afterSourceRead);
}

export function classifyFrozenDrift(sourcePlan, targetDir, afterTargetRead = null) {
  const drift = { added: [], changed: [], unchanged: [], unowned: [] };
  const snapshots = new Map();
  const priorProjection = new Set(scaffoldOwnedFiles(targetDir));
  for (const [index, rel] of sourcePlan.projection.entries()) {
    const destination = join(targetDir, rel);
    if (!lexicalExists(destination)) drift.added.push(rel);
    else if (!priorProjection.has(rel)) drift.unowned.push(rel);
    else {
      const snapshot = snapshotFileNoFollow(destination, `target ${rel}`);
      snapshots.set(rel, snapshot);
      const label = snapshot.bytes.equals(sourcePlan.files.get(rel).bytes)
        ? 'unchanged' : 'changed';
      drift[label].push(rel);
    }
    afterTargetRead?.({ index, rel, total: sourcePlan.projection.length });
  }
  for (const values of Object.values(drift)) values.sort();
  return { drift, snapshots };
}

export function classifyDrift(sourceRoot, targetDir, afterTargetRead = null) {
  return classifyFrozenDrift(
    freezeSourceProjection(sourceRoot), targetDir, afterTargetRead,
  ).drift;
}
