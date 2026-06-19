#!/usr/bin/env node
// ForgeOS forge-init (v1) — stamp a NEW project with inheritable, runnable governance.
// One command scaffolds a target dir with: the universal red-lines (.agent/AGENTS.md,
// COPIED verbatim), generated starter docs, project.yml, and the REAL host-independent
// ENFORCERS (COPIED verbatim) so the fresh project enforces governance immediately —
// `gate` + `arch-check` + `secret-scan` all pass out of the box.
//
// What a fresh project INHERITS (the full host-independent enforcement triad — each of
// these tools only SCANS files and resolves paths from its OWN on-disk location, so it
// needs nothing project-specific to run):
//   * file-size + root-count governance  — harness/gate.mjs + harness/policies.yml
//   * architecture enforcement           — harness/arch/arch-check.mjs (+ scan.mjs,
//     scan-functions.mjs) reading .arch/rules.yaml: layering / package / fan-in /
//     cognitive / anti-pattern-naming / function-length / circular / drift-guard.
//     .arch/rules.yaml ships ALONGSIDE policies.yml because arch-check's drift-guard
//     asserts the two agree — they MUST travel together to stay consistent.
//   * hardcoded-secret scanning          — harness/secret-scan.mjs.
//
// HONEST about what is NOT inherited yet (do not pretend a fresh project has these):
//   * check.py (governance-completeness: scans .agent/ agents/skills/workflows/eval)
//     and acceptance.mjs (the aggregate Stop gate, needs .agent/eval/acceptance.schema.yml)
//     are NOT copied — a fresh project's minimal .agent/ (PROJECT/ROADMAP/CURRENT_SPRINT/
//     project.yml only) lacks those structures, so copying them would FAIL on day one.
//     They light up once the project's .agent/ is fleshed out. See the next-steps print.
//
// Usage:
//   node harness/forge-init.mjs <target-dir> --name <project> \
//        [--mode balanced] [--lifecycle mvp] [--force]
//
// Design: PURE templating functions (return strings, unit-testable without disk) are
// kept separate from the fs/copy I/O boundary at the bottom.
import {
  mkdirSync,
  copyFileSync,
  writeFileSync,
  readdirSync,
  existsSync,
  statSync,
} from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// The script's own location locates the ForgeOS SOURCE repo root so we copy the
// REAL tools (dirname(harness/forge-init.mjs) === harness; its parent === repo root).
const HARNESS_DIR = dirname(fileURLToPath(import.meta.url));
const SOURCE_ROOT = dirname(HARNESS_DIR);

// --- pure templating (no disk; unit-testable) --------------------------------

export function renderProjectMd(name) {
  return `# ${name} — Project

## 是什么 (What)
TODO: 一句话说清 ${name} 是什么、解决谁的什么问题。

## 目标 (Goals)
- TODO: G1 —
- TODO: G2 —

## 非目标 (Non-Goals)
- TODO: 明确不做什么(防止上帝项目蔓延)。

由 ForgeOS 治理。阅读顺序见 [.agent/AGENTS.md](AGENTS.md)。
`;
}

export function renderRoadmapMd(name) {
  return `# ${name} — Roadmap

> 纪律:每一版可独立验证;不把 ${name} 自己做成上帝项目。

## v0 — 起步
- [ ] TODO: 第一个最小可验证切片
- [ ] TODO: 第二个最小可验证切片
- [ ] TODO: 跑通 \`node harness/gate.mjs\`(治理闸门全绿)
`;
}

export function renderCurrentSprintMd(name) {
  return `# ${name} — Current Sprint

## Sprint 0 — 起步
- [ ] TODO: 把 ROADMAP v0 的第一项落地

**stop_condition:** roadmap 完成度 / 闸门全绿(非「继续 N 轮」)。
`;
}

export function renderProjectYml(name, mode, lifecycle) {
  return `# ${name} — project config
# 中枢旋钮 mode × lifecycle 驱动严格度与深度。由 ForgeOS forge-init 生成。

extends: []

project: ${name}
mode: ${mode}                 # explorer | balanced | engineering | cto
lifecycle: ${lifecycle}            # idea | mvp | growth | production

overrides:
  max_file_lines: 500         # 对齐 harness/policies.yml(真相之源)
  max_root_files: 15
`;
}

export function renderReadmeMd(name) {
  return `# ${name}

Governed by ForgeOS. Enforce with \`node harness/gate.mjs\`.

设计与决策的事实源在 [.agent/](.agent/)(PROJECT · ROADMAP · CURRENT_SPRINT · AGENTS)。
`;
}

export function renderGitignore() {
  return `node_modules/
__pycache__/
*.pyc
dist/
build/
coverage/
.DS_Store
`;
}

// --- I/O boundary ------------------------------------------------------------

// CLI arg parse: positional <target-dir> + flags. Pure-ish (no disk); returns a
// plain config object or throws on a usage error.
export function parseArgs(argv) {
  const out = { target: null, name: null, mode: 'balanced', lifecycle: 'mvp', force: false };
  const takesValue = { '--name': 'name', '--mode': 'mode', '--lifecycle': 'lifecycle' };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--force') out.force = true;
    else if (a in takesValue) {
      const v = argv[++i];
      if (v === undefined || v.startsWith('--')) {
        throw new Error(`flag ${a} requires a value (got ${v ?? 'nothing'})`);
      }
      out[takesValue[a]] = v;
    } else if (a.startsWith('--')) throw new Error(`unknown flag: ${a}`);
    else if (out.target === null) out.target = a;
    else throw new Error(`unexpected argument: ${a}`);
  }
  if (!out.target) throw new Error('missing <target-dir>');
  if (!out.name) throw new Error('missing --name <project>');
  return out;
}

// SAFETY: refuse to scaffold over a non-empty .agent/ unless force is set.
function assertSafeTarget(targetDir, force) {
  const agentDir = join(targetDir, '.agent');
  if (force || !existsSync(agentDir)) return;
  let entries = [];
  try {
    entries = readdirSync(agentDir);
  } catch {
    return; // unreadable -> not a populated project we'd clobber
  }
  if (entries.length > 0) {
    throw new Error(
      `${agentDir} already exists and is non-empty; pass --force to overwrite`,
    );
  }
}

// Copy a file from the SOURCE repo into the target, creating parent dirs.
function copyFromSource(relPath, targetDir, created) {
  const dest = join(targetDir, relPath);
  mkdirSync(dirname(dest), { recursive: true });
  copyFileSync(join(SOURCE_ROOT, relPath), dest);
  created.push(relPath);
}

// Write a generated file into the target, creating parent dirs.
function writeGenerated(relPath, content, targetDir, created) {
  const dest = join(targetDir, relPath);
  mkdirSync(dirname(dest), { recursive: true });
  writeFileSync(dest, content);
  created.push(relPath);
}

// Scaffold the whole project. Returns the list of created relative paths.
export function scaffold(cfg) {
  const targetDir = resolve(cfg.target);
  assertSafeTarget(targetDir, cfg.force);
  mkdirSync(targetDir, { recursive: true });
  const created = [];

  // COPIED verbatim from the source repo (red-lines + the full host-independent
  // enforcement triad). Every file below only SCANS files / resolves paths from its
  // OWN on-disk location, so it runs in a fresh project with ZERO project-specific
  // wiring — the fresh project passes gate + arch-check + secret-scan out of the box.
  copyFromSource(join('.agent', 'AGENTS.md'), targetDir, created);
  // 1) file-size + root-count governance.
  copyFromSource(join('harness', 'gate.mjs'), targetDir, created);
  copyFromSource(join('harness', 'policies.yml'), targetDir, created);
  // 2) architecture enforcement: arch-check + its scan library, reading .arch/rules.yaml.
  //    rules.yaml MUST ship with policies.yml — arch-check's drift-guard asserts the two
  //    agree, so omitting either would FAIL the fresh project's arch-check.
  copyFromSource(join('harness', 'arch', 'arch-check.mjs'), targetDir, created);
  copyFromSource(join('harness', 'arch', 'scan.mjs'), targetDir, created);
  copyFromSource(join('harness', 'arch', 'scan-functions.mjs'), targetDir, created);
  copyFromSource(join('.arch', 'rules.yaml'), targetDir, created);
  // 3) hardcoded-secret scanning.
  copyFromSource(join('harness', 'secret-scan.mjs'), targetDir, created);
  // NOT copied (honest): check.py + acceptance.mjs need a fleshed-out .agent/ the fresh
  // project doesn't have yet; copying them would FAIL on day one. They light up later.

  // Generated starters.
  writeGenerated(join('.agent', 'PROJECT.md'), renderProjectMd(cfg.name), targetDir, created);
  writeGenerated(join('.agent', 'ROADMAP.md'), renderRoadmapMd(cfg.name), targetDir, created);
  writeGenerated(
    join('.agent', 'CURRENT_SPRINT.md'),
    renderCurrentSprintMd(cfg.name),
    targetDir,
    created,
  );
  writeGenerated(
    join('.agent', 'project.yml'),
    renderProjectYml(cfg.name, cfg.mode, cfg.lifecycle),
    targetDir,
    created,
  );
  writeGenerated('README.md', renderReadmeMd(cfg.name), targetDir, created);
  writeGenerated('.gitignore', renderGitignore(), targetDir, created);

  return { targetDir, created };
}

function main(argv) {
  let cfg;
  try {
    cfg = parseArgs(argv);
  } catch (err) {
    console.error(`forge-init: ${err.message}`);
    console.error(
      'usage: node harness/forge-init.mjs <target-dir> --name <project> ' +
        '[--mode balanced] [--lifecycle mvp] [--force]',
    );
    process.exit(2);
  }

  let result;
  try {
    result = scaffold(cfg);
  } catch (err) {
    console.error(`forge-init: ${err.message}`);
    process.exit(1);
  }

  console.log(`forge-init: scaffolded ${cfg.name} into ${result.targetDir}`);
  for (const rel of result.created) console.log(`  + ${rel}`);
  console.log('');
  console.log('inherited enforcement (run from the new project — all pass out of the box):');
  console.log('  node harness/gate.mjs              # file-size + root-count');
  console.log('  node harness/arch/arch-check.mjs   # layering/package/fanin/cognitive/');
  console.log('                                     #   naming/function-length/circular/drift');
  console.log('  node harness/secret-scan.mjs       # hardcoded-secret scan');
  console.log('');
  console.log('NOT yet inherited (need a fleshed-out .agent/; enabled as the project grows):');
  console.log('  check.py        — governance completeness (.agent/ agents/skills/workflows/eval)');
  console.log('  acceptance.mjs  — aggregate Stop gate (needs .agent/eval/acceptance.schema.yml)');
  process.exit(0);
}

// Run only when executed directly, not on import (keeps templating unit-testable).
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2));
}
