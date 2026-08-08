# 节点 08–14：交付 SOP

> 交付不是 `plan → code → done`。每个增量都必须同时携带需求追踪、影响边、测试、运行可观测性、迁移/
> 回滚和知识更新；高风险实现、审查与批准必须职责分离。

## 08 — Planning / DAG Engineering（任务与依赖规划）

**目的。** 把已批准方案分解成可独立验收、可安全并行、可恢复的垂直增量。

**入口与输入。** 已批准的 Requirement/Product/UX/Domain/Architecture/Data/API 产物；同一设计摘要绑定的 final
`ChangeImpactReport`、`ChangeCostEstimate`、`RiskAssessment`、`AssessmentReceipt`；开放 findings/debt；团队/Agent 能力、
环境、预算和发布约束。

**必须执行。**

1. 以用户/业务可验证的垂直切片拆分，不按 Controller/Service/DAO 水平切片制造长尾集成；
2. 每项任务关联 requirement、decision、affected component、test 和 rollout；
3. 明确前置、资源/路径冲突、关键路径、可并行 wave、集成点与回滚 checkpoint；
4. 把未知项先拆成 time-boxed spike，并定义“要消除什么未知”，不把 spike 当实现；
5. 为数据/API 变化安排 expand/compatibility/backfill/contract 顺序；
6. 显式规划安全、测试、性能、遥测、文档、发布和知识写回任务；
7. 基于 final Cost/Risk 细化到执行包，记录 estimate delta/basis，并把 hazard 变成有 owner/期限/证据的 treatment task；
8. 为每节点定义 DoR、DoD、验证命令、owner、GrantRequest、预算和失败回流目标；
9. 识别同一文件/职责/状态的并行写冲突并串行化或设置整合 owner。

**能力。** work-breakdown、vertical-slicing、DAG-planning、critical-path、estimation、change-cost、risk-planning、
test/release planning、RACI、context-budgeting。

**必交付物。** `DeliveryPlan`、Execution DAG/waves、Work Packages、RACI、DoR/DoD、Test Matrix、
`EstimateRefinement`、`RiskTreatmentPlan`、`GrantRequestSet`、Rollout/Rollback task set；final assessment 保持只读引用。

**规则与门禁。** DAG 无环且引用可解析；每个 MUST 至少由任务和测试覆盖；Critical path 的 unknown 有 owner；成本是
范围+置信度，不伪造精确人日。08 不得把 Intake pre-scan 当 final assessment，也不得自签 Grant；非 Agent Kernel/PDP 在
plan finalization 校验 SoD/risk/policy 后签发 `CapabilityGrant` 和 `GrantIssuanceReceipt`。

**禁止与权限。** 只写计划/控制产物；不补设计、不写代码、不为并行度制造无意义拆分、不遗漏非编码工作。

**升级、退出与写回。** 关键依赖无 owner、成本超预算、并行冲突不可隔离、设计仍有 load-bearing unknown 时升级。
全部任务达到 DoR、Kernel issuance receipt 齐备并通过 G4 后交 09–14；记录估算依据，供 16 比较 actual。

## 09 — Development（实现）

**目的。** 在批准边界内实现最小完整增量，保持行为、兼容性、可测试性和可运维性。

**入口与输入。** Ready Work Package、精确 `ContextPackage`、Kernel/PDP 签发的 `CapabilityGrant`、contracts、当前代码快照、
测试/门禁命令。

**通用必须执行。**

1. 先复验任务、快照、权限、ADR 和 impact；发现范围漂移立即停止并回 00/08；
2. 复杂遗留行为先补 characterization test，关键规则先补失败测试；
3. 实现最小完整 slice，不夹带无关重写、依赖升级或全仓格式化；
4. 业务规则保持唯一 owner；IO、时钟、随机、网络、数据库等 effect 通过明确 seam；
5. 对错误、取消、超时、重试、并发、部分失败和资源上限做显式处理；
6. 保持当前与前一兼容窗口，迁移遵循批准顺序；
7. 增加必要 log/metric/trace/audit，敏感数据不进入诊断；
8. 写 unit/integration/contract/E2E 中与风险匹配的最小测试集；
9. 做自审并运行格式、lint、typecheck、test、build、security、architecture gates；
10. 生成 `ChangeManifest`：修改、接口、迁移、配置、证据、残余风险与新增 debt。

**后端细项。** application use case 编排；领域不变量；事务/锁/幂等；Repository/adapter；API/event；outbox/inbox；
资源限制。Controller 不直接访问数据库；同一事务内不执行无法回滚的外部副作用，除非有明确一致性协议。

**前端细项。** 按 feature/变化原因拆组件；query/server state 与 local UI state 分层；API adapter；表单/错误/全部
interaction state；无障碍和响应式；bundle/render/network 预算。前端权限只改善体验，授权必须由服务端执行。

**客户端/移动端细项。** 弱网/离线、同步冲突、应用版本错配、平台权限、电池/存储、敏感本地数据与迁移。

**集成/迁移细项。** Adapter/Anti-Corruption Layer；对账、幂等、重放、异常队列、速率与外部合同漂移；迁移脚本
dry-run、校验、暂停、恢复和回滚证据。

**能力。** backend/frontend/client engineering、secure-coding、testing、observability、migration、API integration、
concurrency、error-design、refactoring-small-steps。

**必交付物。** 代码、测试、必要文档/contract、migration/config delta、observability delta、`ChangeManifest`、
自审与 gate evidence。

**规则与门禁。** 分配范围内写；公共合同变化被声明；零新循环依赖；阈值触发职责审查；所有测试结果来自当前快照；
新增依赖有来源/许可证/漏洞证据；失败不得静默。

**禁止与权限。** 只写批准路径和开发/隔离环境；禁止生产/secret 权限、禁止自行改需求/架构、禁止关闭测试或降低
门禁通过、禁止以 TODO 隐藏必需正确性。

**升级、退出与写回。** 需求/ADR 冲突、破坏性合同、数据损失、供应链风险或 blast radius 扩大时升级。当前增量门禁
真实通过并有完整 manifest 后交 10–13；写回代码映射、验证事实和明确 debt。

## 10 — Code Review / Refactoring（独立代码审查）

**目的。** 从 fresh context 独立发现正确性、架构和维护风险；重构按变化原因迁移，不是机械拆文件。

**入口与输入。** Requirement/Decision refs、Change Impact、DeliveryPlan、diff/full affected code、测试与门禁原始证据、
相关债务和工程宪法。Reviewer 不接收实现者的“应该没问题”结论作为事实。

**必须执行。**

1. 先从需求、合同和 current code 重建预期行为，再审 diff；
2. 审正确性、边界、错误、并发、兼容、数据、资源、可观测性和安全；
3. 审依赖方向、模块所有权、领域规则位置、耦合/内聚/重复/可测试性；
4. 复核测试是否真正断言风险而非只跑行数；复跑 load-bearing checks；
5. 对疑似 God File 结合 size、复杂度、依赖、change coupling 和 responsibility map 判断；
6. 每个 finding 给 severity、evidence、impact、required/optional、fix suggestion 和 validation；
7. 需要重构时先冻结行为、建立 seam，按职责逐步抽取并保持公共 API，再删除旧路径；
8. 修复后只重跑受影响 review 加必要回归；原 reviewer 或新的 fresh reviewer 复验 finding closure；
9. 未解决 Blocker/Major 不得用综合得分抵消。

**能力。** code-review、static-analysis、architecture-conformance、complexity/coupling、test-review、refactoring、
pattern-selection、compatibility-review、evidence-assessment。

**必交付物。** `ReviewReport`、normalized findings、Verdict、Refactor Proposal/Responsibility Map（如需）、exception/debt
记录和复审 closure evidence。

**规则与门禁。** fresh-context；只读；finding 引用真实路径/符号/命令；个人风格不作阻断；阈值是审查触发器，
当前宪法硬门除外；实现者不能自批高风险变更。

**禁止与权限。** Reviewer 不改代码、不新增范围、不把文件拆分等同职责拆分、不因偏爱某设计模式要求重写。
Refactor implementer 需另获写授权。

**升级、退出与写回。** 公共合同/架构边界变化、无行为安全网、角色意见冲突或 Critical finding 时升级。所有必修 finding
关闭或被有权者限时接受后交 14；写回 smell、exception、debt 和新 fitness rule 候选。

## 11 — Security / Privacy / Compliance（安全与隐私审查）

**目的。** 从滥用者、数据主体和监管者视角验证变更，不把扫描器“无报告”当安全证明。

**入口与输入。** 数据流/信任边界、Requirement/Architecture/Data/API、diff/config/dependencies、Threat/Control Registry、
运行与扫描证据。

**必须执行。**

1. 更新 assets、actors、entry points、trust boundaries 和 STRIDE/abuse cases；
2. 验证 authentication、session/token、object/tenant-level authorization、RBAC/ABAC；
3. 检查 injection、XSS/CSRF/SSRF、反序列化、文件/路径、redirect、请求走私和资源滥用；
4. 检查 secret/key/certificate 生命周期、算法/参数、敏感日志和错误 oracle；
5. 做 data minimization、purpose/consent、retention/erasure/residency、DPIA 与审计追踪；
6. 检查依赖来源、SCA、SBOM、签名、构建 provenance 和许可证；
7. 设计/复验安全 unit/integration/negative/fuzz/DAST 测试；
8. 记录 control effectiveness、残余风险、exception owner/expiry 和 incident readiness。

**能力。** threat-modeling、OWASP、identity/access、secure-protocol、privacy-engineering、compliance mapping、
supply-chain、security-testing、incident-preparation。

**必交付物。** Threat Model delta、Security/Privacy Requirements、Control Matrix、scan/test evidence、finding/exception、
residual-risk verdict。

**规则与门禁。** 默认拒绝、最小权限、服务端授权；secret/PII 不进入上下文；Critical/High 发布前修复或由授权 risk owner
限时接受；exception 到期自动失效；测试环境和动作有明确授权。

**禁止与权限。** 默认只读代码/配置/扫描结果；不访问真实 secret、不对未授权生产系统做攻击性测试、不自创密码算法、
不永久化 break-glass。

**升级、退出与写回。** 已利用漏洞、跨租户、数据泄露或监管风险立即升级。阻断项关闭/接受后交 14；更新 threat/control/
vulnerability/exception knowledge。

## 12 — QA / Verification（独立质量验证）

**目的。** 用风险驱动的独立证据证明需求满足且回归风险可接受。

**入口与输入。** Requirement Traceability、Product/UX/API contracts、ChangeManifest、risk register、测试环境/数据契约、
实现者测试结果。

**必须执行。**

1. 按业务关键性、变化范围和失败成本设计 test strategy；
2. 覆盖 unit、integration、contract、E2E 的必要层级，避免全押 E2E；
3. 覆盖 happy、negative、boundary、permission、tenant、concurrency、timeout、recovery 和 rollback；
4. 验证 migration、旧版本兼容、feature flag、升级/降级和部分部署；
5. 做 exploratory、accessibility、security、performance/chaos 的职责交叉验证；
6. 管理合成/脱敏数据、fixture 隔离、环境 parity 和时间/随机可控性；
7. 缺陷包含最小复现、期望/实际、证据、severity、scope 和 regression test；
8. 识别 flaky tests，隔离不等于忽略，明确 owner 和修复期限；
9. 输出 requirement → test → result 的当前快照 trace。

**能力。** test-strategy、unit/integration/contract/E2E、exploratory、property/fuzz、concurrency、accessibility、
test-data/environment、defect-management、flaky-governance。

**必交付物。** `TestPlan`、Trace Matrix、automated/manual evidence、Defect Report、Flake Register、QA Verdict。

**规则与门禁。** Critical requirement 追踪 100%；覆盖率只是信号；测试断言行为；失败原样可见；load-bearing test 不能 N/A；
QA verdict 使用机器可解析契约。

**禁止与权限。** 只用批准测试环境和合成/脱敏数据；不只测 happy path、不因赶发布删/skip 失败测试、不共享污染 fixture、
不把旧报告当当前 PASS。

**升级、退出与写回。** 验收矛盾、严重缺陷无法复现、环境偏差或残余风险无 owner 时升级。必测通过且风险明确后交 14；
写回 regression suite、defect pattern 和环境事实。

## 13 — Performance / Reliability（性能与可靠性）

**目的。** 用代表性负载和故障证明系统满足预算，并阻止局部优化造成全局脆弱性。

**入口与输入。** SLO/NFR、workload profile、Architecture/Data/API、ChangeManifest、baseline/profiles、生产拓扑事实。

**必须执行。**

1. 定义用户可见 SLI/SLO、错误预算和分层 latency/resource budgets；
2. 建立当前基线与代表性 workload：数据量、并发、读写比、burst、依赖延迟和故障；
3. 用 profile/trace/query plan 找瓶颈，再做 load/stress/soak/capacity tests；
4. 检查 N+1、索引、分配、序列化、锁竞争、连接池、队列和 batch；
5. 检查 timeout、retry/backoff/jitter、idempotency、circuit breaker、bulkhead、backpressure 和降级；
6. 为 cache 定义 owner、key、TTL、失效、一致性、stampede 和 fallback；
7. 注入依赖超时/错误/重启/分区，验证恢复、数据正确性和告警；
8. 建容量/成本模型和 regression threshold，记录结果置信度与环境差异。

**能力。** SRE/SLO、benchmarking、profiling、database-performance、concurrency、load/capacity、resilience、
chaos/failure-mode、cost-performance。

**必交付物。** SLO/Budget、baseline/benchmark/profile、Capacity Model、Failure Matrix、optimization decision、
performance/reliability verdict 和 regression checks。

**规则与门禁。** 先测量后优化；p50/p95/p99 与错误/饱和/恢复同时报告；测试拓扑差异显式；无界并发/队列阻断；
非幂等操作不盲重试；微基准不冒充系统证明。

**禁止与权限。** 只在性能环境执行负载；生产默认只读观测，实验需明确授权和自动停止；不凭感觉加 cache/index/queue。

**升级、退出与写回。** SLO、容量或故障恢复不达标，或优化改变业务/一致性时升级。预算/恢复/告警有证据后交 14；
写回 baseline、bottleneck、capacity 和被证伪假设。

## 14 — Release / Change Management（发布与变更管理）

**目的。** 把同一不可变、可追溯制品以可停止、可验证、可回退的方式交给生产 operator。

**入口与输入。** 已批准 ChangeManifest、10–13 verdict、immutable artifacts、SBOM/provenance、migration、SLO/alerts、
风险接受与 human approval policy。

**必须执行。**

1. 验证源码/依赖/构建/制品摘要、签名、SBOM 和环境配置差异；
2. 生成 release manifest、版本/changelog、兼容窗口和 feature flag 状态；
3. 排列 schema/app/backfill/contract 顺序，验证 backup/restore 和 rollback/forward-fix；
4. 选择 canary/blue-green/progressive delivery，定义 cohort、观察窗口和自动停止条件；
5. 准备 preflight、deploy、postflight、smoke、business validation 和 data reconciliation；
6. 准备回滚 runbook、触发器、责任人、沟通和 incident handoff；
7. 汇总未解决风险并做 Go/No-Go；高风险由独立人审，不由 Release Agent 自证；
8. 交给外部 CI/operator 执行，收集真实结果并绑定原 release identity。

**能力。** release-management、artifact-provenance、CI/CD、migration-orchestration、progressive-delivery、
feature-flags、rollback、change-approval、production-validation。

**必交付物。** `ReleaseManifest`、signed/hash-bound package、Go/No-Go Checklist、Migration/Validation/Rollback
Runbooks、Release Notes、`ApprovalRecord`、operator handoff；发生外部执行时，还必须导入由外部 authority 签发的
`OperatorReceipt`。仅完成非生产 handoff 时不得伪造该 receipt。

**规则与门禁。** 同一制品逐级晋升；当前 source/artifact/review freshness；阻断 findings=0；rollback 可执行；观察指标和
停止阈值明确；审批身份/理由/scope/expiry 可审计。

**禁止与权限。** ForgeOS 当前边界只生成/验证声明式交付包；不持有生产凭证、不执行远程 deploy/rollback、不把外部
operator 收件冒充成功。任何手工生产变更必须由外部治理系统留痕。

**升级、退出与写回。** gate 失败、环境漂移、备份不可用、风险变化或批准过期时 No-Go。外部生产验证/明确失败或仅完成
非生产交付三者之一被准确记录后退出；写回 release identity、真实步骤、结果和异常。
