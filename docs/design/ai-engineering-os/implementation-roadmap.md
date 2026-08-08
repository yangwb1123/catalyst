# AI Engineering OS 实施路线图

> 目标是增量统一现有 ForgeOS，不创建第四套平行 Agent 系统。本文件中的未勾选项都是已采纳规划，不是当前能力。

## 1. 当前基线与缺口

ForgeOS 已有 13 张正式 `.agent/agents` 角色卡、9 张 `.agent/skills` 技能卡、7 条主工作流，以及 Go/Rust 控制与
执行原语。`docs/reviews` 另有产品、BA、UX、架构、后端、数据库、SRE、DevOps、QA、合规等丰富审查 lens；
`docs/ai-batch` 有离线确定性分类/评估能力。后三者尚未消费同一能力/证据契约。

| 节点 | 当前覆盖 | 已有基础 | 主要缺口 |
|---:|---|---|---|
| 00 Orchestrator | 强 | DAG、预算、checkpoint、回边、收敛、路由 | 图级影响/成本、通用 capability grant、typed context |
| 01 Requirement | 部分 | PM、PRD、confidence | 独立 BA schema、场景/规则/追踪硬契约 |
| 02 Product | 部分 | Discover/product design | journey/state/KPI/rollout 决策台账 |
| 03 UX/UI | 参考级 | review roles、UI baseline | 正式 role/skill/workflow/artifact/accessibility gate |
| 04 Domain | 部分 | architect bounded context、clean architecture | 独立 domain model/invariant/event/context contract |
| 05 Architecture | 强 | architect/CTO/ADR/human gate | ADR 机器元数据、current-state graph、compliance/freshness |
| 06 Data | 参考级 | database reviewer | 正式 schema/transaction/index/migration/retention contract |
| 07 API | 弱 | architecture 中关键接口 | contract registry、compatibility/idempotency/error/event gates |
| 08 Planning | 强 | task contract、DAG、Build/Evolve | impact/cost/risk 驱动的角色和权限派生 |
| 09 Development | 部分 | implementer + polyglot gates | backend/frontend/data specialization 与条件化工程规范 |
| 10 Review/Refactor | 部分 | fresh reviewer、回流、重构 skills | ordinary verdict 严格化、God/responsibility/模式决策证据 |
| 11 Security | 强 | STRIDE、security role/skill、secret/SCA | privacy/compliance 与开发期控制统一契约 |
| 12 QA | 强 | QA、testing、`qa_v1` fail-closed | risk-based trace/mutation/flake/environment registry |
| 13 Performance | 部分 | design review/budget | runtime telemetry → diagnose → verify loop |
| 14 Release | 强 | 声明式 package、freshness、human marker | 与通用 Grant/Claim/Graph 统一，仍保持外部执行边界 |
| 15 Operations | 参考级 | SRE/DevOps reviewer、production readiness |正式 Ops/Incident/RuntimeObservation feed |
| 16 Reflection/Evolution | 部分 | evidence scan、gap/roadmap/implement/review、三类 memory；Reflect 已部分映射 | 统一 Reflection/Assumption/Outcome/Routing 契约、深归因、受控学习 |

执行平面当前也只有本地 process、Docker/Firecracker 等基础，不存在 Device Registry、远程 Runner/Placement/Lease/Migration；
完整 coding-workspace exchange 与 Rust runtime OS sandbox 集成仍是缺口。AADM、Reflection 和 Device Fabric 文档都属于目标态，
不得从命令名称或已有 runner 推断已经交付。

最危险的三个先决缺口：

1. 现有 free-text Memory 不能作为企业事实账本，且旧省略 confidence 的语义不能继承到 confirmed fact；
2. 当前 Context 只覆盖有限 ROADMAP/ADR 标题/规则，未绑定影响子图、事实/假设、权限与遗漏；
3. 普通 reviewer/CTO verdict 的部分路径仍比严格 QA 更宽松，L3/L4 变更不能沿用 fail-open 行为。

## 2. 收敛策略

```text
Canonical capability/governance schemas
     ├── adapter → .agent/agents + .agent/skills + workflows
     ├── adapter → docs/reviews role lenses and prompts
     ├── input   → docs/ai-batch deterministic classification
     └── runtime → forge-core context/policy/orchestration + forge-runtime ledger
```

`.agent` 继续是可执行治理主干；`docs/reviews` 与 `docs/ai-batch` 在迁移完成前明确 advisory/standalone。不能直接把
它们现有上游特定引用升格为 ForgeOS 权威。运行状态继续归 `.forge/`/Hub；设计和审查产物继续归 `docs/discovery`、
`docs/design`、`docs/review`、`docs/release`、`docs/adr`。

## 3. Wave 0 — 蓝图与语义冻结

- [x] 采纳 capability-centric 模型：节点、角色、Skill、Agent instance 和权限分离；
- [x] 列出 00–16 全节点 SOP、工程规范、治理对象和规划期机读目录；
- [x] 明确 current/proposed 边界和现有三体系的收敛方向；
- [x] 采纳 AADM 决策 ABI、Meta Reflection 与 default-off Device Fabric 目标设计；
- [ ] 定义严格的 Governance Envelope、Evidence/Claim、ConstitutionRule、Grant、TransitionReceipt JSON Schema；
- [ ] 冻结 CognitiveAtom、DecisionTransaction、InteractionEvent、Capability invocation、Artifact/Execution receipt Kernel ABI；
- [ ] 定义 canonical bytes/digest domains、大小/数量上限、错误代码和版本迁移策略；
- [ ] 建跨 Go/Rust/Python golden 与 malformed/duplicate/unknown/oversize adversarial fixtures；
- [ ] 设计旧 memory/ADR 的只读导入：默认 `unverified_legacy`，绝不自动确认。

**完成判据。** 相同输入跨语言产生相同 canonical digest；未知/重复字段失败；无 Evidence 的 confirmed Fact、用 Assumption
满足 hard gate、过期 Grant/Approval、非法状态跳转全部被拒绝。

## 4. Wave 1 — Evidence、Claim、Context 与权限内核（最高优先）

- [ ] 实现 append-only Evidence/Claim ledger 与 current materialized view；
- [ ] 实现 KnowledgeClaim 状态、冲突、supersede、freshness 和 validation plan；
- [ ] 实现 `ContextPackage v1` 的选择、遗漏、redaction、token budget、digest 和 cache invalidation；
- [ ] 把当前 artifact provenance、Evolve locator 和 gate/test result 适配为 Evidence；
- [ ] 实现 `CapabilityGrant v1`、最小 effect vocabulary、preflight/postflight 与 audit receipts；
- [ ] 实现非 Agent Governance Kernel/PDP trust root、bootstrap GrantRequest、plan-finalization issuance 与 ApprovalRecord；
- [ ] 实现封闭 Transition 状态表、append-only ledger、非法边 enforcement、N/A applicability、rework/resume/quarantine recovery；
- [ ] 实现 KnowledgeUpdateProposal→Kernel validation→Receipt，所有节点默认只有 `knowledge.propose`；
- [ ] 实现最小 Capability Registry（ID/version/schema/effects/proofs/owner/implementations/tests），CLI/API 只做 adapter；
- [ ] 先以 shadow/report-only 实现 Atom compiler、TaskProfile、Rule activation/suppression、DiscretionEnvelope 和 DecisionCapsule；
- [ ] 实现带 read-version/CAS/idempotency/attempt/commit-abort receipt 的 DecisionTransaction 与有界 rolling controller；
- [ ] 先把本地调用适配为 ExecutionTarget/Attempt/ArtifactRef/EnvironmentDigest/Effect/Mobility，保持行为不变且远程模块 OFF；
- [ ] 将 L3/L4 required review/verdict 改为 fail-closed，保留低风险兼容策略；
- [ ] 让 approval/review/Agent output 绑定 source/context/policy/artifact digests；
- [ ] 增加查询：为什么这个事实成立、哪些假设开放、这个 Agent 看到了什么/没看到什么、被授予什么。

**完成判据。** Coding Agent 越路径、Reviewer 写代码、Migration 直接 apply production、Release 访问 credential 均被拒绝；
Agent 不能签发 Grant 或确认业务事实；源或 context 漂移使批准失效；不可信正文不能进入 instruction lane；非法状态跳转、
过期 approval、恢复重放已消费 Grant 和无 receipt 的 knowledge apply 全部拒绝。

## 5. Wave 2 — Knowledge Graph 与变更影响闭包

- [ ] 定义 stable node identity、edge taxonomy、extractor provenance 和 `GraphSnapshot v1`；
- [ ] 从模块/import/call、API/event schema、DB migration/schema、test、deployment、ADR/owner 建确定性 extractor；
- [ ] 记录 extractor coverage、unresolved edge、staleness；先静态/合同图，暂不以 embedding 当事实；
- [ ] 实现 seed → direct/transitive impact path traversal 和 evidence binding；
- [ ] 实现 compatibility、reversibility、migration、security/privacy、operation、test surfaces；
- [ ] 实现 Cost 与 Risk 独立量化及 materiality floor；
- [ ] 根据 impact/risk 派生最小角色、gates、human approval 和 DAG constraints；
- [ ] 图作为可重建索引；源变化做增量刷新，不能人工改图覆盖权威源。

**完成判据。** 见 §11 场景 A/B；缺图/陈旧边必须输出 UNKNOWN 并提升调查，不能报告 no impact；同输入报告 canonical
一致；篡改 path/evidence/graph digest 被拒绝。

## 6. Wave 3 — ADR、技术债、工程宪法与健康度

- [ ] 为新 ADR 增加 v2 frontmatter、owner/approver、claim/evidence、affected nodes、revisit/validation；
- [ ] 实现 Accepted ADR immutable + supersede 状态机和 Architecture Compliance；
- [ ] 合并 `.agent/DECISIONS` 与 ADR 的查询视图，但保留原始权威记录和迁移 provenance；
- [ ] 实现 `TechnicalDebtItem v1` 的 principal/interest/owner/due trigger/acceptance/waiver；
- [ ] 将 TODO/finding/incident/exception 显式转入 debt 或关闭，禁止无主 TODO；
- [ ] 把工程规则迁为 hard gate/review trigger/guidance，exception 限时且可追踪；
- [ ] 增加 cyclomatic/cognitive/duplication/change coupling/cohesion 等 adapter，工具缺失诚实 N/A；
- [ ] 实现 Health Snapshot、证据覆盖、unknown、trend、hard blocker 和 release eligibility。

**完成判据。** ADR 不能原地改已接受结论；Debt 无 owner/trigger/acceptance 不能 accepted；Critical blocker 无论总分多少
都 release-ineligible；全 N/A/UNKNOWN 不绿色；阈值触发职责分析而非自动生成碎文件。

## 7. 跨 Wave 能力包目录与节点编排

本节是跨 Wave 目录，不表示 38 包都在 Wave 4 实现；每包唯一建设顺序取
`capability-skill-map.v1.yml` 的 `implementation_wave`。Wave 4 重点是业务/交付 Skill 与 role/workflow adapter，Wave 1–3
的治理/图/ADR 包必须先按 mapping 完成，Wave 5–6 的审查/运行包不得提前冒充可用。

按可复用能力创建，不按角色名一对一复制。建议 backlog：

38 个可组合 Skill 包的 trigger/output/rule/automation/forbidden 规格见
[skill-specifications.md](skill-specifications.md)；140 个 lifecycle fine capabilities 的唯一 primary owner 与精确建设 Wave 见
[capability-skill-map.v1.yml](capability-skill-map.v1.yml)。下表只使用 canonical package 名称，不创建第二套命名。

| Cluster | Capability Skills |
|---|---|
| Governance | change-intake-orchestration、evidence-claim-management、project-snapshot、context-engineering、policy-authority、knowledge-graph-curation、change-impact-cost-risk、review-conflict-adjudication |
| Discovery/Experience | requirements-engineering、business-process-state、product-design、ux-research、information-interaction-design、design-system-accessibility |
| Domain/Architecture/Data/API | domain-modeling、architecture-tradeoff、adr-governance、distributed-reliability-design、data-modeling-transactions、data-migration-lifecycle、api-contract-design、event-integration-design |
| Delivery/Quality | delivery-planning、backend-engineering、frontend-client-engineering、secure-coding、observability-engineering、test-engineering、code-review、god-object-refactoring、pattern-selection |
| Assurance/Operations | security-privacy-compliance、performance-capacity、reliability-chaos、release-engineering、sre-readiness-incident、technical-debt-health、reflection-evolution |

每个 Skill 先写 host-independent Capability Contract，再生成宿主适配。实际可移植 Skill 使用精炼 `SKILL.md`，细节放
`references/`，确定性动作放经过测试的 `scripts/`，模板放 `assets/`，并生成 `agents/openai.yaml`；必须运行结构 validator
和 fresh-context forward test。ForgeOS `.agent/skills` adapter 继续满足现有 `harness/check.py` 引用规则。

- [ ] 建 capability registry schema、owner、version、trigger/not-applicable、input/output、rule/gate、permission；
- [ ] 校验 catalog fine capability → package primary owner 全覆盖且唯一，并从 mapping 生成 adapter 引用；
- [ ] 按 `implementation_wave` 逐 package 实现 Skill，不一次创建全部空壳；
- [ ] 从 `docs/reviews` 提炼通用规则，删除/隔离上游项目特有引用；
- [ ] 为缺失的 BA、UX、Domain、Data、API、Backend、Frontend、SRE、Evolution 添加正式 role adapter；
- [ ] 增量扩展现有 discover/design/review/build/deploy/evolve，不另建第二 DAG engine；
- [ ] `harness/check.py` 校验 capability↔role↔workflow↔artifact↔gate↔permission 交叉引用；
- [ ] 定义 CapabilityPluginManifest、publisher/signature、compatibility、permission ceiling、sandbox、upgrade/uninstall/rollback；
- [ ] 每个风险级别至少一组 routing/permission/review eval。

**完成判据。** 任务只加载相关 Skill/reference；角色卡不复制整套知识；触发/不适用条件可测；高风险角色分离；删除一个
capability 能被悬挂引用检查抓到；forward test 能输出规定 artifact 而非泛化建议。

## 8. Wave 5 — Review Loop、冲突裁决与发布闭环

- [ ] 实现 `ReviewCase`、independence proof、normalized finding 和 strict required verdict；
- [ ] Architecture/Security/Data/QA/Performance 按 impact 并行 review；
- [ ] Fix 只回流明确 owner，重跑 affected gates + regression，fresh re-review closure；
- [ ] 实现 constraint-first `ConflictResolution`，保存 dissent/revisit，不用多数票；
- [ ] Critical/High、事实争议、waiver、不可逆决策必须 human adjudication；
- [ ] 统一 release package 与 Claim/Grant/Approval/Graph identity，保持远程生产执行带外；
- [ ] loop/call/cost 有界；耗尽只能 BLOCKED/REJECTED，不能放行。

**完成判据。** 实现者不能成为 required reviewer；畸形/缺失 verdict fail-closed；Critical finding 不能被多数票或健康总分
覆盖；REQUEST_CHANGES 回到精确 task；过期批准和换包重放都被拒绝。

## 9. Wave 6 — 运行反馈与安全演化

- [ ] 冻结 ReflectionCase/Report、AssumptionAudit、RoutingReceipt、OutcomeObservation schema 与 adversarial fixtures；
- [ ] 适配现有 capsule/events/gates/review/QA/Evolve memory，先交付每次 Change 的 R0 report-only；
- [ ] 实现 `OBSERVING→REFLECTING→LEARNING→CLOSED` 与 required finding routing closure；
- [ ] L2 启用 R1 evidence-first Critic，L3/L4 启用独立 R2 adversarial/alternative/counterfactual + human escalation；
- [ ] 实现 RuntimeObservation 的 version/window/query/sample/unit/sensitivity/freshness；
- [ ] 接入 SLO/error budget、incident、user feedback、cost 和 release outcome；
- [ ] 实现 ADR compliance、Debt interest、Health trend 与重复 failure/rework 归因；
- [ ] 实现 EvolutionCandidate 的 hypothesis/counterfactual/experiment/guardrail/rollback；
- [ ] 实验结果验证/推翻 Claim，回写 debt/ADR/health；
- [ ] 只允许 policy 明确的低风险、可逆、可观察候选 auto-act；其它 propose-only；
- [ ] Incident/break-glass/外部 operator receipts 接入但不扩张 ForgeOS 生产权限。
- [ ] DecisionCapsule 关联 delayed Outcome；规则/策略只以 proposal→shadow→champion/challenger 晋升，带 drift/reward-gaming guardrail 和 rollback。

**完成判据。** `p95=800ms` 生成可证伪诊断计划而非直接“加 Redis”；全 N/A 不优化；高风险候选不自行实现；Critic 不能
直接改 Truth/Rule/ADR/code/production；每个 required finding 有 RoutingReceipt；发布结果可追溯到原 requirement/context/
artifact/approval，并能使旧假设 invalidated；一次 observation 不升级 hard rule。

## 10. Wave 7 — Default-off Device Fabric 与企业扩展（后置）

- [ ] F1 Inventory/Observe：静态 target、可选 SSH config import、identity proof-of-possession/owner consent、基础 attestation/
  TTL/key rotation（缺失则降低 ceiling）、只读分级 probe、Placement dry-run；无扫描/执行；
- [ ] F2 结构化 SSH MVP：低风险 TaskSpec、实证通过的风险隔离 floor（未受信/AI 代码至少 microVM-equivalent）、sandbox
  mount/network/process/resource/credential profile 与 egress PEP；task 不可见 Runner transport/mTLS credential；
  Staging/WorkspaceDelta/Publish/Apply/Attempt/Effect receipts、Artifact/env/code digest、timeout/cancel；禁止任意 shell；
- [ ] F3 Runner：主动注册/mTLS/heartbeat、reservation/lease/fencing、本地 lease watchdog/self-fence、credential/egress TTL≤lease、
  外部 effect adapter fencing/idempotency、structured Evidence、短期 Grant；
- [ ] F4 Scheduler/Controller：capability/data/model/topology placement、affinity、cordon/drain、backpressure/reconciliation；
- [ ] F5 Recovery/Migration：Checkpoint、LOST 对账、幂等/补偿；未知/不可逆 effect 不自动 retry/migrate；
- [ ] F6 Federation：P2P、多 control plane、Kubernetes/Nomad/Slurm、quota、cross-domain/federated attestation 最后实现；
- [ ] 多租户 ACL、PDP/OPA、跨域签名链、远程同步与审计查询；
- [ ] append-only event → materialized graph 的重建、并发、灾备和迁移；
- [ ] capability/approval 不可跨 tenant/run/task 重放；
- [ ] 在目标规模验证图查询、增量更新、存储增长和恢复；
- [ ] Web UI 展示 evidence/claim/impact/debt/health，不允许 UI 自造状态。

只有 Wave 0–6 的机器语义稳定、LocalExecutionTarget ABI 已通过 contract tests 后进入 F1；F2–F5 逐级显式启用，F6 与
多租户 ACL 最后。任何模式默认关闭，不能以 day-1 平台化拖慢单仓闭环。

## 11. 必须通过的业务验收场景

### A. 员工删除

输入“增加员工删除功能”后，Impact 必须沿图找到 employee data、backend use case、API/UI、RBAC、audit、retention/restore、
tests 和 migration/query filter；指出核心资产/PII 风险，比较 hard delete、soft delete、anonymize/retention，不能默认其中一个；
派生 Data/Security/QA/Release review。

### B. 订单状态

输入“修改订单状态”后，必须定位 Order Aggregate/invariant/state machine，并沿 API/event 到 Payment、Inventory、Audit 等
真实消费者；缺边标 UNKNOWN。`OrderStatusChanged` 只有在 registry/schema/outbox/idempotency/ordering/replay/observability
齐备时可作为已支持事实。

### C. God File

对 1200 行、35 method、18 dependency 的 OrderService，输出 metrics + Responsibility Map + change reasons + target seams +
characterization tests + incremental migration/rollback；不能只拆成两个 service 文件。OOP/DI/AOP/Event 逐项给适用/拒绝理由。

### D. 事实与假设

代码中已存在 `employee_account` 时，Agent 不得声称表叫 `users`；LDAP 同步若无证据必须为 Assumption，包含 owner、验证计划、
期限和错误影响，且不能满足 hard gate。

### E. 决策冲突

新方案提 MongoDB 而 Accepted ADR 选择 PostgreSQL 时，Context 必须注入 ADR 的 Context/替代/后果，不只标题；若驱动未变化
则遵守，若变化则开 superseding ADR，不静默推翻。

### F. 权限与 Review

Planner 不能写代码；Coding 不能改生产配置；Migration 不能 apply production；Reviewer 不能写/自批；Release 不能持 secret
或远程 deploy。任一 `finding_severity=BLOCKER`，或 policy-required `domain_risk=CRITICAL/HIGH` 均阻断；只有独立 closure，
或具备相应风险接受权威/期限/补偿控制的 ApprovalRecord 才能处置。

### G. 健康与演化

健康快照展示分项、证据覆盖、unknown、趋势和 blocker；Security Critical 使 release ineligible，即使总分 95。生产慢查询
只能触发 diagnosis/hypothesis/experiment，验证后才进入重构或索引 Change。

### H. AADM 决策与裁量

简单 UI 调整只激活 design-system/a11y/局部工程规则并抑制 DDD/分布式规则；高风险数据迁移可以广泛调查和提出候选，
但 execution autonomy 被压低。冲突硬合同输出 Minimal Unsatisfied Core；Transaction 无 read version/CAS/Grant/proof 时不能
commit/verified；world/context/policy 漂移按当前状态走合法边：可回流态进 CHANGES_REQUESTED，其余先 BLOCKED，
RELEASING/未知 external effect 进 QUARANTINED，不能跨状态跳转。

### I. Meta Reflection

一次 feature 完成后，R1 Critic 从 Evidence-first context 发现未验证假设、遗漏失败路径或 over-design，并为每个 required
finding 生成 Claim/Debt/Eval/Rule/ADR/New WorkIntent 的 RoutingReceipt。Critic 不能直接改代码/Truth/hard rule；一次成功
不能升级规则；delayed/confounded outcome 不得冒充因果。

### J. Device Fabric

OFF 模式必须是零 DNS/listener、SSH config import、registry mutation 和远程连接；INVENTORY 不连接、不探测。Enrollment
验证 PoP、owner consent、attestation/identity TTL 与 rotation；stale capability、不可信 target、数据驻留冲突和旧 fencing token
均拒绝。所有 remote executable 达到 verified isolation floor，未受信/AI/未知依赖/L2+ 至少 microVM-equivalent，Task 看不到
Runner/mTLS/SSH control credential。续租失败时本地 watchdog 先 self-fence，credential/egress TTL 不越过 lease，外部 effect
adapter 校验 fencing/idempotency。LOST/cancel timeout 不能自证停止或清理 workspace，未知 external effect 进入 quarantine；
fallback 重新放置/授权。远端只发布 immutable delta，`ApplyDelta` 单独 CAS/授权；egress PEP 默认拒绝。PASS 绑定
code/env/input/target/attempt；生产结果仅接受外部签名 `OperatorReceipt` 并经 G8，Fabric 不能取得生产权限或改变技术方案。

## 12. 每个 Wave 的统一 Definition of Done

- schema 严格、版本化、大小有界、canonical、content-addressed；
- positive/negative/adversarial/crash/replay/concurrency 测试与当前风险相称；
- current vs target 文档一致，无未接线声明冒充 DONE；
- 权限默认拒绝、输入/输出/freshness 可审计；
- full gates 真实运行，缺工具诚实 N/A；
- fresh-context independent review 无未解决 Blocker/Major；
- Roadmap、功能需求审计、ADR、debt/knowledge 同步更新。
