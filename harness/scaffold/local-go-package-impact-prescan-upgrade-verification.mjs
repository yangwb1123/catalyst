// ADR-0062 legacy-upgrade assertions kept outside the near-cap test orchestrator.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const ADR = join('docs', 'adr', '0062-local-go-package-impact-prescan-v1.md');
const SCHEMA = join('docs', 'contracts', 'local-go-package-impact-prescan-v1.schema.json');
const FIXTURE = join('docs', 'contracts', 'fixtures', 'local-go-package-impact-prescan-v1.json');
const CHECKER = join('harness', 'local_go_package_impact_prescan_contract_check.py');
const BOUNDS_TEST = join('harness', 'test_local_go_package_impact_prescan_bounds.py');
const SELF_TEST = join('harness', 'test_local_go_package_impact_prescan_contract_check.py');
const REGISTRY_TEST = join('harness', 'test_local_go_package_impact_prescan_registry.py');
const PACKAGE = join('harness', 'local_go_package_impact_prescan_contract');

export const LOCAL_GO_PACKAGE_IMPACT_PRESCAN_LEGACY_FILES = [
  ADR, SCHEMA, FIXTURE, CHECKER, BOUNDS_TEST, SELF_TEST, REGISTRY_TEST, PACKAGE,
];

export function assertLocalGoPackageImpactPrescanUpgrade(target) {
  const files = [
    ADR, SCHEMA, FIXTURE, CHECKER, BOUNDS_TEST, SELF_TEST, REGISTRY_TEST,
    join(PACKAGE, '__init__.py'),
    join(PACKAGE, 'codec.py'),
    join(PACKAGE, 'constants.py'),
    join(PACKAGE, 'derive.py'),
    join(PACKAGE, 'fixture.py'),
    join(PACKAGE, 'graph.py'),
    join(PACKAGE, 'profiles.py'),
    join(PACKAGE, 'validation.py'),
  ];
  for (const relative of files) assert.equal(existsSync(join(target, relative)), true);
  const state = JSON.parse(readFileSync(join(target, '.agent', 'scaffold-state.json'), 'utf8'));
  for (const relative of files) assert.equal(state.copied.includes(relative), true);
  const tests = spawnSync('python3', ['-B', '-m', 'unittest', SELF_TEST, BOUNDS_TEST], {
    cwd: target, encoding: 'utf8',
  });
  assert.equal(tests.status, 0,
    `upgraded ADR-0062 self-test must pass\n${tests.stdout}\n${tests.stderr}`);
  const registry = spawnSync('python3', ['-B', REGISTRY_TEST], {
    cwd: target, encoding: 'utf8',
  });
  assert.equal(registry.status, 0,
    `upgraded ADR-0062 registry test must pass\n${registry.stdout}\n${registry.stderr}`);
  const check = spawnSync('python3', ['-B', CHECKER, '--golden', '.'], {
    cwd: target, encoding: 'utf8',
  });
  assert.equal(check.status, 0,
    `upgraded ADR-0062 checker must pass\n${check.stdout}\n${check.stderr}`);
  assert.match(check.stdout, /system impact unknown; no authority/);
  assert.equal(existsSync(join(target, 'forge-core')), false);
  assert.equal(existsSync(join(target, 'forge-runtime')), false);
  assert.equal(existsSync(join(target, 'forge-kernel')), false);
}
