# Capability Skill 规格清单

> 本清单把节点中的细粒度 capability 收敛成可维护的 Skill 包。一个 Skill 可供多个角色/节点复用；不是每个表格行都
> 创建永久 Agent。`P0/P1/P2` 是能力重要度/运行使用分层，**不是实施全序**；精确建设 Wave 与细粒度 ownership 以
> [capability-skill-map.v1.yml](capability-skill-map.v1.yml) 为唯一机读权威；未实现项均为规划。

## 1. 每个 Skill 的强制结构

每个实际 Skill 必须包含以下内容，缺一不能标 production-ready：

1. **职责与触发。** 解决什么决策；正向 trigger、明确 not-applicable、支持的语言/框架/lifecycle/risk；
2. **输入契约。** 必需 artifact/claim/evidence/context/grant，缺失时停止还是降级；
3. **执行 SOP。** 有序步骤、分支条件、反事实检查、失败关闭和升级点；
4. **输出契约。** versioned artifact schema、状态、source/context/evidence binding、下游 owner；
5. **规则。** hard gate、review trigger、guidance 分开，含适用/拒绝理由；
6. **禁止与权限。** effect/path/tool/network/secret/environment/budget 边界；
7. **自动化。** 可确定的 scripts、参数/输出/错误码/上限及 positive/negative/adversarial tests；
8. **知识与示例。** 直接相关 references、模板/assets、正确/失败/边界案例；渐进加载而非主 Prompt 全塞；
9. **验收。** Review Checklist、eval fixtures、fresh-context forward test、退出/交接/知识写回。

可移植宿主包使用精炼 `SKILL.md`、`agents/openai.yaml` 和按需 `references/scripts/assets`；ForgeOS adapter 输出
`.agent/skills/<name>.md` 并进入 capability↔role↔workflow↔gate 引用校验。脚本必须实跑，模板不能冒充实现。

实际 Codex `SKILL.md` frontmatter **只能**包含 `name` 与 `description`；`description` 必须完整写出何时触发、何时不适用，
因为正文只在触发后加载。正文使用命令式语气，保持短而可执行；细节下沉到直接引用的 references，避免多层引用链。
生成 UI metadata 时再写 `agents/openai.yaml`，不能把触发条件只藏在该文件或正文中。

## 2. P0 — Wave 1 治理与上下文底座

| Skill 包 | 触发与核心职能 | 必交付物 | 规则/自动化 | 主要禁止项 |
|---|---|---|---|---|
| `change-intake-orchestration` | 任一新请求/告警/debt；归一化 intent、materiality、DAG、预算、恢复和职责分离 | WorkIntent、ExecutionPlan、Run journal | DAG/cross-ref/authority validator；以证据满足判停 | 写产品代码、自授予、自批准 |
| `evidence-claim-management` | 新事实/判断/假设/冲突；采集 Evidence、类型化 Claim、验证/失效/supersede | EvidenceRecord、KnowledgeClaim、conflict set | strict schema/canonical/freshness；Assumption 不过 hard gate | 省略 confidence=1、文档存在=实现完成 |
| `project-snapshot` | Run 开始、resume、source/config/deployment 变化 | Project/Graph source snapshot、coverage manifest | path/type/size/symlink/digest/extractor version 检查 | 把 Git commit 单独当产品状态、读 secret |
| `context-engineering` | 每个 Agent 节点；从 impact/claims/contracts/permissions 选择有界 context | ContextPackage、selection/omission/redaction manifest | token budget、digest cache invalidation、stale/contested filter | 只给代码不提供决策、把 secret 放入 Prompt |
| `policy-authority` | 节点将执行 read/write/process/network/migration/release effect | Constitution decision、CapabilityGrant、receipts | default deny、exact scope、expiry、pre/postflight、SoD | 角色名隐含权限、transfer grant、永久 waiver |

## 3. P1 — Wave 2/5 治理智能

| Skill 包 | 触发与核心职能 | 必交付物 | 规则/自动化 | 主要禁止项 |
|---|---|---|---|---|
| `knowledge-graph-curation` | Wave 2；源变化、extractor/coverage/stale/conflict | GraphSnapshot/delta、unresolved/staleness report | rebuildable/provenance/stable IDs；missing→UNKNOWN | 人工改图覆盖源、embedding=事实、无期限 stale |
| `change-impact-cost-risk` | Wave 2；Graph ready 后的行为、合同、数据、权限、依赖或运行变更 | Impact paths、Cost range、Risk hazards、required roles/gates | graph traversal、compatibility/staleness；最高风险 floor | 用 intake pre-scan 冒充 final、缺边报无影响 |
| `review-conflict-adjudication` | 多 reviewer finding 或方案冲突 | ReviewCase、ConflictResolution、ADR/plan refs | independence、strict verdict、constraint-first precedence | 多数票覆盖红线、聊天结论不落记录 |

## 4. P1 — 需求、产品与体验

| Skill 包 | 触发与核心职能 | 必交付物 | 规则/自动化 | 主要禁止项 |
|---|---|---|---|---|
| `requirements-engineering` | Idea/feature/bug 行为不清；Actors、场景、规则、NFR、GWT、追踪 | RequirementBaseline、Rule Catalog、Acceptance、Trace Matrix | MUST→acceptance 100%、术语/ID/schema validator | 技术方案先行、猜测补业务 |
| `business-process-state` | 审批、订单、权限、超时、补偿等有生命周期 | As-Is/To-Be、state/transition/exception/permission matrix | unreachable/illegal transition、rule priority review | 只画 happy path、状态存在代码多处 |
| `product-design` | 需求已基线；JTBD、scope、MVP、指标、rollout/deprecation | ProductBrief、Feature Map、KPI/guardrail、Rollout | 每 feature→value/metric；scope change 回流 | 竞品即价值、未来性镀金 |
| `ux-research` | 用户/场景/频率/痛点缺证据 | Research Plan/Summary、persona/task evidence、research gaps | source/sample/method/limitation；事实与推断分开 | 伪造访谈、用通用画像代替研究 |
| `information-interaction-design` | 新/改用户流程、导航、表单、高风险操作 | IA、task flow、prototype、Interaction State Matrix | loading/empty/error/offline/permission/recovery completeness | 静态稿无行为、暗中改业务规则 |
| `design-system-accessibility` | 新 UI pattern/component/token 或设计验收 | token/component delta、a11y/i18n matrix、Design QA | keyboard/focus/semantics/contrast/RTL checks | 只用颜色表达、重复造组件、牺牲可访问性 |

## 5. P1 — 领域、架构、数据与接口

| Skill 包 | 触发与核心职能 | 必交付物 | 规则/自动化 | 主要禁止项 |
|---|---|---|---|---|
| `domain-modeling` | 复杂规则、术语/owner 冲突、一致性边界 | Language、Context Map、Aggregate/Invariant/Command/Event catalogs | rule→owner/test trace；aggregate by invariant | 表=aggregate、CRUD 套 DDD |
| `architecture-tradeoff` | 跨模块/NFR/infra/边界/演进选择 | current/target C4、Option Matrix、fitness、migration | 至少比较维持/最小/结构方案；driver evidence | 流行技术选型、big-bang rewrite |
| `adr-governance` | 非显然权衡、跨 Sprint 决策、既有 ADR 冲突 | ADR v2、compliance/revisit/supersede | Accepted immutable、frontmatter/cross-ref/freshness check | 原地改历史、ADR 存在=代码符合 |
| `distributed-reliability-design` | network/queue/cache/multi-replica/partition | failure/consistency/retry/idempotency matrix、SLO allocation | timeout/backoff/jitter/backpressure/failure tests | 无界 retry、声称 remote exactly-once |
| `data-modeling-transactions` | 新/改实体、查询、事务、租户/保留 | Schema/Data Dictionary、Transaction/Query/Index Matrix | constraint/schema/query-plan/lock-order review | Service 散 SQL、无证据索引、默认硬删除 |
| `data-migration-lifecycle` | schema/backfill/retention/erasure/restore 变化 | expand-backfill-contract、validation、rollback/recovery | old/new compatibility、interrupt/replay/large-data tests | 先删列、无界锁表、生产直跑 |
| `api-contract-design` | 新/改 REST/GraphQL/RPC/public SDK | OpenAPI/Proto、error/auth/idempotency、consumer compatibility | schema lint/breaking check/contract tests | 暴露表、200 包错误、无限分页 |
| `event-integration-design` | 多自治消费者、时间解耦、外部 webhook/event | AsyncAPI/event schema、consumer registry、delivery/replay contract | outbox/inbox、dedup、order、DLQ、trace/reconcile | 三个副作用就上总线、无证据 exactly-once |

## 6. P1 — 规划、实现与工程质量

| Skill 包 | 触发与核心职能 | 必交付物 | 规则/自动化 | 主要禁止项 |
|---|---|---|---|---|
| `delivery-planning` | Design ready；vertical slice、DAG/waves、DoR/DoD、成本/测试/发布 | DeliveryPlan、Work Packages、RACI、Test Matrix | DAG/trace/grant validator；range+confidence estimate | 只规划编码、伪精确工时、写冲突并行 |
| `backend-engineering` | 后端 use case/domain/data/API change | application/domain/adapter code、tests、manifest | transaction/effect/error/concurrency/limit checks | Controller→DB、业务层 new client、长事务外调 |
| `frontend-client-engineering` | Web/mobile/client behavior | feature components、state/API adapter、a11y/perf tests | all states、contract types、bundle/render/network budget | 前端权限当授权、God Page、碎片组件 |
| `secure-coding` | 任一输入/身份/数据/外部 effect 代码 | security tests、control delta、safe error/log behavior | input/path/size/auth/secret/SCA checks | custom crypto、secret/log PII、静默降级 |
| `observability-engineering` | 新 use case/dependency/failure mode | logs/metrics/traces/audit、dashboard/alert delta | correlation、cardinality、sensitivity、actionability | 只打日志、泄密、高基数无界标签 |
| `test-engineering` | 新行为/bug/refactor/migration | risk test plan、unit/integration/contract/E2E、trace/defect | current result、negative/boundary/race/fuzz、flake registry | 覆盖率=正确、删失败测试、旧 PASS |
| `code-review` | 任一实现；fresh correctness/architecture/test/compatibility review | normalized findings、strict verdict、closure | evidence/severity/required validation、independence | 实现者自批、风格阻断、Reviewer 写代码 |
| `god-object-refactoring` | 体积、复杂度、职责、change coupling、effect/test pain | metrics、Responsibility Map、seams、incremental migration | characterization + per-step gates + API preservation | 按行拆文件、无安全网 big bang |
| `pattern-selection` | 分支、状态、依赖、横切、模型边界、异步压力 | option/selected/rejected rationale、tests | OOP/DI/AOP/DDD/Event 条件矩阵 | interface-per-class、模式按数量机械触发 |

## 7. P1 — 安全、性能、可靠性与发布

| Skill 包 | 触发与核心职能 | 必交付物 | 规则/自动化 | 主要禁止项 |
|---|---|---|---|---|
| `security-privacy-compliance` | auth/tenant/PII/payment/public input/dependency/法规 | Threat Model、Control/Privacy Matrix、findings/exceptions | STRIDE/OWASP/SAST/SCA/secret/privacy tests；expiry | 未授权渗透、读真实 secret、永久 break-glass |
| `performance-capacity` | SLO/scale/query/concurrency/cache/queue 影响 | baseline/profile/benchmark、capacity、regression | representative load、p50/p95/p99/error/saturation/cost | 感觉优化、微基准冒充系统、生产乱压测 |
| `reliability-chaos` | dependency failure、retry、recovery、DR | Failure Matrix、fault tests、RTO/RPO evidence | idempotency/backpressure/degrade/restore validation | 非幂等盲重试、无界队列、无回滚实验 |
| `release-engineering` | 全 review 通过，准备交付 | immutable manifest/package、Go/No-Go、migration/rollback/runbook | provenance/freshness/signature/postflight/approval | 持生产 secret、远程 deploy、自证成功 |

## 8. P2 — 运行与持续演化

| Skill 包 | 触发与核心职能 | 必交付物 | 规则/自动化 | 主要禁止项 |
|---|---|---|---|---|
| `sre-readiness-incident` | 上线准备、SLO 告警、Sev event、恢复/复盘 | Service/SLO/alert/runbook、timeline、recovery、postmortem | actionable alert、release-bound evidence、break-glass audit | 故障归责个人、无记录变更、临时绕过永久化 |
| `technical-debt-health` | finding/TODO/incident/drift 或周期扫描 | Debt Register、Health Snapshot、trend/blockers | owner/interest/trigger/acceptance；N/A 不绿色 | 综合分盖 Critical、无主 TODO、审美债 |
| `reflection-evolution` | 每次关闭前纠查；运行/用户/成本/质量趋势需要改善 | Reflection/Assumption Audit、hypothesis/counterfactual/options、experiment/guardrail/rollback | evidence-first Critic；result validates/invalidates claim；proposal-only learning | `800ms→Redis` 直觉跳跃、自评自改硬规则、高风险 auto-act |

## 9. Skill Review Checklist

每个 Skill 的 Reviewer 至少回答：

- trigger 与 not-applicable 是否能阻止误触发和过度设计？
- 输入缺失、Evidence stale、Context truncated、Grant 不足时是否明确停止/降级？
- 输出是否有 schema、source/context/evidence binding、owner、状态和大小上限？
- hard gate、review trigger、guidance 是否分开？数值是否可配置且解释例外？
- 是否把事实、推理、假设、建议分开？是否会把文档/模型自报冒充实现？
- 权限是否 default deny、exact scope、time/budget bound、不可转让并有 postflight？
- scripts 是否确定、可移植、真实运行并有错误/边界/对抗 tests？
- references 是否直接相关且渐进加载？示例是否包含失败与反例？
- 是否定义升级、退出、交接、memory/KG/debt 更新？
- fresh-context forward test 是否在至少一个正常和一个危险场景下产生合规产物？

只有 Checklist、validator、脚本测试和 forward eval 同时通过，Skill 才从 `planned → experimental → adopted →
enforced`；被替代 Skill 保留版本/迁移和 supersede 关系，不能直接消失导致历史 Run 无法解释。
