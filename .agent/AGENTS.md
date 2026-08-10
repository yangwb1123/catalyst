# AGENTS.md — 工程宪法

> 接管本仓库的 AI agent 的工程宪法;ForgeOS 自身遵守,经 `forge-init` 继承给每个被治理项目。
> 三档红线:🔴 **硬闸门**(`forge accept` 自动执法,违反即 REJECTED)· 🟡 **规范**(fresh-context Reviewer 裁量判)· 📋 **纪律**(工作方式约束)。
> 阈值是「触发审查」信号,非机械砍刀。无对应工具的检查诚实标 N/A,绝不伪造为通过。

## 🔴 硬闸门 — forge accept 自动拦截

- 单文件 ≤ 500 行 · 根目录 ≤ 15 文件
- 依赖方向:interfaces → application → domain 单向向内;domain 绝不 import 外层
- 函数 ≤ 50 行 · 循环依赖 = 0
- 包大小 · 生产扇入 · 认知负荷不超阈 · 禁技术角色目录名(utils/common/manager/handler/impl …)
- agent/skill/workflow/路由/mode 声明:全部交叉引用可解析,无悬挂引用、无声明-实现漂移
- 无硬编码 secret · test/app-test 全绿 · lint/coverage/sca 有工具则真查、缺则诚实 N/A

> 全部硬闸门均已机器执法,零 TODO;详情见 `forge accept` 输出。

## 🟡 规范 — Reviewer 独立裁量

- **单一职责**:一文件/一模块只做一件事;组合优先于继承;禁 God Object;禁根目录堆业务代码
- **Honesty 红线**:无工具项诚实标 N/A,不伪造通过;声明未实现的承诺属 GAP,不可默认为「已实现」
- **反镀金**:不做声明中不存在的特性;修复缺口前不顺手发明新行为

## 📋 纪律 — 工作方式约束

- **先拆分,再继续**:命中任何阈值即停新增,先重构,复检达标再继续
- **Reviewer 必须 fresh-context 独立 Agent**:实现者绝不审自己的代码;每轮 Review 使用独立干净的 Agent 会话
- **Host-agnostic 执法**:带外 gate(`forge accept`)是真相之源;宿主 hook(如 edit-time 加速器)只是可选的快速反馈,不替代完整闸门
- **复杂设计记 ADR**:架构决策、非显而易见的权衡、跨 sprint 的推理写入 `docs/adr/`
- **真点火需显式授权**:LLM 调用的真实预算消耗需获得用户明确许可,默认 dry-run 安全

## 机器可读规范入口

- `engineering/activation.yml`:v1 shadow 默认值和 canonical refs；旧项目无显式绑定时也安全降级为 shadow
- `engineering/disciplines.yml`:14 个 Prompt/Context/Memory/Tool/Planning/Loop/Reflection/Graph/Harness/Eval/Knowledge/Evolution/State/Contract 学科状态
- `engineering/rules.yml` + `engineering/detectors.yml`:原子规则、强度、例外与 detector；Error 只能绑定 `forge accept` 的真实载重探针
- `engineering/context-routes.yml`:typed predicate、固定合并代数、预算阻断/省略回执和不可信内容 lane（当前仅 shadow policy）
- `engineering/workflow-profiles.yml`:L0–L4 风险到 W0–W3 保障下限；只增强既有 workflow，不创建第二套 DAG
- `eval/completion-evidence.schema.yml`:只封装 source-bound 结构化执行观察，禁止写 `completed/accepted/verdict`；最终裁决只归 `forge accept`
- `engineering/backend-decision-gates.yml`:后端逐触发器 L1–L4/W1–W3 的业务不变量、模型边界、持久化、契约、算法、并发、可靠性、安全、容量、运维、迁移和演进决策合同（当前 shadow）
- `eval/backend-decision-package.schema.yml`:要求 14 个维度逐项 `addressed/not_applicable/blocked`，区分事实/假设/证据；不得自行批准或宣告完成
- `skills/backend-engineering.md` 及其 Domain/Data/API/Reliability/Security/Performance/Ops adapters：按路径和 capability 路由，不要求简单 CRUD 机械生成全部模型层
- `engineering/frontend-design-gates.yml` + `frontend-profiles.yml`:前端场景/Profile/Page Pattern、业务链路、状态×权限×数据×系统动作、Token、响应式、可访问性、动效/性能和视觉证据合同（当前 shadow）
- `eval/frontend-design-package.schema.yml`:分离 artifact 与 proof claim，截图绑定 source/build/fixture/environment；只判结构有效性，不授予 UI 质量或完成裁决
- `skills/{information-interaction-design,design-system-accessibility,frontend-client-engineering}.md`:复用现有 capability ownership；产品风格是 Profile，React/Vue/Flutter/RN 是条件化平台映射，不制造平行 Skill 树
- `skills/ui-geometry.md`:条件化 supporting procedural adapter；用 `business_ui_composition` 把角色/任务/状态/数据语义绑定到区域、轴、分组、间距、线条、形状与响应式关系，并可接收声明式 `geometry_measurement_receipts`。它不新增 capability owner，也不证明浏览器/原生 Runner 真执行、视觉质量或完成
- `engineering/governance-contracts.yml` + `skills/evidence-claim-management.md`:EvidenceRecord/KnowledgeClaim v1 identity/canonical/state、本地 exact append journal 与 versioned pure adapters；已交付的 ADR-0051 local gate 与 ADR-0052 local Evolve locator observation 都只允许显式 opt-in 的 Catalyst Go API，默认 capture 关闭且不签发 PASS/scan judgment/completion/persistence；ADR-0052 保留完整 canonical report、共享 bounded-interval source 与跨 relation multiplicity，但不认证 Git 或自动绑定 ADR-0050；引用闭包的 1024 records/16 MiB/256 depth 只防资源耗尽；scaffold 不安装 runtime，缺兼容 binary 时必须 `not_executed`

## 阅读顺序

`BOOTSTRAP → .agent/{PROJECT · ARCHITECTURE · ROADMAP · CURRENT_SPRINT} → 本文件 → engineering/context-routes.yml → 任务相关知识 → 代码`
