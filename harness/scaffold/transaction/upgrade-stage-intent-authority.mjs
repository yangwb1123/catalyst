// Append-only pre-content authority bound to the durable transaction journal inode.
import {
  closeSync, constants, fstatSync, fsyncSync, lstatSync, openSync, writeFileSync,
} from 'node:fs';

const NOFOLLOW = constants.O_NOFOLLOW;
const NONBLOCK = constants.O_NONBLOCK ?? 0;
const MAX_JOURNAL_BYTES = 1024 * 1024;
const INTENT_API = 'forgeos.scaffold-upgrade-stage-intent/v1';
const MAX_STAT_DECIMAL = '18446744073709551615';

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

function identityValue(value, label) {
  if (!exactKeys(value, ['dev', 'ino'])
      || !/^(0|[1-9]\d*)$/.test(value.dev ?? '')
      || !/^[1-9]\d*$/.test(value.ino ?? '')) {
    throw new Error(`malformed ${label} identity`);
  }
  return Object.freeze({ dev: BigInt(value.dev), ino: BigInt(value.ino) });
}

function requireJournal(stat, expected, label) {
  if (!stat.isFile() || stat.isSymbolicLink() || Number(stat.nlink) !== 1
      || Number(stat.mode & 0o777n) !== 0o600
      || Number(stat.size) > MAX_JOURNAL_BYTES
      || !sameIdentity(stat, expected)) throw new Error(`unsafe ${label}`);
  return stat;
}

function intentDocument(transaction, intent) {
  return {
    api_version: INTENT_API,
    directory: identityDocument(intent.directoryIdentity),
    next: { mode: intent.nextMode, sha256: intent.nextSha256 },
    stage_name: intent.stageName,
    transaction_id: transaction.id,
  };
}

export function encodeUpgradeJournal(document) {
  return Buffer.from(`${JSON.stringify(document)}\n`);
}

function maximumIntentDocument(document, entry) {
  return {
    api_version: INTENT_API,
    directory: { dev: MAX_STAT_DECIMAL, ino: MAX_STAT_DECIMAL },
    next: { mode: 0o777, sha256: 'f'.repeat(64) },
    stage_name: entry.stage_name,
    transaction_id: document.transaction_id,
  };
}

export function assertUpgradeJournalCapacity(document) {
  let bytes = encodeUpgradeJournal(document).length;
  for (const entry of document.entries) {
    bytes += Buffer.byteLength(`${JSON.stringify(maximumIntentDocument(document, entry))}\n`);
    if (bytes > MAX_JOURNAL_BYTES) {
      throw new Error('scaffold upgrade plan exceeds journal intent capacity');
    }
  }
  return bytes;
}

export function appendUpgradeStageIntent(transaction, intent) {
  const index = transaction.stageIntents.size;
  const expectedEntry = transaction.document.entries[index];
  if (expectedEntry?.stage_name !== intent.stageName
      || transaction.stageIntents.has(intent.stageName)) {
    throw new Error('out-of-order scaffold upgrade stage intent');
  }
  const expected = transaction.control.identities.get('journal');
  const path = transaction.control.paths.journal;
  let fd;
  try {
    fd = openSync(path, constants.O_WRONLY | constants.O_APPEND | NOFOLLOW | NONBLOCK);
    const opened = requireJournal(fstatSync(fd, { bigint: true }), expected, 'upgrade journal');
    requireJournal(lstatSync(path, { bigint: true }), opened, 'upgrade journal');
    const document = intentDocument(transaction, intent);
    const encoded = Buffer.from(`${JSON.stringify(document)}\n`);
    if (opened.size + BigInt(encoded.length) > BigInt(MAX_JOURNAL_BYTES)) {
      throw new Error('scaffold upgrade stage intents exceed journal limit');
    }
    writeFileSync(fd, encoded);
    fsyncSync(fd);
    requireJournal(fstatSync(fd, { bigint: true }), opened, 'upgrade journal');
    requireJournal(lstatSync(path, { bigint: true }), opened, 'upgrade journal');
    transaction.stageIntents.set(intent.stageName, intent);
    transaction.hooks.afterUpgradeStageIntentWrite?.({ stageName: intent.stageName });
  } finally {
    if (fd !== undefined) closeSync(fd);
  }
}

export function parseUpgradeJournal(bytes) {
  const text = bytes.toString('utf8');
  try { return { document: JSON.parse(text), intentDocuments: [] }; } catch {}
  const completeEnd = text.endsWith('\n') ? text.length : text.lastIndexOf('\n') + 1;
  if (completeEnd === 0) throw new Error('malformed scaffold upgrade transaction journal');
  const incompleteTail = completeEnd !== text.length;
  const lines = text.slice(0, completeEnd - 1).split('\n');
  if ((!incompleteTail && lines.length < 2)
      || lines.some((line) => line.length === 0)) {
    throw new Error('malformed scaffold upgrade transaction journal');
  }
  try {
    return {
      document: JSON.parse(lines[0]),
      intentDocuments: lines.slice(1).map((line) => JSON.parse(line)),
    };
  } catch {
    throw new Error('malformed scaffold upgrade transaction journal');
  }
}

function decodeIntent(value, entry, transactionId) {
  if (!exactKeys(value,
    ['api_version', 'directory', 'next', 'stage_name', 'transaction_id'])
      || value.api_version !== INTENT_API || value.transaction_id !== transactionId
      || value.stage_name !== entry.stage_name
      || !exactKeys(value.next, ['mode', 'sha256'])
      || !Number.isInteger(value.next.mode) || value.next.mode < 0
      || value.next.mode > 0o777 || !/^[a-f0-9]{64}$/.test(value.next.sha256 ?? '')) {
    throw new Error(`malformed stage intent for ${entry.rel}`);
  }
  return Object.freeze({
    directoryIdentity: identityValue(value.directory, `stage intent ${entry.rel}`),
    nextMode: value.next.mode, nextSha256: value.next.sha256,
  });
}

export function decodeUpgradeStageIntents(documents, transaction) {
  if (documents.length > transaction.entries.length) {
    throw new Error('too many scaffold upgrade stage intents');
  }
  return new Map(documents.map((value, index) => {
    const entry = transaction.entries[index];
    return [entry.stage_name, decodeIntent(value, entry, transaction.transaction_id)];
  }));
}
