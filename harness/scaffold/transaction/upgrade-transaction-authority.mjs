// Durable cross-file authority for prepared upgrade stages. The journal inode
// binds the prepared manifest, and the started marker binds both control inodes.
import { createHash, randomBytes } from 'node:crypto';
import {
  closeSync, constants, fchmodSync, fstatSync, fsyncSync, linkSync, lstatSync,
  openSync, readdirSync, renameSync, unlinkSync, writeFileSync,
} from 'node:fs';
import { basename, dirname, join } from 'node:path';

const NOFOLLOW = constants.O_NOFOLLOW;
const NONBLOCK = constants.O_NONBLOCK ?? 0;
const MAX_CONTROL_BYTES = 1024 * 1024;

function sameIdentity(left, right) {
  return left.dev === right.dev && left.ino === right.ino;
}

function exactKeys(value, expected) {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    && JSON.stringify(Object.keys(value).sort()) === JSON.stringify(expected);
}

function identityDocument(value) {
  return { dev: String(value.dev), ino: String(value.ino) };
}

function optionalIdentityDocument(value) {
  return value === null || value === undefined ? null : identityDocument(value);
}

function identityValue(value, label) {
  if (!exactKeys(value, ['dev', 'ino'])
      || !/^(0|[1-9]\d*)$/.test(value.dev ?? '')
      || !/^[1-9]\d*$/.test(value.ino ?? '')) {
    throw new Error(`malformed ${label} identity`);
  }
  return Object.freeze({ dev: BigInt(value.dev), ino: BigInt(value.ino) });
}

function digest(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function controlFile(stat, label, expected = null, links = 1) {
  if (!stat.isFile() || stat.isSymbolicLink() || Number(stat.nlink) !== links
      || Number(stat.mode & 0o777n) !== 0o600 || Number(stat.size) > MAX_CONTROL_BYTES
      || (expected !== null && !sameIdentity(stat, expected))) {
    throw new Error(`unsafe ${label}`);
  }
  return stat;
}

function restorePreserved(path, quarantine, label) {
  try { linkSync(quarantine, path); } catch (error) {
    throw new Error(`preserved unknown ${label} at ${quarantine}: ${error.message}`);
  }
  throw new Error(`preserved unknown ${label} at ${path}`);
}

function removePrivateControl(path, expected, label) {
  let fd;
  const quarantine = `${path}.finishing-${randomBytes(16).toString('hex')}`;
  try {
    fd = openSync(path, constants.O_RDONLY | NOFOLLOW | NONBLOCK);
    const opened = fstatSync(fd, { bigint: true });
    if (!opened.isFile() || !sameIdentity(opened, expected)
        || ![1, 2].includes(Number(opened.nlink))) throw new Error(`unsafe ${label}`);
    renameSync(path, quarantine);
    const moved = lstatSync(quarantine, { bigint: true });
    if (!sameIdentity(moved, expected)) restorePreserved(path, quarantine, label);
    unlinkSync(quarantine);
  } finally { if (fd !== undefined) closeSync(fd); }
}

function publishingCandidates(path) {
  const prefix = `${basename(path)}.publishing-`;
  try {
    return readdirSync(dirname(path)).filter((name) => name.startsWith(prefix))
      .map((name) => join(dirname(path), name));
  } catch (error) {
    if (error?.code === 'ENOENT') return [];
    throw error;
  }
}

export function completeControlPublication(path, label, parentFd) {
  let stat;
  try { stat = lstatSync(path, { bigint: true }); } catch (error) {
    if (error?.code === 'ENOENT') return false;
    throw error;
  }
  if (Number(stat.nlink) === 1) return true;
  if (Number(stat.nlink) !== 2) throw new Error(`unsafe ${label}`);
  const matching = publishingCandidates(path).filter((candidate) => {
    try { return sameIdentity(lstatSync(candidate, { bigint: true }), stat); } catch { return false; }
  });
  if (matching.length !== 1) throw new Error(`ambiguous ${label} publication`);
  removePrivateControl(matching[0], stat, label);
  fsyncSync(parentFd);
  controlFile(lstatSync(path, { bigint: true }), label, stat);
  return true;
}

export function publishControlFile(
  path, name, bytes, parentFd, { hooks = {}, onDurable = null } = {},
) {
  const label = `${name} control`;
  const privatePath = `${path}.publishing-${randomBytes(16).toString('hex')}`;
  let fd; let created = null;
  try {
    fd = openSync(
      privatePath,
      constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | NOFOLLOW | NONBLOCK,
      0o600,
    );
    fchmodSync(fd, 0o600);
    created = fstatSync(fd, { bigint: true });
    controlFile(created, label);
    const createdIdentity = Object.freeze({ dev: created.dev, ino: created.ino });
    const content = typeof bytes === 'function' ? bytes(createdIdentity) : bytes;
    writeFileSync(fd, content);
    fsyncSync(fd);
    controlFile(fstatSync(fd, { bigint: true }), label, createdIdentity);
    hooks.afterTransactionControlPrivateSync?.({ name, path });
    linkSync(privatePath, path);
    hooks.afterTransactionControlPublish?.({ name, path });
    removePrivateControl(privatePath, createdIdentity, label);
    controlFile(lstatSync(path, { bigint: true }), label, createdIdentity);
    fsyncSync(parentFd);
    onDurable?.(createdIdentity);
    hooks.afterTransactionControlParentSync?.({ name, path });
    return createdIdentity;
  } catch (error) {
    if (created !== null) {
      try { removePrivateControl(privatePath, created, label); } catch (cleanup) {
        if (cleanup?.code !== 'ENOENT') {
          throw new Error(`${error.message}; private control cleanup failed: ${cleanup.message}`);
        }
      }
    }
    throw error;
  } finally {
    if (fd !== undefined) closeSync(fd);
  }
}

function priorDocument(claim) {
  if (claim.priorControlIdentity === undefined) return null;
  return {
    control: identityDocument(claim.priorControlIdentity),
    file: {
      ...identityDocument(claim.priorIdentity), mode: claim.priorMode,
      sha256: claim.priorSha256,
    },
  };
}

function stageDocument(entry, claim) {
  if (claim === undefined || claim.stageName !== entry.stage_name) {
    throw new Error(`missing prepared stage authority for ${entry.rel}`);
  }
  return {
    claim: identityDocument(claim.controlIdentity),
    directory: identityDocument(claim.directoryIdentity),
    kind: entry.kind,
    next: {
      ...identityDocument(claim.identity), mode: claim.nextMode,
      sha256: claim.nextSha256,
    },
    prior: priorDocument(claim),
    rel: entry.rel,
    stage_name: entry.stage_name,
  };
}

export function preparedAuthorityDocument(transaction, claims) {
  return {
    api_version: 'forgeos.scaffold-upgrade-prepared-authority/v1',
    journal: identityDocument(transaction.control.identities.get('journal')),
    stages: transaction.document.entries.map(
      (entry) => stageDocument(entry, claims.get(entry.stage_name)),
    ),
    transaction_id: transaction.id,
  };
}

export function startedAuthorityDocument(transaction, preparedIdentity) {
  return {
    api_version: 'forgeos.scaffold-upgrade-started-authority/v1',
    journal: identityDocument(transaction.control.identities.get('journal')),
    prepared: identityDocument(preparedIdentity),
    transaction_id: transaction.id,
  };
}

function decodeNext(value, label) {
  if (!exactKeys(value, ['dev', 'ino', 'mode', 'sha256'])
      || !Number.isInteger(value.mode) || value.mode < 0 || value.mode > 0o777
      || !/^[a-f0-9]{64}$/.test(value.sha256 ?? '')) {
    throw new Error(`malformed ${label}`);
  }
  return {
    identity: identityValue({ dev: value.dev, ino: value.ino }, label),
    mode: value.mode, sha256: value.sha256,
  };
}

function decodePrior(value, label) {
  if (value === null) return null;
  if (!exactKeys(value, ['control', 'file'])) throw new Error(`malformed ${label}`);
  const file = decodeNext(value.file, `${label} file`);
  return { ...file, controlIdentity: identityValue(value.control, `${label} control`) };
}

function decodeStage(value, entry) {
  const keys = ['claim', 'directory', 'kind', 'next', 'prior', 'rel', 'stage_name'];
  if (!exactKeys(value, keys) || value.kind !== entry.kind || value.rel !== entry.rel
      || value.stage_name !== entry.stage_name) {
    throw new Error(`malformed prepared stage authority for ${entry.rel}`);
  }
  const next = decodeNext(value.next, `prepared next ${entry.rel}`);
  return Object.freeze({
    controlIdentity: identityValue(value.claim, `prepared claim ${entry.rel}`),
    directoryIdentity: identityValue(value.directory, `prepared directory ${entry.rel}`),
    nextIdentity: next.identity, nextMode: next.mode, nextSha256: next.sha256,
    prior: decodePrior(value.prior, `prepared prior ${entry.rel}`),
  });
}

function parseRecord(record, label) {
  try { return JSON.parse(record.bytes); } catch {
    throw new Error(`malformed scaffold upgrade ${label}`);
  }
}

export function decodePreparedAuthority(record, journalRecord, transaction) {
  const value = parseRecord(record, 'prepared authority');
  if (!exactKeys(value, ['api_version', 'journal', 'stages', 'transaction_id'])
      || value.api_version !== 'forgeos.scaffold-upgrade-prepared-authority/v1'
      || value.transaction_id !== transaction.transaction_id
      || !Array.isArray(value.stages)
      || value.stages.length !== transaction.entries.length
      || !sameIdentity(identityValue(value.journal, 'prepared journal'), journalRecord.identity)) {
    throw new Error('malformed scaffold upgrade prepared authority');
  }
  return new Map(transaction.entries.map(
    (entry, index) => [entry.stage_name, decodeStage(value.stages[index], entry)],
  ));
}

export function decodeStartedAuthority(record, journalRecord, preparedRecord, transactionId) {
  const value = parseRecord(record, 'started authority');
  if (!exactKeys(value, ['api_version', 'journal', 'prepared', 'transaction_id'])
      || value.api_version !== 'forgeos.scaffold-upgrade-started-authority/v1'
      || value.transaction_id !== transactionId
      || !sameIdentity(identityValue(value.journal, 'started journal'), journalRecord.identity)
      || !sameIdentity(identityValue(value.prepared, 'started prepared'), preparedRecord.identity)) {
    throw new Error('malformed scaffold upgrade started authority');
  }
  return value.transaction_id;
}

export function cleanupAuthorityDocument(identities, target, transactionId, proof) {
  const controls = Object.fromEntries(
    ['committed', 'finished', 'journal', 'prepared', 'started'].map(
      (name) => [name, optionalIdentityDocument(identities.get(name))],
    ),
  );
  return {
    api_version: 'forgeos.scaffold-upgrade-control-cleanup/v1',
    controls, proof: identityDocument(proof), target: identityDocument(target),
    transaction_id: transactionId,
  };
}

function optionalIdentityValue(value, label) {
  return value === null ? null : identityValue(value, label);
}

export function decodeCleanupAuthority(bytes, opened, target, authority) {
  let value;
  try { value = JSON.parse(bytes); } catch {
    throw new Error('unsafe cleanup control: malformed JSON');
  }
  const controlKeys = ['committed', 'finished', 'journal', 'prepared', 'started'];
  if (!exactKeys(value, ['api_version', 'controls', 'proof', 'target', 'transaction_id'])
      || value.api_version !== 'forgeos.scaffold-upgrade-control-cleanup/v1'
      || !/^[a-f0-9]{32}$/.test(value.transaction_id ?? '')
      || !exactKeys(value.controls, controlKeys)) {
    throw new Error('unsafe cleanup control: malformed document');
  }
  const proof = identityValue(value.proof, 'cleanup proof');
  const storedTarget = identityValue(value.target, 'cleanup target');
  const controls = Object.fromEntries(Object.entries(value.controls).map(
    ([name, item]) => [name, optionalIdentityValue(item, `${name} cleanup claim`)],
  ));
  if (!sameIdentity(proof, opened) || !sameIdentity(storedTarget, target)
      || controls.finished === null || authority === null
      || value.transaction_id !== authority.transactionId
      || controls.journal === null
      || !sameIdentity(controls.journal, authority.journalIdentity)) {
    throw new Error('unsafe cleanup control: missing independent journal authority');
  }
  for (const [name, expected] of authority.controls) {
    if (name === 'cleanup' || expected === null || expected === undefined) continue;
    if (controls[name] === null || !sameIdentity(controls[name], expected)) {
      throw new Error(`unsafe cleanup control: changed ${name} authority`);
    }
  }
  return { controls, identity: proof, transactionId: value.transaction_id };
}
