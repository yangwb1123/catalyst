// ADR-0071 portable Context Engineering package; no Catalyst Go/Rust runtime.
import { join } from 'node:path';

const VENDOR = [
  '__init__.py', 'assembler.py', 'codec.py', 'constants.py', 'shape.py',
  'token_counter.py',
].map((name) => join(
  'skills', 'context-engineering', 'scripts', '_vendor',
  'context_package_contract', name,
));

export const CONTEXT_ENGINEERING_COPIED_FILES = [
  join('docs', 'adr', 'ADR-0071-portable-context-engineering-skill.md'),
  join('skills', 'context-engineering', 'SKILL.md'),
  join('skills', 'context-engineering', 'agents', 'openai.yaml'),
  join('skills', 'context-engineering', 'references', 'contract.md'),
  join('skills', 'context-engineering', 'references', 'evals.json'),
  join('skills', 'context-engineering', 'references', 'fixtures',
    'context-package-v1.json'),
  join('skills', 'context-engineering', 'references', 'package-manifest.json'),
  join('skills', 'context-engineering', 'scripts', 'assemble.py'),
  join('skills', 'context-engineering', 'scripts', 'check_package.py'),
  join('skills', 'context-engineering', 'scripts', '_vendor', '__init__.py'),
  ...VENDOR,
  join('skills', 'context-engineering', 'tests', 'test_portable_scripts.py'),
];

export const CONTEXT_ENGINEERING_EXPECTED_FILES = CONTEXT_ENGINEERING_COPIED_FILES;
