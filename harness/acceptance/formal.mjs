// Formal source-candidate coordinator. Its parent injects candidate, kernel,
// and scheduler operations so this package stays below the live coordinator
// without acquiring an advisory-cache dependency or an import cycle.
import { createCandidateJournal } from './candidate-journal.mjs';

function failureRows(detail, api) {
  return api.parallelSchemaOrder.map((criterion) => api.withCategory({
    criterion, status: api.FAIL, detail,
  }));
}

function journalDetail(observation) {
  if (observation.error) return `candidate journal failed: ${observation.error.message}`;
  const listed = observation.events.slice(0, 12).map((path) => `event: ${path}`);
  return listed.length > 0 ? ` (${listed.join('; ')})` : '';
}

async function stableBarrier(journal) {
  await journal.barrier();
  const observation = journal.drift();
  if (observation.error || observation.events.length > 0) {
    throw new Error(`candidate changed during formal acceptance; rerun required${
      journalDetail(observation)
    }`);
  }
}

function diagnosticInventory(root, env, api) {
  if (env.FORGE_CANDIDATE_DIFF !== '1') return null;
  try { return api.candidateInventory(root, env); }
  catch { return null; }
}

function driftDetail(root, env, beforeInventory, observation, api) {
  let detail = `candidate changed during formal acceptance; rerun required${
    journalDetail(observation)
  }`;
  if (beforeInventory === null) return detail;
  try {
    const afterInventory = api.candidateInventory(root, env);
    const diffs = api.candidateInventoryDiff(beforeInventory, afterInventory);
    if (diffs.length > 0) detail += ` (${diffs.slice(0, 12).join('; ')})`;
  } catch { /* diagnostics are best-effort */ }
  return detail;
}

export async function collectFormal(options, api) {
  const root = options.root ?? api.ROOT;
  const env = options.env ?? process.env;
  let journal;
  try { journal = createCandidateJournal(root); }
  catch (error) {
    return failureRows(`candidate journal unavailable: ${error.message}`, api);
  }
  try {
    const before = api.candidateFingerprint(root, env);
    await stableBarrier(journal);
    const beforeInventory = diagnosticInventory(root, env, api);
    await stableBarrier(journal);
    const rows = await api.runSubtasks(api.SUBTASKS, options);
    const results = api.assembleResults(new Map(rows.map((row) => [row.criterion, row])));
    await journal.barrier();
    const after = api.candidateFingerprint(root, env);
    await journal.barrier();
    const observation = journal.drift();
    if (before === after && !observation.error && observation.events.length === 0) {
      return results;
    }
    return failureRows(driftDetail(root, env, beforeInventory, observation, api), api);
  } catch (error) {
    const detail = error.message.startsWith('candidate changed')
      ? error.message : `candidate stability check failed: ${error.message}`;
    return failureRows(detail, api);
  } finally {
    try { await journal.close(); }
    catch { /* completed barriers already bound the interval */ }
  }
}
