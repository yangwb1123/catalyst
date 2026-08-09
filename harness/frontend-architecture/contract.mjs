import { readFileSync } from 'node:fs';
import { isAbsolute, normalize, resolve, sep } from 'node:path';

export const CONTRACT_REF = '.arch/frontend-architecture.v1.json';
export const BASELINE_REF = '.arch/frontend-architecture-baseline.v1.json';
export const WAIVER_REF = '.arch/frontend-architecture-waivers.v1.json';

const CONTRACT_FIELDS = new Set([
  'schema', 'mode', 'targets', 'adapterSupport', 'budgetDefaults',
  'nonWaivableRules', 'waivableRules',
]);
const TARGET_FIELDS = new Set([
  'id', 'root', 'adapter', 'project', 'ownership', 'modules', 'moduleSets',
  'allowedDependencies', 'budgets',
]);
const MODULE_FIELDS = new Set([
  'id', 'root', 'layer', 'publicEntrypoints', 'testEntrypoints',
]);
const MODULE_SET_FIELDS = new Set([
  'idPrefix', 'root', 'layer', 'moduleDepth', 'publicEntrypoints', 'testEntrypoints',
]);
const BUDGET_FIELDS = new Set([
  'enforcement', 'filesPerModule', 'publicSymbols', 'directModuleDependencies',
  'directoryDepth', 'godSignalFamilies', 'physicalLines', 'topLevelDeclarations',
  'imports', 'exports', 'stateCalls', 'effectCalls', 'eventHandlers', 'branchPoints',
]);
const WAIVER_FIELDS = new Set([
  'id', 'ruleId', 'findingFingerprint', 'source', 'target', 'reason',
  'riskOwner', 'approver', 'compensatingProofs', 'debtLink', 'createdAt',
  'expiresAt', 'removalTrigger', 'maxOccurrences',
]);
const BASELINE_FIELDS = new Set([
  'findingFingerprint', 'ruleId', 'owner', 'debtLink', 'firstSeenRevision',
  'removalTrigger',
]);
const ADAPTERS = new Set(['typescript', 'vue', 'dart']);
const OWNERSHIP = new Set(['complete', 'partial']);
const ENFORCEMENT = new Set(['review', 'hard']);
const REQUIRED_NON_WAIVABLE = new Set([
  'FEARCH-CONFIG-001', 'FEARCH-OWNERSHIP-001',
  'FEARCH-DIRECTION-001', 'FEARCH-CYCLE-001',
]);
const ALLOWED_WAIVABLE = new Set(['FEARCH-PUBLIC-001', 'FEARCH-BUDGET-001']);
const MAX_WAIVER_DURATION_MS = 90 * 24 * 60 * 60 * 1000;

export function loadArchitectureBundle(repoRoot) {
  const issues = [];
  const contract = readJson(repoRoot, CONTRACT_REF, issues);
  const baseline = readJson(repoRoot, BASELINE_REF, issues);
  const waivers = readJson(repoRoot, WAIVER_REF, issues);
  if (issues.length === 0) {
    issues.push(...validateContract(contract));
    issues.push(...validateBaseline(baseline, contract));
    issues.push(...validateWaivers(waivers, contract));
  }
  return { contract, baseline, waivers, issues };
}

function readJson(repoRoot, relative, issues) {
  try {
    return JSON.parse(readFileSync(resolve(repoRoot, relative), 'utf8'));
  } catch (error) {
    issues.push(`${relative}: invalid or unavailable JSON (${oneLine(error.message)})`);
    return null;
  }
}

export function validateContract(value) {
  const issues = [];
  if (!isRecord(value)) return [`${CONTRACT_REF}: expected an object`];
  exactFields(value, CONTRACT_FIELDS, CONTRACT_REF, issues);
  expect(value.schema === 'forgeos.frontend-architecture/v1', `${CONTRACT_REF}: invalid schema`, issues);
  expect(value.mode === 'shadow', `${CONTRACT_REF}: v1 mode must remain shadow`, issues);
  expect(Array.isArray(value.targets), `${CONTRACT_REF}: targets must be an array`, issues);
  expect(isRecord(value.adapterSupport), `${CONTRACT_REF}: adapterSupport must be an object`, issues);
  expect(isRecord(value.budgetDefaults), `${CONTRACT_REF}: budgetDefaults must be an object`, issues);
  validateBudgets(value.budgetDefaults, `${CONTRACT_REF}: budgetDefaults`, issues);
  stringSet(value.nonWaivableRules, `${CONTRACT_REF}: nonWaivableRules`, issues);
  stringSet(value.waivableRules, `${CONTRACT_REF}: waivableRules`, issues);
  for (const rule of REQUIRED_NON_WAIVABLE) {
    expect(value.nonWaivableRules?.includes(rule), `${CONTRACT_REF}: required non-waivable rule ${rule} is missing`, issues);
  }
  for (const rule of value.nonWaivableRules ?? []) {
    expect(REQUIRED_NON_WAIVABLE.has(rule), `${CONTRACT_REF}: unknown non-waivable rule ${rule}`, issues);
  }
  for (const rule of value.waivableRules ?? []) {
    expect(ALLOWED_WAIVABLE.has(rule), `${CONTRACT_REF}: rule ${rule} is not allowed to be waivable`, issues);
  }
  const overlap = new Set(value.nonWaivableRules ?? []);
  for (const rule of value.waivableRules ?? []) {
    if (overlap.has(rule)) issues.push(`${CONTRACT_REF}: rule ${rule} cannot be both waivable and non-waivable`);
  }
  const ids = new Set();
  const roots = new Set();
  for (const target of value.targets ?? []) validateTarget(target, ids, roots, issues);
  return issues;
}

function validateTarget(target, ids, roots, issues) {
  const label = `${CONTRACT_REF}: target`;
  if (!isRecord(target)) {
    issues.push(`${label} must be an object`);
    return;
  }
  exactFields(target, TARGET_FIELDS, label, issues);
  idField(target.id, `${label}.id`, ids, issues);
  safeRelative(target.root, `${label} ${target.id}.root`, issues);
  safeRelative(target.project, `${label} ${target.id}.project`, issues);
  if (typeof target.root === 'string' && roots.has(target.root)) issues.push(`${label}: duplicate root ${target.root}`);
  roots.add(target.root);
  expect(ADAPTERS.has(target.adapter), `${label} ${target.id}: unsupported adapter`, issues);
  expect(OWNERSHIP.has(target.ownership), `${label} ${target.id}: invalid ownership`, issues);
  expect(Array.isArray(target.modules), `${label} ${target.id}: modules must be an array`, issues);
  expect(Array.isArray(target.moduleSets), `${label} ${target.id}: moduleSets must be an array`, issues);
  expect(
    (target.modules?.length ?? 0) + (target.moduleSets?.length ?? 0) > 0,
    `${label} ${target.id}: at least one module or moduleSet is required`, issues,
  );
  const moduleIds = new Set();
  for (const module of target.modules ?? []) validateModule(module, moduleIds, `${label} ${target.id}`, issues);
  for (const set of target.moduleSets ?? []) validateModuleSet(set, moduleIds, `${label} ${target.id}`, issues);
  const matrixLayers = validateDependencyMatrix(target.allowedDependencies, `${label} ${target.id}`, issues);
  const declaredLayers = new Set([
    ...(target.modules ?? []).map((module) => module?.layer),
    ...(target.moduleSets ?? []).map((set) => set?.layer),
  ].filter((layer) => typeof layer === 'string' && layer.length > 0));
  for (const layer of declaredLayers) {
    expect(matrixLayers.has(layer), `${label} ${target.id}: layer ${layer} is missing from allowedDependencies`, issues);
  }
  if (target.budgets !== undefined) validateBudgets(target.budgets, `${label} ${target.id}.budgets`, issues);
}

function validateModule(module, ids, label, issues) {
  if (!isRecord(module)) {
    issues.push(`${label}: module must be an object`);
    return;
  }
  exactFields(module, MODULE_FIELDS, `${label}: module`, issues);
  idField(module.id, `${label}: module.id`, ids, issues);
  safeRelative(module.root, `${label}: module ${module.id}.root`, issues);
  nonEmpty(module.layer, `${label}: module ${module.id}.layer`, issues);
  entrypoints(module, `${label}: module ${module.id}`, issues);
}

function validateModuleSet(set, ids, label, issues) {
  if (!isRecord(set)) {
    issues.push(`${label}: moduleSet must be an object`);
    return;
  }
  exactFields(set, MODULE_SET_FIELDS, `${label}: moduleSet`, issues);
  idField(set.idPrefix, `${label}: moduleSet.idPrefix`, ids, issues);
  safeRelative(set.root, `${label}: moduleSet ${set.idPrefix}.root`, issues);
  nonEmpty(set.layer, `${label}: moduleSet ${set.idPrefix}.layer`, issues);
  expect(set.moduleDepth === 1, `${label}: moduleSet ${set.idPrefix}.moduleDepth must be 1 in v1`, issues);
  entrypoints(set, `${label}: moduleSet ${set.idPrefix}`, issues);
}

function entrypoints(item, label, issues) {
  stringSet(item.publicEntrypoints, `${label}.publicEntrypoints`, issues, true);
  stringSet(item.testEntrypoints, `${label}.testEntrypoints`, issues);
  for (const path of [...(item.publicEntrypoints ?? []), ...(item.testEntrypoints ?? [])]) {
    safeRelative(path, `${label} entrypoint`, issues);
  }
}

function validateDependencyMatrix(matrix, label, issues) {
  if (!isRecord(matrix) || Object.keys(matrix).length === 0) {
    issues.push(`${label}: allowedDependencies must be a non-empty object`);
    return new Set();
  }
  const layers = new Set(Object.keys(matrix));
  for (const [from, allowed] of Object.entries(matrix)) {
    stringSet(allowed, `${label}: allowedDependencies.${from}`, issues);
    for (const to of allowed ?? []) if (!layers.has(to)) issues.push(`${label}: ${from} allows unknown layer ${to}`);
  }
  return layers;
}

function validateBudgets(value, label, issues) {
  if (!isRecord(value)) {
    issues.push(`${label} must be an object`);
    return;
  }
  exactFields(value, BUDGET_FIELDS, label, issues, true);
  if (value.enforcement !== undefined) expect(ENFORCEMENT.has(value.enforcement), `${label}.enforcement is invalid`, issues);
  for (const [key, number] of Object.entries(value)) {
    if (key === 'enforcement') continue;
    expect(Number.isInteger(number) && number > 0, `${label}.${key} must be a positive integer`, issues);
  }
}

export function validateBaseline(value, contract) {
  const issues = [];
  if (!isRecord(value) || value.schema !== 'forgeos.frontend-architecture-baseline/v1' || !Array.isArray(value.findings)) {
    return [`${BASELINE_REF}: invalid schema or findings`];
  }
  const fingerprints = new Set();
  const knownRules = new Set([
    ...(contract?.nonWaivableRules ?? []), ...(contract?.waivableRules ?? []),
  ]);
  const nonWaivable = new Set(contract?.nonWaivableRules ?? []);
  for (const item of value.findings) {
    const label = `${BASELINE_REF}: finding`;
    if (!isRecord(item)) { issues.push(`${label} must be an object`); continue; }
    exactFields(item, BASELINE_FIELDS, label, issues);
    fingerprint(item.findingFingerprint, label, fingerprints, issues);
    for (const field of ['ruleId', 'owner', 'debtLink', 'firstSeenRevision', 'removalTrigger']) nonEmpty(item[field], `${label}.${field}`, issues);
    expect(knownRules.has(item.ruleId), `${label}: rule ${item.ruleId} is not declared by the contract`, issues);
    expect(!nonWaivable.has(item.ruleId), `${label}: non-waivable rule ${item.ruleId} cannot be baselined`, issues);
  }
  return issues;
}

export function validateWaivers(value, contract, now = new Date()) {
  const issues = [];
  if (!isRecord(value) || value.schema !== 'forgeos.frontend-architecture-waivers/v1' || !Array.isArray(value.waivers)) {
    return [`${WAIVER_REF}: invalid schema or waivers`];
  }
  const ids = new Set();
  const fingerprints = new Set();
  const allowed = new Set(contract?.waivableRules ?? []);
  for (const item of value.waivers) validateWaiver(item, ids, fingerprints, allowed, now, issues);
  return issues;
}

function validateWaiver(item, ids, fingerprints, allowed, now, issues) {
  const label = `${WAIVER_REF}: waiver`;
  if (!isRecord(item)) { issues.push(`${label} must be an object`); return; }
  exactFields(item, WAIVER_FIELDS, label, issues);
  idField(item.id, `${label}.id`, ids, issues);
  fingerprint(item.findingFingerprint, label, fingerprints, issues);
  expect(allowed.has(item.ruleId), `${label} ${item.id}: rule is not waivable`, issues);
  for (const field of ['source', 'target']) {
    safeRelative(item[field], `${label} ${item.id}.${field}`, issues);
    expect(!String(item[field] ?? '').includes('*'), `${label} ${item.id}: wildcards are forbidden`, issues);
  }
  for (const field of ['reason', 'riskOwner', 'approver', 'debtLink', 'createdAt', 'expiresAt', 'removalTrigger']) nonEmpty(item[field], `${label} ${item.id}.${field}`, issues);
  expect(item.riskOwner !== item.approver, `${label} ${item.id}: self-approval is forbidden`, issues);
  stringSet(item.compensatingProofs, `${label} ${item.id}.compensatingProofs`, issues, true);
  expect(Number.isInteger(item.maxOccurrences) && item.maxOccurrences > 0, `${label} ${item.id}.maxOccurrences must be positive`, issues);
  const expiry = Date.parse(item.expiresAt);
  const created = Date.parse(item.createdAt);
  expect(Number.isFinite(created), `${label} ${item.id}: invalid createdAt`, issues);
  expect(Number.isFinite(expiry), `${label} ${item.id}: invalid expiresAt`, issues);
  if (Number.isFinite(created)) {
    expect(created <= now.getTime(), `${label} ${item.id}: createdAt cannot be in the future`, issues);
  }
  if (Number.isFinite(created) && Number.isFinite(expiry)) {
    expect(expiry > created, `${label} ${item.id}: expiresAt must be after createdAt`, issues);
    expect(expiry - created <= MAX_WAIVER_DURATION_MS, `${label} ${item.id}: waiver duration exceeds 90 days`, issues);
  }
  if (Number.isFinite(expiry) && expiry < now.getTime()) issues.push(`${label} ${item.id}: waiver is expired`);
}

export function mergeBudgets(defaults, target) {
  return { ...defaults, ...(target?.budgets ?? {}) };
}

export function withinRoot(repoRoot, relative) {
  const root = resolve(repoRoot);
  const target = resolve(root, relative);
  return target === root || target.startsWith(root + sep);
}

function exactFields(value, allowed, label, issues, allowPartial = false) {
  for (const key of Object.keys(value)) if (!allowed.has(key)) issues.push(`${label}: unknown field ${key}`);
  if (!allowPartial) for (const key of allowed) if (!(key in value)) issues.push(`${label}: missing field ${key}`);
}

function idField(value, label, seen, issues) {
  nonEmpty(value, label, issues);
  if (typeof value === 'string' && !/^[a-z][a-z0-9_.-]*$/.test(value)) issues.push(`${label} has invalid format`);
  if (seen.has(value)) issues.push(`${label} is duplicate: ${value}`);
  seen.add(value);
}

function fingerprint(value, label, seen, issues) {
  expect(/^sha256:[a-f0-9]{64}$/.test(String(value ?? '')), `${label}: invalid findingFingerprint`, issues);
  if (seen.has(value)) issues.push(`${label}: duplicate findingFingerprint`);
  seen.add(value);
}

function safeRelative(value, label, issues) {
  nonEmpty(value, label, issues);
  if (typeof value !== 'string') return;
  const portable = value.replaceAll('\\', '/');
  expect(!isAbsolute(value) && !portable.startsWith('/') && !portable.split('/').includes('..'), `${label} must be repository-relative without '..'`, issues);
  expect(normalize(value) !== '.', `${label} cannot resolve to repository root`, issues);
}

function stringSet(value, label, issues, requireNonEmpty = false) {
  expect(Array.isArray(value), `${label} must be an array`, issues);
  if (!Array.isArray(value)) return;
  if (requireNonEmpty) expect(value.length > 0, `${label} must not be empty`, issues);
  expect(value.every((item) => typeof item === 'string' && item.length > 0), `${label} must contain non-empty strings`, issues);
  expect(new Set(value).size === value.length, `${label} must not contain duplicates`, issues);
}

function nonEmpty(value, label, issues) {
  expect(typeof value === 'string' && value.trim().length > 0, `${label} must be a non-empty string`, issues);
}

function expect(condition, message, issues) {
  if (!condition) issues.push(message);
}

function isRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function oneLine(value) {
  return String(value).replace(/\s+/g, ' ').trim();
}
