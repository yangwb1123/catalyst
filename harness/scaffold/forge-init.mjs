#!/usr/bin/env node
// ForgeOS forge-init (v1) — stamp a NEW project with inheritable, RUNNABLE,
// COMPLETE governance. One command scaffolds a target dir that boots into the
// full ForgeOS harness: `node harness/acceptance.mjs` runs and reports ACCEPTED
// on day one — not just the enforcement triad, but the whole acceptance gate.
//
// The 70% / 30% split (Catalyst Vision): UNIVERSAL governance is COPIED verbatim
// (it is the same for every project), PROJECT-SPECIFIC identity is GENERATED from
// --name/--mode/--lifecycle (it differs per project).
//
// COPIED verbatim (the 70% — universal, inheritable governance):
//   * Red-lines              .agent/AGENTS.md
//   * Governance assets      .agent/{agents,skills,workflows,eval,routing,policies}/
//       — the declarative role cards / skills / workflows / acceptance schema /
//         routing policy / mode table that check.py + acceptance.mjs validate &
//         consume. Without them check.py FAILs and acceptance has no schema.
//   * The full harness       harness/*.{mjs,py} TOOLS + their self-tests, plus
//       harness/arch/* and .arch/rules.yaml + harness/policies.yml. Every tool
//       resolves paths from its OWN on-disk location, so it runs in a fresh
//       project with ZERO project-specific wiring.
//   * CC adapter + CI        CLAUDE.md (points at .agent + harness) and
//       .github/workflows/forge.yml (runs `forge accept` as the CI gate).
//
// GENERATED per project (the 30% — project identity):
//   .agent/{PROJECT,ROADMAP,CURRENT_SPRINT}.md + .agent/project.yml + README + .gitignore.
//
// HONESTY: a fresh project ships no real FEATURES, so acceptance reports
//   coverage / lint / typecheck / build as N/A (never faked into a pass). The
//   load-bearing criteria are REAL: test_pass runs the copied harness self-tests
//   (green); complexity / arch_violations / architecture / security_findings scan
//   the fresh tree (clean); and app_test_pass runs a tiny SEED app's real,
//   passing test (examples/starter/ — replaced as features land, not a fake) so
//   the load-bearing app gate is live on day one. The verdict is ACCEPTED.
//
// PREREQUISITE: check.py needs PyYAML (`pip install pyyaml`); the generated CI
//   installs it. Without PyYAML, arch_violations/test_pass fail with check.py's
//   honest exit-2 "PyYAML is required" — an environment gap, not a scaffold bug.
//
// Usage:
//   node harness/forge-init.mjs <target-dir> --name <project> \
//        [--mode balanced] [--lifecycle mvp] [--force]
//
// Design: PURE templating functions (return strings, unit-testable without disk)
// are kept separate from the fs/copy I/O boundary at the bottom; the copy lists
// (GOVERNANCE_DIRS / COPIED_FILES) keep scaffold() data-driven and small.
import { mkdirSync, writeFileSync, readdirSync, existsSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
// The SOURCE-tree copy primitives are shared with forge-upgrade (single source of
// truth for the __pycache__-skipping recursive walk), so they live in scaffold-fs
// (the same harness/scaffold/ sub-package).
import { copyFromSource, copyTree } from './scaffold-fs.mjs';

// The script's own location locates the ForgeOS SOURCE repo root so we copy the
// REAL tools. This tool lives in harness/scaffold/, so the repo root is TWO levels
// up: dirname(harness/scaffold/forge-init.mjs) === harness/scaffold; its parent ===
// harness; its parent === repo root.
const SCAFFOLD_DIR = dirname(fileURLToPath(import.meta.url));
const SOURCE_ROOT = dirname(dirname(SCAFFOLD_DIR));

// --- COPY MANIFESTS (data-driven — the 70% universal governance) -------------

// Whole .agent/ governance-asset directories copied VERBATIM (recursively). These
// are universal: the declarative role cards / skills / workflows / acceptance
// schema / routing policy / mode table that check.py validates and acceptance.mjs
// consumes. Project IDENTITY (PROJECT/ROADMAP/CURRENT_SPRINT/project.yml) is NOT
// here — it is generated per project below.
// Exported (with COPIED_FILES) so test_forge-init.mjs's manifest-integrity guard
// walks harness/ against the REAL manifest, catching drift the moment harness/
// grows out of sync with these lists.
export const GOVERNANCE_DIRS = [
  join('.agent', 'agents'),
  join('.agent', 'skills'),
  join('.agent', 'workflows'),
  join('.agent', 'eval'),
  join('.agent', 'routing'),
  join('.agent', 'policies'),
];

// Individual files copied verbatim: the red-lines, the architecture rules, and
// the FULL harness — every TOOL plus its SELF-TEST, so check + accept both RUN in
// the fresh project and self-govern (the harness runs its own tests under
// acceptance's test_pass). Listed explicitly (not a blind harness/ copy) to omit
// __pycache__ and the human-only READMEs. The adapters/<lang>.yml command maps
// ARE copied (the lint criterion reads them); only adapters/README.md is omitted.
export const COPIED_FILES = [
  join('.agent', 'AGENTS.md'),
  join('.arch', 'rules.yaml'),
  // harness tools
  join('harness', 'gate.mjs'),
  join('harness', 'policies.yml'),
  join('harness', 'check.py'),
  join('harness', 'mode_gating_check.py'), // imported by check.py; without it check.py fails to import
  join('harness', 'acceptance.mjs'),
  // acceptance.mjs is split into a dependency-free kernel (shared run/result/
  // splitCmd + PASS/FAIL/NA/ROOT/HARNESS_DIR) and the adapter-backed quality
  // probes (lint + coverage); acceptance.mjs imports BOTH, so a fresh project
  // missing either fails to import the gate (ERR_MODULE_NOT_FOUND) — copy-anywhere
  // iron rule.
  join('harness', 'acceptance-kernel.mjs'),
  join('harness', 'acceptance-quality.mjs'),
  // adapters.mjs is imported by acceptance-quality.mjs (the lint/coverage criteria
  // shell the per-language adapter tools); without it the copied acceptance gate
  // would fail to import in the fresh project. The adapters/<lang>.yml command maps
  // it reads are copied below.
  join('harness', 'adapters.mjs'),
  join('harness', 'yaml2json.py'),
  join('harness', 'scorecard.mjs'),
  join('harness', 'scorecard-update.mjs'),
  join('harness', 'secret-scan.mjs'),
  join('harness', 'sca.mjs'), // imported by acceptance.mjs's dependency_vulnerabilities criterion
  // select-tests.mjs is the incremental (advisory) test selector — a fast edit-time
  // signal that NEVER replaces the full forge accept; it imports acceptance-kernel.mjs
  // (already copied). A scaffolded project inherits the same dev-loop accelerator.
  join('harness', 'select-tests.mjs'),
  join('harness', 'arch', 'arch-check.mjs'),
  join('harness', 'arch', 'scan.mjs'),
  join('harness', 'arch', 'scan-functions.mjs'),
  // per-language adapter command maps (read at runtime by adapters.mjs / the
  // lint criterion); the adapters/README.md prose is intentionally omitted.
  join('harness', 'adapters', 'go.yml'),
  join('harness', 'adapters', 'python.yml'),
  join('harness', 'adapters', 'typescript.yml'),
  // harness self-tests (acceptance's test_pass runs these — the harness self-governs).
  // test_enforce.mjs (pins the warn|block enforce resolution in the copied adapters.mjs)
  // was once dropped here — the drift test_forge-init.mjs's manifest guard now forbids.
  join('harness', 'test_check.py'),
  join('harness', 'test_mode_gating_check.py'),
  join('harness', 'test_yaml2json.py'),
  join('harness', 'test_acceptance.mjs'),
  join('harness', 'test_adapters.mjs'),
  join('harness', 'test_gate.mjs'),
  join('harness', 'test_enforce.mjs'),
  join('harness', 'test_scorecard.mjs'),
  join('harness', 'test_scorecard-telemetry.mjs'),
  join('harness', 'test_scorecard-update.mjs'),
  join('harness', 'test_secret-scan.mjs'),
  join('harness', 'test_sca.mjs'),
  join('harness', 'test_select-tests.mjs'),
  join('harness', 'arch', 'test_arch-check.mjs'),
];

// Harness sources DELIBERATELY not copied (test_forge-init.mjs's manifest guard
// whitelists these): forge-init.mjs is the SCAFFOLDER itself (a generated project
// does not carry the tool that created it) and test_forge-init.mjs exercises that
// absent tool. Any OTHER harness source must be in COPIED_FILES / GOVERNANCE_DIRS.
// All scaffold/upgrade-time tooling lives together in harness/scaffold/ (its own
// sub-package — kept out of the thin harness/ gate package). A generated project
// does not scaffold or upgrade sub-projects, so NONE of harness/scaffold/ is copied.
export const HARNESS_NOT_COPIED = [
  join('harness', 'scaffold', 'forge-init.mjs'),
  join('harness', 'scaffold', 'test_forge-init.mjs'),
  // scaffold-fs.mjs holds the copy/enumerate primitives forge-init and forge-
  // upgrade share; like forge-init itself it is a SCAFFOLD/UPGRADE-time tool, not
  // project-runtime governance (a generated project does not scaffold sub-projects),
  // so it is intentionally not copied.
  join('harness', 'scaffold', 'scaffold-fs.mjs'),
  // forge-upgrade resyncs a project's copied governance FROM a ForgeOS source repo;
  // it is an OPERATOR tool run against a project from OUTSIDE, never carried inside
  // one (a project does not upgrade itself from itself). Its self-test is likewise
  // an upgrade-time tool. Listed here so test_forge-init's manifest guard FORCES a
  // conscious decision whenever these change — the safety net, not an oversight.
  join('harness', 'scaffold', 'forge-upgrade.mjs'),
  join('harness', 'scaffold', 'test_forge-upgrade.mjs'),
];

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
- [ ] TODO: 跑通 \`node harness/acceptance.mjs\`(验收闸门 ACCEPTED)
`;
}

export function renderCurrentSprintMd(name) {
  return `# ${name} — Current Sprint

## Sprint 0 — 起步
- [ ] TODO: 把 ROADMAP v0 的第一项落地

**stop_condition:** roadmap 完成度 / 闸门 ACCEPTED(非「继续 N 轮」)。
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

Governed by ForgeOS. Verify with \`node harness/acceptance.mjs\` (forge accept).

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
.forge/
.DS_Store
`;
}

// CLAUDE.md — the Claude Code adapter. Points the agent at the .agent/ governance
// fact-source (red-lines first) and the runnable harness gate. Kept declarative:
// the truth lives in .agent + harness; this is the entry map.
export function renderClaudeMd(name) {
  return `# ${name} — Claude Code Adapter

由 ForgeOS forge-init 生成。本文件是 **Claude Code 适配器**:把 agent 指向治理事实源
([.agent/](.agent/))与可执行闸门(\`harness/\`)。真相之源是带外 harness,本文件只是入口地图。

## 先读红线 (Read the red-lines FIRST)
开工前先读 [.agent/AGENTS.md](.agent/AGENTS.md) —— 不可逾越的红线与阅读顺序。
随后:[.agent/PROJECT.md](.agent/PROJECT.md) · [.agent/ROADMAP.md](.agent/ROADMAP.md) ·
[.agent/CURRENT_SPRINT.md](.agent/CURRENT_SPRINT.md)。

## 治理资产 (Governance assets, under .agent/)
- \`agents/\`     角色卡 (architect / planner / implementer / reviewer / qa / …)
- \`skills/\`     可复用技能 (clean-architecture / testing / code-review / …)
- \`workflows/\`  生命周期工作流 (discover / design / build / evolve)
- \`eval/\`       验收 schema (acceptance.schema.yml —— Stop 闸门的机器可判定 DoD)
- \`routing/\`    模型路由策略 · \`policies/\` mode 表

## 闸门:完成前必须 ACCEPTED (Gate before done)
\`\`\`sh
node harness/acceptance.mjs      # forge accept —— 聚合验收闸门,必须 ACCEPTED
\`\`\`
聚合的判据(各自有真实带外检查):
- \`node harness/gate.mjs\`              文件行数 + 根目录文件数
- \`python3 harness/check.py\`           .agent/ 治理资产完整性(agent/skill 引用、路由档位)
- \`node harness/arch/arch-check.mjs\`   架构:分层/包/扇入/认知/命名/函数长度/环/漂移
- \`node harness/secret-scan.mjs\`       硬编码密钥扫描

## 诚实 (Honesty)
本项目尚无真实业务功能 → acceptance 会把 coverage / lint / typecheck / build 诚实标为
**N/A**(不伪装为 pass)。载重判据是真实检查,必须全绿:test_pass(跑复制的 harness 自测)、
complexity、arch_violations、architecture、security_findings,以及 app_test_pass ——
它跑 \`examples/starter/\` 这个最小种子 app 的真实测试(随真实 feature 落地而替换,不是假绿)。
注:\`check.py\` 需要 PyYAML(\`pip install pyyaml\`;CI 已安装)。
`;
}

// .github/workflows/forge.yml — CI gate. Runs `forge accept` (acceptance.mjs) on
// push/PR: checkout, set up Node + Python (+PyYAML for check.py), run the gate.
// This makes the SAME acceptance verdict the merge gate in CI.
export function renderForgeCi() {
  return `# ForgeOS CI gate — generated by forge-init.
# Runs \`forge accept\` (harness/acceptance.mjs) on every push / PR: the SAME
# aggregate acceptance verdict that runs locally becomes the merge gate here.
name: forge

on:
  push:
    branches: [main, master]
  pull_request:

jobs:
  accept:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
      - uses: actions/setup-python@v5
        with:
          python-version: '3.x'
      # check.py needs PyYAML; the rest of the harness is zero-dependency.
      - run: python3 -m pip install --quiet pyyaml
      - name: forge accept (aggregate acceptance gate)
        run: node harness/acceptance.mjs
`;
}

// --- seed app (makes app_test_pass a REAL pass, not N/A) ----------------------
// acceptance.mjs's app_test_pass is LOAD-BEARING: it discovers examples/<app>/
// with a test/ dir and runs the suite; with NO app it reports N/A, which is a
// hard reject (a load-bearing criterion must be PASS). So a complete template
// MUST ship a minimal REAL app + REAL passing test — the same dogfooding pattern
// the source repo uses. This is HONEST: the test genuinely runs and passes; it is
// a starter the project owner replaces with real features (NOT a faked pass).
// Kept clean so it also passes gate / arch-check / secret-scan unchanged.

export function renderStarterApp() {
  return `// ${''}Starter module — replace with your first real feature.
// Zero-dependency so the generated project stays dependency-free until you add
// your own. Exists so the acceptance gate's app_test_pass criterion is a REAL
// (passing) check from day one rather than an empty N/A.
export function greet(name) {
  return \`Hello, \${name}!\`;
}
`;
}

export function renderStarterAppTest() {
  return `// Starter test (node:test, zero deps) — proves the app suite is wired into
// the acceptance gate (app_test_pass). Replace alongside src/ as you build.
// Run: node --test examples/starter/test/greet.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { greet } from '../src/greet.mjs';

test('greet builds a friendly message', () => {
  assert.equal(greet('ForgeOS'), 'Hello, ForgeOS!');
});
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

// Write a generated file into the target, creating parent dirs.
function writeGenerated(relPath, content, targetDir, created) {
  const dest = join(targetDir, relPath);
  mkdirSync(dirname(dest), { recursive: true });
  writeFileSync(dest, content);
  created.push(relPath);
}

// Scaffold the whole project. Returns the list of created relative paths.
// Three data-driven phases: (1) copy whole governance-asset trees, (2) copy the
// explicit file manifest (red-lines + full harness), (3) generate project
// identity + CC adapter + CI. Everything copied resolves paths from its own
// on-disk location, so the fresh project runs the FULL acceptance gate.
export function scaffold(cfg) {
  const targetDir = resolve(cfg.target);
  assertSafeTarget(targetDir, cfg.force);
  mkdirSync(targetDir, { recursive: true });
  const created = [];

  for (const relDir of GOVERNANCE_DIRS) copyTree(relDir, SOURCE_ROOT, targetDir, created);
  for (const rel of COPIED_FILES) copyFromSource(rel, SOURCE_ROOT, targetDir, created);

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
  writeGenerated('CLAUDE.md', renderClaudeMd(cfg.name), targetDir, created);
  writeGenerated(join('.github', 'workflows', 'forge.yml'), renderForgeCi(), targetDir, created);
  // Seed app + test so the load-bearing app_test_pass criterion is a REAL pass.
  writeGenerated(
    join('examples', 'starter', 'src', 'greet.mjs'),
    renderStarterApp(),
    targetDir,
    created,
  );
  writeGenerated(
    join('examples', 'starter', 'test', 'greet.test.mjs'),
    renderStarterAppTest(),
    targetDir,
    created,
  );
  writeGenerated('README.md', renderReadmeMd(cfg.name), targetDir, created);
  writeGenerated('.gitignore', renderGitignore(), targetDir, created);

  return { targetDir, created };
}

function printNextSteps() {
  console.log('');
  console.log('inherited COMPLETE governance (run from the new project):');
  console.log('  node harness/acceptance.mjs        # forge accept — aggregate gate, ACCEPTED');
  console.log('    ├─ node harness/gate.mjs          #   file-size + root-count');
  console.log('    ├─ python3 harness/check.py       #   .agent/ governance-asset integrity');
  console.log('    ├─ node harness/arch/arch-check.mjs#   layering/package/fanin/cognitive/');
  console.log('    │                                 #     naming/function-length/circular/drift');
  console.log('    ├─ node harness/secret-scan.mjs   #   hardcoded-secret scan');
  console.log('    ├─ harness self-tests (test_pass) #   the harness self-governs');
  console.log('    └─ examples/starter (app_test_pass)#   seed app — real passing test');
  console.log('');
  console.log('honest N/A until real features land (NOT faked into a pass):');
  console.log('  coverage / lint / typecheck / build — no such tool wired yet');
  console.log('');
  console.log('prereq: check.py needs PyYAML (`pip install pyyaml`; the CI installs it).');
  console.log('CC adapter: CLAUDE.md   ·   CI gate: .github/workflows/forge.yml');
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
  console.log(`  (${result.created.length} files: governance assets + full harness + CC adapter + CI)`);
  printNextSteps();
  process.exit(0);
}

// Run only when executed directly, not on import (keeps templating unit-testable).
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2));
}
