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
//   * Governance assets      .agent/{agents,skills,workflows,eval,routing,policies,engineering}/
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
import { mkdirSync, readdirSync, existsSync, lstatSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
// The SOURCE-tree copy primitives are shared with forge-upgrade (single source of
// truth for the __pycache__-skipping recursive walk), so they live in scaffold-fs
// (the same harness/scaffold/ sub-package).
import {
  assertNoSymlinkComponents,
  assertSafeRegularFile,
  assertSafeSourceProjection,
  copyFromSource,
  copyTree,
  enumerateTree,
  writeFileNoFollow,
} from './scaffold-fs.mjs';
// COPY MANIFESTS (data-driven — the 70% universal governance) live in their own
// module to keep this file under the harness's own 500-line cap; re-exported
// here (not just imported) so existing import sites (test_forge-init.mjs et al.)
// are unaffected.
import {
  GOVERNANCE_DIRS,
  COPIED_FILES,
  HARNESS_NOT_COPIED,
  SCAFFOLD_STATE_FILE,
} from './copy-manifest.mjs';

export { GOVERNANCE_DIRS, COPIED_FILES, HARNESS_NOT_COPIED, SCAFFOLD_STATE_FILE };

// The script's own location locates the ForgeOS SOURCE repo root so we copy the
// REAL tools. This tool lives in harness/scaffold/, so the repo root is TWO levels
// up: dirname(harness/scaffold/forge-init.mjs) === harness/scaffold; its parent ===
// harness; its parent === repo root.
const SCAFFOLD_DIR = dirname(fileURLToPath(import.meta.url));
const SOURCE_ROOT = dirname(dirname(SCAFFOLD_DIR));

export const GENERATED_FILES = [
  join('.agent', 'PROJECT.md'),
  join('.agent', 'ROADMAP.md'),
  join('.agent', 'CURRENT_SPRINT.md'),
  join('.agent', 'project.yml'),
  'CLAUDE.md',
  join('.github', 'workflows', 'forge.yml'),
  join('examples', 'starter', 'package.json'),
  join('examples', 'starter', 'src', 'greet.mjs'),
  join('examples', 'starter', 'test', 'greet.test.mjs'),
  'README.md',
  '.gitignore',
];

// Exact copied-file projection shared with forge-upgrade and the conflict
// preflight. Keeping this set data-driven makes it impossible for init to check
// fewer destinations than it later writes.
export function copiedProjection(sourceRoot = SOURCE_ROOT) {
  const fromDirs = GOVERNANCE_DIRS.flatMap((rel) => enumerateTree(rel, sourceRoot));
  const projection = [...new Set([...fromDirs, ...COPIED_FILES])].sort();
  return assertSafeSourceProjection(sourceRoot, projection);
}

export function renderScaffoldState(paths = copiedProjection()) {
  return `${JSON.stringify({ version: 1, copied: [...paths].sort() }, null, 2)}\n`;
}

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

engineering_spec:
  version: 1
  activation: shadow          # contracts validate; runtime routing remains unchanged
  refs:
    activation: .agent/engineering/activation.yml
    disciplines: .agent/engineering/disciplines.yml
    rules: .agent/engineering/rules.yml
    detectors: .agent/engineering/detectors.yml
    context_routes: .agent/engineering/context-routes.yml
    workflow_profiles: .agent/engineering/workflow-profiles.yml
    capability_catalog: docs/design/ai-engineering-os/capability-catalog.v1.yml
    capability_skill_map: docs/design/ai-engineering-os/capability-skill-map.v1.yml
    acceptance_policy: .agent/eval/acceptance.schema.yml
    completion_contract: .agent/eval/completion-evidence.schema.yml
  completion_authority: forge_accept
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
- \`workflows/\`  生命周期工作流 (discover / design / review / build / deploy / rollback / evolve)
- \`engineering/\` activation、14 学科、规则/detector、typed Context 路由与 W0-W3 保障契约(shadow)
- \`eval/\`       验收 + source-bound TaskEvidencePackage schema（不能自授 completed）
- \`routing/\`    模型路由策略 · \`policies/\` mode 表

\`deploy\` / \`rollback\` 只生成并验证声明式 \`docs/release/*\` 交付物；真实远程操作与
凭证始终归带外 CI/operator，不能把 workflow 名称当作“已部署”证据。

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
          node-version: '22'
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

export function renderStarterPackage() {
  return `${JSON.stringify({
    name: 'forgeos-starter-example',
    private: true,
    type: 'module',
    scripts: {
      test: 'node --test --test-reporter=tap test/*.test.mjs',
    },
  }, null, 2)}\n`;
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

// SAFETY: resolve every destination before the first write and refuse any
// collision unless force is explicit. This covers README/CI/harness conflicts,
// not just a pre-existing .agent directory.
function lexicalExists(path) {
  try {
    lstatSync(path);
    return true;
  } catch (err) {
    if (err?.code === 'ENOENT' || err?.code === 'ENOTDIR') return false;
    throw new Error(`cannot safely inspect ${path}: ${err.message}`);
  }
}

function assertSafeTarget(targetDir, force) {
  const planned = [...new Set([
    ...copiedProjection(), ...GENERATED_FILES, SCAFFOLD_STATE_FILE,
  ])];
  assertNoSymlinkComponents(targetDir, 'target directory');
  for (const rel of planned) {
    const destination = join(targetDir, rel);
    assertNoSymlinkComponents(destination, rel);
    // --force authorizes overwriting safe regular files; it does not authorize
    // discovering a late hardlink/special leaf after earlier files were changed.
    if (lexicalExists(destination)) assertSafeRegularFile(destination, rel);
  }
  if (force) return;
  const conflicts = planned.filter((rel) => lexicalExists(join(targetDir, rel)));
  const agentDir = join(targetDir, '.agent');
  if (existsSync(agentDir)) {
    let entries;
    try {
      entries = readdirSync(agentDir);
    } catch (err) {
      throw new Error(`cannot safely inspect ${agentDir}: ${err.message}`);
    }
    if (entries.length > 0 && conflicts.length === 0) conflicts.push('.agent/ (non-empty)');
  }
  if (conflicts.length > 0) {
    const preview = conflicts.slice(0, 8).join(', ');
    const more = conflicts.length > 8 ? `, … (+${conflicts.length - 8} more)` : '';
    throw new Error(
      `target contains ${conflicts.length} scaffold conflict(s): ${preview}${more}; ` +
      'pass --force to overwrite',
    );
  }
}

// Write a generated file into the target, creating parent dirs.
function writeGenerated(relPath, content, targetDir, created) {
  const dest = join(targetDir, relPath);
  writeFileNoFollow(dest, content, relPath);
  created.push(relPath);
}

function writeGeneratedProjectFiles(cfg, targetDir, created) {
  const files = [
    [join('.agent', 'PROJECT.md'), renderProjectMd(cfg.name)],
    [join('.agent', 'ROADMAP.md'), renderRoadmapMd(cfg.name)],
    [join('.agent', 'CURRENT_SPRINT.md'), renderCurrentSprintMd(cfg.name)],
    [join('.agent', 'project.yml'), renderProjectYml(cfg.name, cfg.mode, cfg.lifecycle)],
    ['CLAUDE.md', renderClaudeMd(cfg.name)],
    [join('.github', 'workflows', 'forge.yml'), renderForgeCi()],
    [join('examples', 'starter', 'package.json'), renderStarterPackage()],
    [join('examples', 'starter', 'src', 'greet.mjs'), renderStarterApp()],
    [join('examples', 'starter', 'test', 'greet.test.mjs'), renderStarterAppTest()],
    ['README.md', renderReadmeMd(cfg.name)],
    ['.gitignore', renderGitignore()],
    [SCAFFOLD_STATE_FILE, renderScaffoldState(copiedProjection())],
  ];
  for (const [relPath, content] of files) {
    writeGenerated(relPath, content, targetDir, created);
  }
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
  // Seed app + test so the load-bearing app_test_pass criterion is a REAL pass.
  writeGeneratedProjectFiles(cfg, targetDir, created);

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
