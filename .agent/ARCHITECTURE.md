# ForgeOS — Architecture

> 本文 = **当前/脊柱**(v0–v1 在 Claude Code 上的形态)。
> **目标架构(分布式 HA 微服务全貌)** 见 [`architecture/north-star.md`](architecture/north-star.md) + [`architecture/ha-security-rollout.md`](architecture/ha-security-rollout.md)。

## 脊柱:Idea → Production
```
DISCOVER  (深度由 mode 裁决)
  Requirement-Discovery → 置信度% + 缺失信息   stop: confidence ≥ 80%
  Market-Research       → 竞品/能力矩阵         (必须真实检索+引用,防自信虚构)
  Product-Designer      → MVP / 高级 分层
DESIGN
  Solution-Architect    → 架构(按 lifecycle 分阶段,非峰值 QPS)
  Proposal-Generator    → 1页方案+成本+风险 ──▶ ★ HUMAN APPROVAL ★
                                                 └ 批准 → 仅解锁 REVIEW（产物由 phase 契约负责）
REVIEW  (深度由 mode 裁决;对齐 AI-SDLC Stage 2-6)
  Security-Engineer     → STRIDE 威胁建模 + RFC 合规矩阵
  Distributed-Engineer  → 故障模式矩阵 + 一致性策略 + 重试策略
  Performance-Engineer  → 性能预算 + 生产就绪检查清单
  CTO                   → 综合裁决(Approve/Simplify/Redesign/Delay/Reject)
BUILD
  Planner → Implementer → [Harness 闸门] → Reviewer → QA   stop: ROADMAP 100%
DEPLOY  (声明式生产交付边界;不访问凭证/不执行远程部署)
  Release-Engineer      → Manifest + Plan + Runbook + Go/No-Go + Validation
  External CI/Operator  → 实际应用 ──▶ ★ HUMAN APPROVAL MARKER ★
ROLLBACK  (独立按需,不接主链)
  Release-Engineer      → Rollback Plan + Runbook + Checklist + Validation
  External CI/Operator  → 实际应用 ──▶ ★ HUMAN APPROVAL MARKER ★
EVOLVE
  Scan → Gap → Roadmap → Implement → Harness → Review → Evaluate → (loop)
```

## 中枢旋钮:mode × lifecycle
一个设置同时驱动三处:**Router 档位 · Harness 严格度 · Workflow 深度**。
- mode: explorer(快/省/跳 Discover) · balanced · engineering(全闸门) · cto(只出 PRD/Arch,人确认)
- lifecycle: idea → mvp → growth → production
- explorer→engineering = 一次「创业→企业」状态迁移:自动收紧 harness + 派生补测试/CI/监控任务。

## 载重墙(对原始构想的修正)
"站在所有 CLI 之上" ⇒ 只能强制最弱宿主允许的东西。因此:
- **真相之源 = 带外执法层**(Sandbox / CI runner 跑 harness 闸门),host-independent。
- 各工具的 hook(CC 的 PostToolUse/Stop 等)= **加速器适配器**(编辑器内快速失败),非地基。
- 每个宿主一个薄 adapter;无阻断能力处优雅降级为 advisory。
- **生产交付边界**:`deploy`/`rollback` 只生成与验证精确声明的 `docs/release/*`；
  command-mode 使用最小固定 prompt、operator-pinned Claude executable bytes(非供应商身份)、整树 postflight 和
  receipt/source/artifact freshness。这里的 source 是排除 `.forge/**`、`docs/release/**`
  和 commit metadata 的 product 工作树摘要，不是 Git commit identity。云/K8s 凭证和远程执行始终归外部 CI/operator；
  人核对外部证据后写 approval marker，agent 不得自证发布成功。

## 引擎 (Engines)
Gateway · Orchestrator · Agent-Runtime · **Model-Router** · Context-Engine · Memory-Engine ·
Knowledge-Engine · **Evaluation-Engine** · **Sandbox(载重墙)** · Web-UI
> **v2 现状**:forge-core Go 运行时当前 **21 包**(纯标准库零依赖),已落地 5 个核心引擎与可工作的本地 Agent-Runtime 切片:
> **Orchestrator**(`internal/orchestrator`)· **Model-Router**(`routing`)· **Context-Engine**(`prompt`)·
> **Memory-Engine**(`memory`)· **Evaluation-Engine**(`converge`);Agent-Runtime 已具备本地命令执行、预算/超时/进程组、最小环境、stdin prompt、产物溯源与 run lock。`forge run --chain` 以版本化状态跨 Discover→Design→Review→Build→Deploy→Evolve 持久恢复，拒绝/cycle/max-stage/策略 halt 均失败关闭。
> `forge-runtime/` 现有 Rust 原生多轮模型/工具循环、SQLite local-first Conversation Hub 与 durable Project Run：无路径 Global、有路径 Project、Group 联动；execution-bound Project Run、append-only event journal、O(1) 增量语义 cursor、同快照 inspection、严格有界 causal user/assistant history、Run 原子授权 assistant 写回、terminal/incomplete/pending-tool 判定均跨进程持久化。Group dossier 还可被原子冻结为独立的 prepared Group Run，幂等重放精确旧快照且不查询最新历史；prepare/show/list 始终本地管理态，不启动模型、工具或 workspace。默认 deterministic/offline；显式 `--live` 默认零工具，仅 exact `--allow-read` 授权，并启用固定 HTTPS origin、无 redirect/隐式 retry、`store:false` 完整 validated output-item 回放、phase-aware final projection、terminal status/item identity 校验及 transport/SSE/token/output 全套上限；incomplete 永不释放工具。SQLite open→PRAGMA/WAL→schema 有统一 5 秒重试，DB/WAL/SHM 私有权限及 workspace capability 失败有并发/反例回归。Group snapshot 的模型消费、自动 execution resume/branching、远程账号与同步、共享 ACL/Group 多 Agent、审批、写/进程工具及 OS 沙箱仍未实现。
> **Gateway · 完整 Knowledge-Engine · Sandbox runner(载重墙)· Web-UI 仍为路线图。**

## 模型路由 (v1 限 Claude 档)
classify → score(复杂度/风险/依赖/安全/上下文) → tier(mode,lifecycle)
→ ⚠ risk≥critical 强制 ≥Opus → 💰 预算守卫 → 📈 历史择优(来自 Eval 记分卡)。
档位:Haiku=文档/CRUD/测试 · Sonnet=常规实现 · Opus=架构/安全 + 所有 Reviewer。跨厂商池 = v3。
