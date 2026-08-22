// Orders every published destination and stage directory before the durable
// transaction commit marker. The marker is valid only after these fsyncs return.
import { fsyncSync } from 'node:fs';

function syncDirectory(fd) {
  fsyncSync(fd);
}

export function syncUpgradePublication(boundaries, claims) {
  const descriptors = new Set();
  for (const boundary of boundaries) {
    const parent = boundary?.components?.at(-1);
    if (parent !== undefined) descriptors.add(parent.fd);
  }
  for (const claim of claims) {
    if (claim?.directoryFd !== undefined) descriptors.add(claim.directoryFd);
  }
  for (const fd of descriptors) syncDirectory(fd);
}
