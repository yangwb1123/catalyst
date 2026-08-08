# ai-batch-runner 全资产移植与对比分析

本目录是 [ai-batch-runner](~/ai-batch-runner,ai-dev 工具集)的**完整资产移植**。
分批落地:Sprint 73(评审框架 + 高维分析工具)+ 本批(工程规范/产品思维/机制
文档/门禁脚本/回归套件/UI 规范)。

## 一、资产地图与对照(全部 copy 项)

| ai-batch-runner 资产 | 本仓库位置 | ForgeOS 等价物 / 用途 |
|---|---|---|
| `ai/prompts/00-09`(十阶段评审) | `docs/reviews/prompts/` | 评审框架(已用 6 轮,41+ 缺陷修复) |
| `ai/prompts-shared/`(共享片段) | `docs/reviews/prompts-shared/` | 已适配 ForgeOS 证据权威表 |
| `prompts/`(31+ 角色) | `docs/reviews/roles/` | fresh-context 独立评审 Agent |
| `pbatch/` 高维分析(assess/rules/classify/eval) | `docs/ai-batch/pbatch/` + `pi-batch.py` | 需求评估/规范匹配/任务分类/回归 |
| `backend-specs/`(17 md + rules.yaml + design-intelligence 8) | `docs/ai-batch/backend-specs/` | **新增**:工程规范资产(架构宪法/生产就绪/持久化建模/DDD/测试/系统工程/网络工程/演进/OOP-DI/复杂度/agent-guardrails/算法) |
| `product-specs/`(4 md) | `docs/ai-batch/product-specs/` | **新增**:产品思维分级/开源就绪/商业就绪/完成证据 |
| `docs/` 机制精选(15 md) | `docs/ai-batch/mechanism/` | **新增**:工程哲学/规范体系/决策日志/教训/复盘/7×24 运营/门禁注册表/提交规范 |
| `scripts/` 门禁精选(4 py) | `docs/ai-batch/scripts/` | **新增**:完成证据/拒绝检测/LLM 裁决/后端工程门禁 |
| `evals/`(3 yaml) | `docs/ai-batch/evals/` | **新增**:规则回归套件配置 |
| `ui-specs/`(全部) | `docs/ai-batch/ui-specs/` | **v3 预留**:ForgeOS 当前无前端;Web UI(v3)启动时使用 |

## 二、明确不 copy(对比后判定)

| 资产 | 原因 |
|---|---|
| `snaplink-platform/`、`projects/`(sverp/iris-ui) | 项目特有(ERP/设备/前端),非通用资产 |
| `examples/*.yaml` 流水线 | meta 编排/pi-batch 流水线,与 ForgeOS Graph 编排协议重叠(反镀金) |
| `pi-batch.py` 编排层(runner/pipeline/campaign/memory/learn) | ForgeOS 有更强的 Graph 编排协议;记忆/自演进有等价机制(ADR/会话) |
| `ui-specs` 检查脚本(check-ui-spec 等) | 无前端代码可检查;v3 时随 Web UI 启用 |
| `.pi-batch/rejected/` 等运行时产物 | 上游运行残留,非资产 |

## 三、新增资产与 ForgeOS 现有能力的对应

| 新增资产 | ForgeOS 现状 | 互补价值 |
|---|---|---|
| `backend-specs/architecture-constitution.md`(最高层架构宪法) | `.agent/ARCHITECTURE.md` + AGENTS.md 宪法 | 15 问/耦合八型/爆炸半径/重试预算审查视角 |
| `backend-specs/persistence-modeling.md`(四模型/主键/金额/索引/审计/迁移) | SQLite v23 链(结构 digest/迁移/备份) | 审查 SQLite schema 的对照基准 |
| `backend-specs/production-readiness.md`(11 项门禁) | Stage 06 生产就绪评审(备份/恢复) | 补充:容量/多租户/可观测/发布恢复维度 |
| `backend-specs/agent-guardrails.md`(防幻觉/变更控制) | 诚实纪律(AGENTS.md) | 结构化防幻觉审查清单 |
| `product-specs/completion-evidence.md`(完成证据) | forge accept 的诚实 N/A | 产物级完成报告基准 |
| `scripts/check-completion-report.py` | 无对应 | 产物 Definition-of-Done 检查(可选 gate) |
| `scripts/check-no-refusal.py` | 无对应 | 检测 agent 拒绝(限额/策略)模式 |
| `mechanism/ENGINEERING_PHILOSOPHY.md` 等 | 工程宪法 | 团队哲学参照 |

## 四、使用方式

```bash
# 需求评估(高维,已可用)
python docs/ai-batch/pi-batch.py assess "..." --file req.md

# 评审(已用 6 轮)
python docs/reviews/run-review.py --all --context docs/reviews/examples/xxx.yaml

# 规范资产直接阅读:docs/ai-batch/backend-specs/persistence-modeling.md 等
# 门禁脚本(可选注册为验证器):
#   python docs/ai-batch/scripts/check-completion-report.py <artifact>
```

## 五、诚实说明

- `backend-specs`/`product-specs` 中的角色与规范来自上游项目语境,已做
  Snaplink→ForgeOS 名称替换,但业务示例(ERP/登录等)保留原文作为知识。
- `mechanism/` 文档保留上游原文(含其项目名),作为**运营档案与哲学参照**,
  不声称是 ForgeOS 现状;ForgeOS 的权威入口始终是 `.agent/` 与 `harness/`。
- `ui-specs/` 明确为 v3 预留,当前无前端代码消费。
