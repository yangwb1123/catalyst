# BOOTSTRAP — 新 Agent 入口 (start here)

> 你是接管本仓库的 AI Agent。**先读本文,再读其它一切。** 本文是唯一入口。
> Read me first, then follow 阅读顺序 below. 双语:中文权威,English mirrors.

## 项目是什么 (What)
ForgeOS = **AI-native 软件工厂**:站在 Claude Code / Codex / Gemini CLI 等编码 CLI 之上的
**治理 + 编排控制平面**,让 AI 24h 自治完成 Idea→Production,而不写「上帝文件」、不让架构腐化。
不是某个应用,是元框架。详见 [`.agent/PROJECT.md`](.agent/PROJECT.md)。

## 技术栈 (Stack)
- **目的地**:Go-核心 polyglot(`forge-core`=Go · `forge-ai`=Python · `forge-runtime`=Rust · `forge-web`=TS)。
- **v0–v1**:编排骑 Claude Code 原生能力(subagents/hooks/skills);
  只写**声明式**(agent 卡 / workflow / policy)+ 薄胶水(`harness/gate.mjs` 现用 Node,够用)。
- **v2(现状)**:自研 Go 运行时已**搭好脚手架**——`forge-core/` 已落地、已构建、全绿(纯 Go 标准库、
  零外部依赖,13 个包,CLI `forge run/evolve/gate/check/accept/migrate/route`)。Agent 阶段**默认 dry-run / 不调 LLM**,
  真实执行器经 `--executor command --agent-cmd claude` 提供;YAML 经 python shim 转码。
- 时序与理由见 [`.agent/DECISIONS.md`](.agent/DECISIONS.md)(D1–D2)。

## 工程原则 (Principles)
单一职责 · 文件 ≤ 500 行 · 函数 ≤ 50 行 · 根目录文件 ≤ 15 · 循环依赖 = 0 · 依赖单向 · 禁 God Object。
全部红线(本仓库自身也遵守)见 [`.agent/AGENTS.md`](.agent/AGENTS.md)。**先拆分,再继续。**

## 目录规范 (Layout)
```
BOOTSTRAP.md              ← 你在这里;唯一入口
README.md                 ← 对外简介
.agent/                   ← 设计与决策的唯一事实源(Context 骨架)
  PROJECT · ARCHITECTURE · ROADMAP · CURRENT_SPRINT · AGENTS · DECISIONS
  project.yml             ← 本项目配置(mode/lifecycle/overrides/features)
  agents/                 ← 角色卡(product-manager / architect / researcher …)
  skills/                 ← 可复用操作(refactor-large-file …)
  workflows/              ← 四阶段流程编排(Discover→Design→Build→Evolve)
  architecture/           ← north-star + HA/安全演进(目标态,非现状)
harness/                  ← 约束执法(真相之源,host-independent)
  gate.mjs · policies.yml ← 主循环拥有,勿改
  adapters/               ← polyglot 闸门适配器(ts/python/go;lint/coverage/test 框架已接)
forge-core/               ← v2 自研编排运行时(纯 Go 标准库,零依赖,13 包;CLI forge run/evolve/gate/check/accept/migrate/route)
examples/                 ← dogfood 真实应用(url-shortener:经完整 pipeline 端到端建成)
docs/                     ← discovery/design/adr 产物(按需生成)
```
**只在你被分配的路径内写。** 业务代码按领域归入 `src/<domain>/`,绝不堆根目录。

## 开发流程 (Workflow)
脊柱 = **Discover → Design → Build → Evolve**(深度由 `project.yml: mode` 裁决):
- **Discover** — 需求探索 > 代码实现;PRD + 置信度,confidence ≥ 80% 才出。
- **Design** — 按 lifecycle 分阶段架构 → 方案 ──▶ ★ HUMAN APPROVAL ★(全系统最高杠杆闸门)。
- **Build** — Plan → Implement → [Harness 闸门] → Review → QA,stop = ROADMAP 100%。
- **Evolve** — Scan → Gap → Roadmap → Implement → Review → Evaluate →(loop)。

各阶段编排见 [`.agent/workflows/`](.agent/workflows/);角色卡见 [`.agent/agents/`](.agent/agents/)。

## 如何跑闸门 (Gate)
```
node harness/gate.mjs        # 快速体积闸门(edit-time 即时信号)
node harness/acceptance.mjs  # 完整 Stop 闸门(forge accept,聚合 8 检查 + test)
```
`gate.mjs` 查体积(`enforce: block`,见 `harness/policies.yml`);完整 Stop 闸门跑 `node harness/acceptance.mjs`(forge accept),聚合:`gate.mjs`(体积)· `arch-check`(架构 **8 检查**:layering / 包 / 扇入 / 认知 / 反模式命名 / **函数 ≤ 50 行** / **循环依赖 = 0** / drift-guard)· `check.py`(治理)· `secret-scan` · test / app-test;无工具的项诚实标 N/A。**每次修改后跑。**

## 阅读顺序 (Read order)
1. **BOOTSTRAP**(本文)→
2. [`.agent/PROJECT.md`](.agent/PROJECT.md)(是什么/目标)→
3. [`.agent/ARCHITECTURE.md`](.agent/ARCHITECTURE.md)(脊柱/中枢旋钮)→
4. [`.agent/ROADMAP.md`](.agent/ROADMAP.md)(版本纪律)→
5. [`.agent/CURRENT_SPRINT.md`](.agent/CURRENT_SPRINT.md)(此刻在做什么)→
6. [`.agent/AGENTS.md`](.agent/AGENTS.md)(工程红线)→
7. **代码 / 你的任务路径**。
