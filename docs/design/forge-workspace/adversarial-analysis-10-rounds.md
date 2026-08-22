# Forge Workspace 十轮产品与架构对抗式分析

> 状态：**Proposed / Red-team review，不构成批准**
> 日期：2026-08-21
> 审查对象：[产品蓝图](../ai-engineering-os/product-control-plane-blueprint.md)、[实施计划](implementation-plan.md)、[详细架构](architecture-plan.md)、[功能交互](functional-interaction-design.md)

## 1. 审查方法

每一轮由两个视角共同攻击方案：

- **产品经理视角**：用户价值、采用成本、范围、指标、商业和交付风险；
- **架构师视角**：边界、复杂度、一致性、安全、可恢复性和演进成本。

每轮必须给出最强反方论点、不能用愿景替代的证据、对计划的实际修正和停止条件。结论不是证明原方案正确，而是缩小仍可理性下注的范围。

## Round 1 — 这到底是产品，还是一套没人直接需要的平台？

### 攻击

产品经理：用户不会购买“AI 软件工程 Kubernetes”。他们只想让一个 Agent 更可靠地完成任务；Space、Graph、Kernel、Receipt 等概念会增加学习和操作成本。

架构师：同时建设平台内核和 Workspace App 会产生两套产品路线。现有 `.agent/PROJECT.md` 又明确说 ForgeOS 不是具体应用，组织目标冲突可能让每个功能都既是框架又是 App。

### 最强反方结论

如果用户的首要问题只是“看见 Codex 做了什么并能恢复”，那么完整 Control Plane 愿景属于提前平台化。产品可能在交付首个可用界面前耗尽资源。

### 方案修正

- 品类外部表达改为“可观测、可恢复的 Agent 项目交付工具”，不在首屏销售平台术语；
- ForgeOS Platform 是内部技术边界，Forge Workspace 是唯一用户产品；
- R0 只验证一个 JTBD：从 Objective 发起一次受控任务，看到完整 Timeline 和验证结果；
- Space 在 R0 只是项目容器，不承诺完整跨项目智能。

### 证据与停止条件

必须用 5–10 个真实项目任务观察：用户能否无需解释 Platform 术语完成流程。若大多数用户仍退回直接使用 Agent CLI，且 Timeline/恢复未显著降低返工或不信任，则暂停 WorkGraph/Graph 投资，重新定义产品。

## Round 2 — 当前项目是否再次陷入“协议先于用户价值”？

### 攻击

产品经理：已有大量 ADR、contract、golden 和跨语言 parity，但 App、TUI、Change Cockpit 仍不存在。新计划又提出 Platform Core，可能重复同一路径。

架构师：在完整领域未知时冻结大量 wire，会把早期错误固化为兼容负担；每增加一个跨语言 contract 都放大迁移和测试矩阵。

### 最强反方结论

Platform Core 可能只是把现有合同膨胀重新命名。若先设计 Objective/Change/WorkGraph 的全部协议，仍会得到“协议完整、产品不可用”。

### 方案修正

Platform Core v1 首期只允许直接服务 R0 的最小 wire family：

1. common event envelope；
2. StartAttempt/Interrupt/ActionDecision；
3. Runtime Action events；
4. ArtifactRef；
5. ExecutionReceipt；
6. VerificationRequest/Receipt。

Objective/Change 可以先作为 Go 内部 domain，不做跨语言 wire。任何新合同必须写出正在解锁的用户操作和删除它会破坏的垂直测试。

### 证据与停止条件

R0 前 Platform Core normative wire 不超过上述六类。若新增 wire 没有被真实 Go↔Rust/Harness process test 和 UI Timeline 使用，禁止进入 active Registry 或正式兼容承诺。

## Round 3 — Go + Rust + Node/Python 是否是不可承受的团队税？

### 攻击

产品经理：多语言不创造直接用户价值，却增加招聘、调试、打包和发布成本。

架构师：本地进程协议、两套数据库和跨语言 canonicalization 都是故障面。单进程 Go 或全 Rust 可能更简单。

### 最强反方结论

保留现有投资不应成为沉没成本谬误。若 Rust Runtime 只包装 CLI 进程，而 Go 已能可靠执行命令，双语言边界可能没有足够收益。

### 方案修正

- 不立即重写，但给语言边界设置量化复核点；
- Rust 必须证明其独有价值：安全 effect boundary、durable Action journal、process/stream correctness、uncertainty/no-resend；
- Go 不再新增 provider/tool loop，Rust 不再新增产品调度；
- Node/Python 只保留 Harness 生态适配和独立验证，不进入产品状态机；
- 统一发布包必须自动发现 runtime compatibility，不能要求用户手工配版本。

### 证据与停止条件

R1 复核：若超过 30% 的实现/缺陷工作量来自 Go↔Rust 协议和版本漂移，且 Rust 没有关闭可测量的安全/恢复风险，则评估合并执行面。若 Rust 明显降低 uncertain/resend/path/process 风险，则保留边界。

## Round 4 — “Platform Core”会不会变成新的共享 God Module？

### 攻击

产品经理：用户看不到 Platform Core，投入很难排优先级。

架构师：共享 ID、状态机、命令、事件、Receipt 很容易继续吸收 Policy、Knowledge、Graph、Evolution，最终所有组件都依赖一个巨大核心。

### 最强反方结论

所谓稳定语义可能并不稳定；跨组件共享 domain 类型会导致 lockstep release，并把独立 bounded context 重新耦合。

### 方案修正

- Platform Core 只拥有跨进程 wire，不拥有所有领域实现；
- Space/Objective/Change 规则留在 Go，Attempt/Action 规则留在 Rust；
- `contracts/` 按 wire family 分包，无全量 `core.schema`、无通用 Entity 基类；
- binding 只在边界转换，内部 domain 不直接使用 wire DTO；
- 每个 wire 有 owner、consumer 和删除/升版策略。

### 证据与停止条件

若一个 contract 只有单一进程消费者，不进入 Platform Core。若任何 package 超过三个 bounded context 或 release 必须全仓 lockstep 才能工作，立即拆分并重新审查。

## Round 5 — “每次 Agent 执行了什么都能看到”是否是虚假承诺？

### 攻击

产品经理：用户会把“全部可观测”理解为包括模型内部推理、所有文件读取、子进程和网络，而不同 Agent Host 暴露能力不同。

架构师：仅解析 CLI JSONL 看不到绕过 adapter 的 effect；Sandbox/syscall 观察又昂贵且平台相关。展示不完整 Timeline 可能比不展示更危险。

### 最强反方结论

如果无法证明 coverage，Timeline 会制造安全感。尤其是 Claude/Codex 版本变化后，未知事件可能被错误忽略。

### 方案修正

定义三层可观测覆盖：

- L1 Host-reported：宿主结构化 session/turn/tool event；
- L2 Runtime-observed：Runtime 监督的 process/file/network/tool effect；
- L3 Verified outcome：Harness 对 Snapshot/Artifact 的独立检查。

UI 按 Adapter capability matrix 展示覆盖率和 gap；未知事件 fail visible，不能静默丢弃；产品文案使用“可观测到并记录所有通过 Forge effect boundary 的动作”，不承诺隐藏思维链。

### 证据与停止条件

每个 Adapter release 都要跑 transcript compatibility 和 effect fixture。若关键写/命令可绕过 Runtime 且 UI 无法标识 gap，不允许该 Adapter 进入自动模式，只能 advisory/observe 模式。

## Round 6 — 两个 SQLite 是否制造本地版分布式系统和 split-brain？

### 攻击

产品经理：用户只关心本地可靠，双数据库会增加恢复、备份、诊断和安装复杂度。

架构师：Control 认为 Attempt requested，Runtime 可能已 accepted；outbox/inbox、cursor 和幂等正是在实现分布式一致性。单数据库单进程可能更可靠。

### 最强反方结论

“所有权清晰”不足以自动证明双库合理。若两个进程总是同机同生命周期，物理隔离可能得不偿失。

### 方案修正

- R0 不要求独立长期 daemon；可由 Go server 监督 Rust child，但仍禁止共享表；
- 提供一个原子用户备份/诊断命令，统一收集两库、CAS manifest 和版本，不让用户理解内部边界；
- 所有跨库状态在 UI 显示 pending/uncertain，不制造同步完成假象；
- 记录 event lag、outbox backlog、recovery time 和协议缺陷成本；
- 保留“单进程嵌入或单数据库”的替代 ADR，不把双库当教条。

### 证据与停止条件

故障注入必须覆盖任意一步 kill。若 R1 中跨库恢复缺陷或运维成本持续高于 Runtime 隔离带来的收益，重新评估物理合并；不得通过让 Go 直读 Runtime 表临时绕过。

## Round 7 — 自动推进是否会放大 Agent 错误和成本？

### 攻击

产品经理：自动化越强，用户越难保持情境；过多审批又会让系统比直接用 CLI 更慢。

架构师：确定性 Reconciler 只能确定地执行错误计划。Snapshot、验收和 Policy 不完整时，自动推进会把局部误判传播到后续节点。

### 最强反方结论

“LLM 不拥有状态迁移”并不等于安全；如果 WorkGraph 和 AcceptanceCriteria 由同一 Agent 生成，Controller 只是可靠地放大它的错误。

### 方案修正

自治等级按 effect 和风险分层：

- A0：只规划/观察；
- A1：只读执行和验证；
- A2：批准计划内的本地写，关键 effect 逐次审批；
- A3：低风险多节点自动推进；
- A4：受控远程/发布，首期禁用。

默认 A1/A2。计划批准、执行、Verification 和 residual risk 分开；Reconciler 对 Unknown、stale、uncertain、coverage gap 一律停机。

### 证据与停止条件

R1 指标必须包含人工干预率、错误传播深度、无效 token/cost、replan 率和审批等待。若自动推进没有降低 lead time，或提高严重返工/越权，退回更低自治级别，而不是增加更多 Agent reviewer 掩盖问题。

## Round 8 — 跨项目知识图谱是否是昂贵但低使用率的展示功能？

### 攻击

产品经理：多数早期用户只有一两个仓库；全局图可视化常常漂亮但不能驱动具体决策。

架构师：stable identity、API/DB/deployment/owner extractors、freshness 和跨语言 build graph 是独立大产品，容易吞掉主线。

### 最强反方结论

在没有稳定 Change/Outcome 前建设图谱，会产生大量 stale/inferred 数据，而且无法证明提高完成质量。

### 方案修正

- R0/R1 只做 Project catalog 和当前 Change 的局部 impact evidence；
- Phase 3 首先支持 manifest/import、OpenAPI 和 DB migration 三类高确定性 extractor；
- Graph UI 默认展示“回答当前问题的最小路径”，不做宇宙图；
- 只有用户实际打开影响路径、因此修改计划或避免缺陷时，才算图谱价值；
- LLM inferred edge 默认不参与权限和硬 gate。

### 证据与停止条件

如果跨项目 impact 未减少漏改、回滚或审查时间，停止增加 extractor。专用图数据库必须由节点/边规模、查询延迟和 SQLite 维护成本触发，不能由架构偏好触发。

## Round 9 — “自我进化”是否只是自动累积错误相关性？

### 攻击

产品经理：样本小、任务异质，成本/通过率变化很难归因；“自我进化”营销风险极高。

架构师：模型、Prompt、Context、Workflow、Router 同时变化会破坏可重复性。系统还可能针对 Harness 过拟合而不提升用户 Outcome。

### 最强反方结论

在 Outcome 定义和 telemetry 稳定前，任何自动学习都是噪声。自由文本 memory 更可能固化错误经验，而不是产生知识。

### 方案修正

- Phase 4 前只记录 Observation 和人工可见 Proposal；
- 只有版本化单变量策略允许实验；
- 评估以 accepted Outcome、返工、成本、时延和人工干预为指标，不以 Agent 自评分；
- historical replay 明确不能证明真实未来效果；
- 晋升必须有 baseline、样本、置信边界、独立审批和一键回滚。

### 证据与停止条件

在至少 50 个可比较 accepted Outcome 或一个经人工认可的代表性 eval suite 前，不启用自动实验晋升。若 uplift 无法跨任务类别复现，保持提案模式，不扩大学习 authority。

## Round 10 — Harness 收敛会不会削弱唯一真正可靠的护城河？

### 攻击

产品经理：用户购买的是可信完成，缩减 Harness 可能直接削弱差异化；迁目录不会创造价值。

架构师：把 checker 迁回产品 owner 会产生自证；而保留多语言独立实现虽昂贵，却是信任边界需要的 N-version 验证。

### 最强反方结论

“Harness 太大”不等于应该缩小。它可能只是产品安全需求本来就大。错误收敛会把独立 verifier 变成薄包装。

### 方案修正

- 不以 LOC 下降为目标，以独立性、重复语义和验证覆盖为指标；
- 安全、身份、授权、路径、canonical digest 和 completion evidence 保留独立 checker；
- 普通产品状态机只保留 conformance checker，不在 Harness 复制默认值和业务推进；
- scaffold/producer 迁出，但 Harness 保留真实 acceptance；
- 每次移除独立实现必须先做 threat model，说明该边界为何不需要 N-version 验证。

### 证据与停止条件

如果迁移导致 fault/adversarial coverage 下降、App 损坏时无法独立验收，立即回滚。若 Harness 继续增长但每个模块都有独立威胁或生态适配理由，则代码量本身不是失败。

## 2. 十轮后修订结论

### 2.1 保留的核心下注

- Forge Workspace 作为用户产品、ForgeOS 作为内部平台边界；
- Go Control / Rust Execution / Harness Verification 的责任分离；
- Runtime Action journal、CAS、Receipt 和确定性 Reconciler；
- CLI/TUI/App 共享 App Server 和 query projection；
- 自动推进、自我进化和图谱都采用分级、证据和停止条件。

### 2.2 必须缩小的范围

- R0 只交付一次受控 Agent 任务的可观测、恢复和验证；
- Platform Core v1 限制为六类真实使用 wire；
- Objective/Change 首期不做跨语言共享领域模型；
- 跨项目图谱延后，先做当前 Change 的确定性 impact；
- Evolution 在足够 Outcome 样本前保持 proposal-only；
- 不把双 SQLite、多语言或 Harness 目录视为不可推翻的教条。

### 2.3 Go / No-Go Gates

| Gate | Go 条件 | No-Go / Pivot 条件 |
|---|---|---|
| G0 Product | 真实用户能独立完成 R0 旅程 | 大多仍绕过产品直接用 CLI |
| G1 Observability | 关键 effect 有 coverage matrix 和 replay | 关键写/命令存在不可见绕过 |
| G2 Reliability | crash/restart 不重复 unsafe effect | uncertain 被频繁误判或需手工修库 |
| G3 Architecture | 跨语言边界缺陷成本可控 | >30% 工作量长期耗在协议漂移 |
| G4 Automation | lead time 降低且严重返工不升 | 自动推进扩大错误/成本 |
| G5 Graph | impact path 实际改变计划、减少漏改 | 低使用率、无质量改善 |
| G6 Evolution | 可复现 uplift + 独立批准/回滚 | 小样本、过拟合 Harness、无法归因 |

## 3. 最终对抗性裁决

当前方案值得继续，但只能批准到 **R0 可观测交付垂直切片**，不应一次批准完整自动推进、全局图谱和自我进化。

架构上最有价值的不是新增更多合同，而是证明以下闭环真实工作：

```text
Objective
→ scoped Attempt
→ observable Action
→ immutable Artifact
→ independent Verification
→ explainable Outcome
```

只有该闭环被真实用户采用，且崩溃、权限、成本和不可见边界得到量化证据后，才进入 WorkGraph 自动推进。Graph 和 Evolution 必须分别通过自己的产品价值 Gate，不能因为它们出现在愿景中就自动获得实施优先级。
