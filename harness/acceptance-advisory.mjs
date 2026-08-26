// Explicit advisory-only cache coordinator. Formal and --json paths never load
// this module; acceptance.mjs reaches it only after parsing a literal --cache.
import { FAIL, ROOT, withCategory } from './acceptance-kernel.mjs';
import {
  CACHEABLE_PLANS, cacheDisabled, cachedDecide, fingerprintProbes, loadCache,
  saveRows,
} from './acceptance-cache.mjs';
import { SUBTASKS, assembleResults, runSubtasks } from './acceptance-parallel.mjs';

function failureRow(name, detail) {
  return withCategory({ criterion: name, status: FAIL, detail });
}

function prepareCache(root, env) {
  const cached = new Map();
  const names = Object.keys(CACHEABLE_PLANS);
  try {
    const fingerprints = fingerprintProbes(names, root, env);
    const store = loadCache(root);
    for (const name of names) {
      const criterion = CACHEABLE_PLANS[name].criterion;
      const row = cachedDecide(name, criterion, root, fingerprints.get(name), store);
      if (row) cached.set(name, withCategory(row));
    }
    return { cached, fingerprints };
  } catch {
    return { cached, fingerprints: null };
  }
}

function invalidateRows(byName, names, detail) {
  for (const name of names) {
    byName.set(name, failureRow(name, `${name}: ${detail}`));
  }
}

function cacheEntries(fingerprints, byName) {
  return Object.keys(CACHEABLE_PLANS).map((name) => ({
    name,
    fingerprint: fingerprints.get(name),
    result: {
      status: byName.get(name).status,
      detail: byName.get(name).detail.replace(/ \[advisory cache\]$/, ''),
      category: byName.get(name).category,
    },
  }));
}

function settleCache(root, env, live, pre, byName) {
  if (!pre) {
    invalidateRows(byName, SUBTASKS, 'candidate could not be fingerprinted; rerun required');
    return false;
  }
  const names = Object.keys(CACHEABLE_PLANS);
  let post;
  try {
    post = fingerprintProbes(names, root, env);
  } catch {
    invalidateRows(byName, SUBTASKS, 'candidate could not be re-fingerprinted; rerun required');
    return false;
  }
  if (names.some((name) => pre.get(name) !== post.get(name))) {
    invalidateRows(byName, SUBTASKS, 'candidate changed during advisory run; rerun required');
    return false;
  }
  if (!live.some((name) => CACHEABLE_PLANS[name])) return true;
  return saveRows(root, cacheEntries(post, byName));
}

export async function collectAdvisory(options = {}) {
  const root = options.root ?? ROOT;
  const env = options.env ?? process.env;
  const prepared = cacheDisabled(env)
    ? { cached: new Map(), fingerprints: null } : prepareCache(root, env);
  const live = SUBTASKS.filter((name) => !prepared.cached.has(name));
  const rows = await runSubtasks(live, options);
  const byName = new Map(prepared.cached);
  for (const row of rows) byName.set(row.criterion, row);
  if (!cacheDisabled(env)) settleCache(root, env, live, prepared.fingerprints, byName);
  return assembleResults(byName);
}
