// One live acceptance probe per process. The coordinator owns ordering, cache
// decisions, limits, and categories; this worker emits exactly one JSON row and
// never calls process.exit(), allowing stdout/stderr to drain before `close`.
import { pathToFileURL } from 'node:url';

import {
  nodeTestRow, probeAppTests, probeArch, probeArchitecture, probeBuild,
  probeComplexity, probeProjectTests, probeSCA, probeSecurity, probeTypecheck,
  pythonTestRowParallel,
} from './acceptance.mjs';
import { probeCoverage, probeLint } from './acceptance-quality.mjs';

const MAX_DETAIL_BYTES = 256 * 1024;

function projectTestRow() {
  return { ...probeProjectTests(), criterion: 'test_pass_project' };
}

const PROBES = {
  test_pass_node: nodeTestRow,
  test_pass_python: pythonTestRowParallel,
  test_pass_project: projectTestRow,
  app_test_pass: probeAppTests,
  complexity_violations: probeComplexity,
  arch_violations: probeArch,
  architecture: probeArchitecture,
  security_findings: probeSecurity,
  dependency_vulnerabilities: probeSCA,
  lint: probeLint,
  coverage: probeCoverage,
  typecheck: probeTypecheck,
  build: probeBuild,
};

function writeOutput(stream, text) {
  return new Promise((resolve) => stream.write(text, resolve));
}

function boundedRow(name, row) {
  if (!row || typeof row.detail !== 'string'
      || Buffer.byteLength(row.detail) > MAX_DETAIL_BYTES) {
    return {
      criterion: name, status: 'FAIL',
      detail: `${name}: probe detail exceeded the worker serialization bound`,
    };
  }
  return row;
}

export async function runNamedProbe(name) {
  const probe = PROBES[name];
  if (!probe) {
    await writeOutput(process.stderr, `acceptance-worker: unknown probe ${name}\n`);
    return 2;
  }
  try {
    await writeOutput(process.stdout, JSON.stringify(boundedRow(name, await probe())));
    return 0;
  } catch (error) {
    await writeOutput(
      process.stderr,
      `acceptance-worker: probe ${name} crashed: ${error?.stack ?? error}\n`,
    );
    return 1;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  runNamedProbe(process.argv[2]).then((code) => { process.exitCode = code; });
}
