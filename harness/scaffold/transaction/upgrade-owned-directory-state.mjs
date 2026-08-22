// Stable parent-relative names for restartable owned-directory cleanup.
import { createHash } from 'node:crypto';
import { join } from 'node:path';

function cleanupNames(owner, relativePath) {
  const digest = createHash('sha256').update(`${owner}\0${relativePath}`)
    .digest('hex').slice(0, 16);
  const suffix = `${owner.slice(0, 12)}-${digest}`;
  return {
    proof: `.forge-upgrade-owned-proof-${suffix}`,
    quarantine: `.forge-upgrade-owned-dir-${suffix}`,
  };
}

export function ownedDirectoryCleanupContext(item, owner) {
  const base = join(item.descriptorRoot, String(item.parentFd));
  const names = cleanupNames(owner, item.relative);
  return {
    child: join(base, item.name), descriptorRoot: item.descriptorRoot,
    name: item.name, parentFd: item.parentFd, path: item.path,
    proof: join(base, names.proof), quarantine: join(base, names.quarantine),
    relative: item.relative,
  };
}
