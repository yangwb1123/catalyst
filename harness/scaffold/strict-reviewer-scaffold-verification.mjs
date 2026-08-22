// ADR-0063/0064 fresh and legacy-upgrade assertions kept outside near-cap orchestrators.
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const ADR = join('docs', 'adr', '0063-l3-l4-build-reviewer-strict-verdict-v1.md');
const BINDING_ADR = join('docs', 'adr', '0064-local-digest-bound-agent-output-review-approval.md');
const BUILD = join('.agent', 'workflows', 'build.yml');
const REVIEWER = join('.agent', 'agents', 'reviewer.md');
const WORKFLOWS = [
  'discover', 'design', 'review', 'build', 'deploy', 'rollback', 'evolve',
].map((name) => join('.agent', 'workflows', `${name}.yml`));

export const STRICT_REVIEWER_LEGACY_FILES = [ADR, BINDING_ADR, ...WORKFLOWS, REVIEWER];

export function assertStrictReviewerScaffold(target) {
  for (const relative of STRICT_REVIEWER_LEGACY_FILES) {
    assert.equal(existsSync(join(target, relative)), true, `missing output-binding asset: ${relative}`);
  }
  const state = JSON.parse(readFileSync(join(target, '.agent', 'scaffold-state.json'), 'utf8'));
  for (const relative of STRICT_REVIEWER_LEGACY_FILES) {
    assert.equal(state.copied.includes(relative), true, `${relative} must enter upgrade ledger`);
  }

  for (const workflow of WORKFLOWS) {
    const source = readFileSync(join(target, workflow), 'utf8');
    const selectors = [...source.matchAll(/^output_binding_contract:\s*local_digest_v1\s*(?:#.*)?$/gm)];
    assert.equal(selectors.length, 1, `${workflow} must carry exactly one local_digest_v1 selector`);
  }

  const build = readFileSync(join(target, BUILD), 'utf8');
  const contractLines = [...build.matchAll(/^\s*verdict_contract:\s*reviewer_v2\s*(?:#.*)?$/gm)];
  assert.equal(contractLines.length, 1, 'Build must carry exactly one reviewer_v2 declaration');
  assert.match(build, /reviewer_v2 binds[\s\S]*L3\/L4 decisions to this attempt/);
  assert.match(build, /L0–L2\/unbound retain advisory behavior/);

  const reviewer = readFileSync(join(target, REVIEWER), 'utf8');
  assert.match(reviewer, /L3\/L4[\s\S]*fail-closed/);
  assert.match(reviewer, /L0–L2[\s\S]*materiality_not_bound[\s\S]*fail-open/);
  assert.match(reviewer, /未经认证的 caller declaration/);
  assert.match(reviewer, /身份[\s\S]*review 质量[\s\S]*cryptographic SoD/);
  assert.match(reviewer, /source\/prompt\/policy\/declared-artifact digests/);

  const adr = readFileSync(join(target, ADR), 'utf8');
  assert.match(adr, /caller-declared[\s\S]*materiality/);
  assert.match(adr, /旧 runtime[\s\S]*unknown contract[\s\S]*拒绝/);
  assert.match(adr, /crash recovery consistency/);
  assert.match(adr, /same-UID/);
  assert.match(adr, /不提供[\s\S]*身份认证[\s\S]*cryptographic separation of duties/);
  assert.match(adr, /source revision、diff、ContextPackage、policy、prompt、model、artifact/);

  const bindingAdr = readFileSync(join(target, BINDING_ADR), 'utf8');
  assert.match(bindingAdr, /已采纳并交付/);
  assert.match(bindingAdr, /七个 shipped canonical workflow/);
  assert.match(bindingAdr, /同时关闭 A–D/);

  assert.equal(existsSync(join(target, 'forge-core')), false,
    'scaffold must not install the Catalyst-only Go runtime');
  assert.equal(existsSync(join(target, 'forge-runtime')), false,
    'scaffold must not install the Rust runtime');
  assert.equal(existsSync(join(target, 'forge-kernel')), false,
    'scaffold must not install Kernel, roots, keys, or state');
  assert.equal(existsSync(join(target, 'forge')), false,
    'scaffold must not install or replace the host forge executable');
}
