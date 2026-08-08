# ADR-0037 — 能力中心化的 AI 软件工程组织与治理知识模型

- 状态：已接受（2026-08）
- 范围：目标组织模型与后续实施顺序；本 ADR 不授予新的运行时、写入或生产权限
- 关联：ADR-0004（AI-SDLC Review）、ADR-0014（严格 QA）、ADR-0020（证据化 Evolve）、
  ADR-0038（AADM/Reflection）、ADR-0039（Device Fabric）、`.agent/AGENTS.md`、`.agent/architecture/loop-engineering.md`

## 背景

ForgeOS 已有可工作的 workflow/DAG、预算与恢复、带外门禁、fresh-context Review、严格 QA、声明式发布边界、
artifact provenance 和 evidence-backed Evolve。仓库同时存在三套有价值但未统一的工程知识：

1. `.agent/` 的 13 张可执行角色卡、9 张 Skill 卡和 7 条工作流；
2. `docs/reviews` 的产品、BA、UX、架构、数据库、SRE、合规等 advisory review lenses；
3. `docs/ai-batch` 的 standalone 确定性分类与评估规则。

它们尚未共享企业级的软件工程知识模型。当前 Memory 仍是平面自由文本，Context 只投影有限 ROADMAP/ADR 标题/
工程规则，风险主要从路径启发，且没有统一的 Fact/Decision/Assumption/Evidence、System Knowledge Graph、Change
Impact/Cost、Technical Debt、Software Health 或通用 capability grant。

继续简单增加 Agent 会放大不一致：角色越多，越可能基于不同事实、不同上下文和隐含权限产生互相冲突的产物。需要先
固化软件工程语义和持续演化机制。

## 决策

### 1. 组织采用“生命周期节点 × 可复用能力 × 显式权限”

生命周期节点负责一次明确决策；角色提供独立责任视角；Capability/Skill 提供可复用方法、规则、模板和检测器；
Agent instance 只是某次 Run 临时绑定 role、capabilities、ContextPackage、CapabilityGrant 和 budget 的执行者。

不按每个职能名永久创建 Agent，也不让角色名称隐含权限。低风险任务可合并角色，高风险任务必须 separation of duties。

### 2. 采纳 00–16 生命周期责任图

目标节点为 Intake/Orchestration、Requirement/BA、Product、UX/UI、Domain、Architecture、Data、API/Integration、
Planning、Development、Review/Refactoring、Security/Privacy/Compliance、QA、Performance/Reliability、Release、
Operations/SRE、Evolution。

00 依据影响构建 DAG；03–07 可按依赖并行；10–13 独立并行审查；安全、测试、可运维性同时横切更早节点；生产执行
继续由外部 CI/operator 负责。

### 3. 先建 Governance Kernel，再扩角色 Skill

实施顺序冻结为：

1. Governance Envelope、Evidence/Claim、权限与状态迁移；
2. content-addressed ContextPackage；
3. 可重建 System Knowledge Graph 与 Impact/Cost/Risk；
4. ADR v2、Technical Debt、Engineering Constitution、Software Health；
5. capability registry、角色/Skill/workflow adapter 与 strict Review Loop；
6. RuntimeObservation、Incident 与安全 Evolution；
7. 多租户/远程企业能力后置。

不能先把 free-text RAG 或大模型推断写成 Knowledge Graph 再补事实语义。Graph 是权威源的可重建索引；缺边输出 UNKNOWN，
不输出“无影响”。Embedding 可做检索加速，但不提供事实权威。

配套 ADR-0038 把 AADM/Capability ABI/Meta Reflection 固化为节点之下的决策内核；ADR-0039 把 ExecutionTarget/Device Fabric
固化为默认 OFF 的独立执行平面。二者扩展本 ADR，不改变 00–16、Governance Kernel、proposal-first learning 或生产带外边界。

### 4. 认识状态必须类型化

至少区分 Fact、Constraint、Decision、Inference、Assumption、Hypothesis、Lesson、Proposal 和 Unknown。Evidence 支持或
反驳 Claim，但不自动等于 Fact。Confirmed Fact 必须绑定当前有效证据；Assumption/Inference 不能满足 hard gate；
Accepted Decision 指向 ADR/approval；Proposal 不能变成隐式 scope。

### 5. 工程模式条件化，不写成教条

God File 结合职责、变化耦合、内聚、依赖、effect 和测试痛点分析，不只看行数。当前 500 文件行/50 函数行/零循环依赖
继续是项目硬门；其它复杂度指标默认是 review trigger。

OOP 用于身份/生命周期/不变量；DI 用于外部或易变 seam；AOP 只用于语义一致且顺序/失败明确的横切策略；DDD 只用于
复杂领域；事件驱动必须连同 schema/outbox/幂等/顺序/retry/DLQ/replay/观测一起设计。组合和纯函数在相应问题上优先。

### 6. 统一而不另建平行体系

`.agent` 保持可执行主干。后续建立一个 canonical capability/governance registry，再生成/校验 `.agent` role/skill/workflow
适配，并让 `docs/reviews` 与 `docs/ai-batch` 消费同一语义。任何规划 YAML 在 validator/runtime adapter 完成前都明确
`planning_only`，不能被描述为已执行。

## 权限与诚实边界

- 本 ADR 只采纳目标模型与实施顺序；没有实现任何新 schema、Graph、Agent、workflow、gate 或生产能力；
- 不扩大 ForgeOS 的远程发布非目标，不允许 Agent 持有生产凭证或自证上线成功；
- 不把旧 Memory、文档或模型输出自动迁成 confirmed fact；
- 不把 Software Health 总分作为发布批准，也不让综合分抵消 Critical finding；
- 不把 Context cache、Graph edge 或 Review 多数票冒充当前事实/决策权威。

详细 SOP、契约、工程规则、机读规划目录和实施验收见
`docs/design/ai-engineering-os/`。

## 后果

**正面。** Agent 数量与工程知识解耦；影响、权限、证据和长期记忆可在所有角色间统一；Role/Skill 可渐进扩展且可被
机器校验；Review 冲突有明确事实与权威顺序；Evolution 能从生产事实生成新变更而不隐式扩权。

**成本。** 需要版本化 schema、跨语言 canonical fixtures、旧资产迁移、Graph extractor、Context/permission adapter、
新增 governance storage 和大量对抗测试。短期文档比 runtime 领先，必须持续标注 planned 与 shipped。

**风险。** 过度建模会拖慢低风险工作，因此流程按 L0–L4 materiality 裁剪；Graph 只做可重建索引；能力按 cluster 增量
交付，不一次创建空壳；所有阈值区分 hard gate 与 review trigger。

## 被拒方案

1. **为每项职能创建永久 Agent。** 权限、知识和执行身份耦合，角色越多漂移越大。
2. **先做向量 RAG/大而全 Knowledge Graph。** 缺少 Claim/Evidence 语义会把猜测永久化。
3. **把所有工程实践设为硬规则。** OOP/DI/AOP/DDD 和多数复杂度阈值依赖上下文，机械执法会制造碎片和抽象污染。
4. **重写现有工作流。** 现有 `.agent`/Go/Rust 控制原语已验证，应通过 adapter 增量扩展而非另建第二 DAG engine。
5. **一步实现全部角色与企业平台。** 违反增量纪律；先完成 Governance Kernel 和两个端到端验收切片。

## 重审触发器

- Evidence/Claim 模型无法表达两个端到端影响场景而需要破坏性调整；
- capability registry 与现有 `.agent` workflow 无法通过无损 adapter 对齐；
- Graph 维护成本超过影响分析收益，或覆盖/新鲜度长期不足以支持强结论；
- 多租户/远程生产边界经独立 ADR 和用户授权发生变化。
