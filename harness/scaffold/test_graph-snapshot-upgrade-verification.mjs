import assert from 'node:assert/strict';
import { dirname } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  assertGraphSnapshotRegistryVersion,
  GRAPH_SNAPSHOT_REGISTRY_VERSION,
} from './graph-snapshot-upgrade-verification.mjs';

const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));

test('GraphSnapshot scaffold verifier locks governance registry version 39', () => {
  assert.equal(GRAPH_SNAPSHOT_REGISTRY_VERSION, 39);
  assert.doesNotThrow(() => assertGraphSnapshotRegistryVersion(SOURCE_ROOT));
});
