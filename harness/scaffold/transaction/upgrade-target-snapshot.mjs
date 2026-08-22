// Binds transaction reads to the inode, mode, and bytes seen at classification.
function sameIdentity(left, right) {
  return left.dev === right.dev && left.ino === right.ino;
}

function changedPathError(record) {
  return new Error(
    `refusing changed file path for ${record.kind} target ${record.rel}: `
      + 'destination changed after classification',
  );
}

export function requireClassifiedSnapshot(snapshots, record, current) {
  const expected = snapshots.get(record.rel);
  if (expected === undefined
      || !sameIdentity(current.identity, expected.identity)
      || current.mode !== expected.mode
      || !current.bytes.equals(expected.bytes)) {
    throw changedPathError(record);
  }
  return expected;
}
