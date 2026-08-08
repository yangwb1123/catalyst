# 节点 15–16：运行与演化 SOP

> 生产运行权不由“Agent 很聪明”自动获得。ForgeOS 当前只准备和验证声明式交付；生产变更、事故处置和恢复由
> 外部 CI/operator 在独立授权下执行。本文定义目标组织职责与证据回流协议。

## 15 — Operations / SRE / Incident（运行与事件响应）

**目的。** 用用户可见 SLO 管理系统，快速发现、限制和恢复故障，并把生产事实安全地反馈给工程知识层。

**入口与输入。** Service Catalog、deployment/release identity、SLO/SLI、runtime topology、dashboards/alerts、runbooks、
dependency/owner/on-call、backup/DR、当前 risk/debt；release/recovery verification 或外部 operator action 路径还必须有外部
authority 的 `OperatorReceipt`。incident 可直接 intake，不等待一张新的 receipt；其后的 operator action 仍须回收真实 receipt。

**日常运行职能。**

1. 维护 service → owner → dependency → SLO → dashboard → alert → runbook 的可追踪关系；
2. 采集有界、脱敏、版本绑定的 logs/metrics/traces/events 和业务 KPI；
3. 以用户症状、错误预算和可行动性设计告警，治理重复/无 owner/无 runbook 的噪声；
4. 管理容量、配额、证书、依赖生命周期、备份、restore、DR 和定期演练；
5. 管理 feature flag、配置漂移、环境差异和已批准 maintenance；
6. 分析 toil、成本和人工操作，优先自动化重复且可安全验证的 runbook；
7. 用真实运行结果复验 Architecture/Data/Performance 的假设。

**事件响应职能。**

1. 按用户/数据/安全影响分级，建立 Incident Commander、Ops、Comms 和 scribe；
2. 冻结时间线、版本、配置、症状和已执行操作，事实与猜测分开；
3. 优先 containment 和安全恢复，不在事故中追求完美根因；
4. 每个操作绑定授权、范围、预期、停止/回滚条件和 receipt；
5. 安全、跨租户、数据完整性事件立即联动 Security/Data owner；
6. 恢复后验证用户路径、数据一致性、队列/backlog、SLO 和潜在二次影响；
7. 做无责 postmortem：触发、检测、响应、系统性原因、贡献因素、有效/无效控制；
8. 行动项必须有类型、owner、优先级、期限、验收和是否进入 debt/evolution。

**能力。** service-management、observability、SLO/error-budget、alerting、on-call、incident-command、
diagnostics、backup/restore/DR、capacity、config/flag-management、postmortem、toil-automation、FinOps。

**必交付物。** Service/Ownership Catalog、SLO Dashboard、Alert/Runbook、Operational Readiness Review、
`RuntimeObservation[]`、Incident Timeline、Recovery Evidence、Postmortem、Action Items。

**规则与门禁。**

- Runtime Evidence 绑定 environment、service version、release digest、query/window/sample/unit 和 freshness；
- 告警必须有 owner、用户影响、runbook 和 actionable threshold；
- break-glass 双人/外部批准、最小范围、短时、全审计、自动到期并强制复盘；
- 恢复以数据/业务校验为准，不以“进程启动”判成功；
- postmortem 不以个人失误作根因，行动项不以“加强注意”收尾；
- Error Budget 耗尽触发产品/可靠性权衡，不被总健康分抵消。
- G8 只应用于 release/recovery verification；incident intake 本身不因缺少新的 `OperatorReceipt` 被阻断。

**禁止与权限。** Agent 默认只读生产遥测且不得读取 secret/原始敏感正文。预批准 runbook 也必须由外部 operator
在精确 scope 执行；禁止无记录手工变更、永久保留应急绕过、事故中自动重试不确定外部 effect、用日志泄露 PII。

**升级。** Sev1/Sev2、数据损失/不一致、安全/隐私、跨租户、SLO 大幅失守、恢复失败或未知外部 effect 进入
`QUARANTINED/BLOCKED`，由有权人员裁决。

**退出与写回。** 事件只有在服务/数据验证、沟通完成、临时权限撤销、行动项有 owner 后关闭；写回真实 runtime
topology、failure mode、SLO baseline、incident/debt、被验证或推翻的假设，不把临时 workaround 写成架构事实。

## 16 — Meta Reflection / Evolution / Technical Debt（纠查与持续演化）

**目的。** 在关闭前二阶审计整条决策链，再把交付与生产反馈转化为可证伪、可排序的新 Change Intent；持续降低变化成本，
而不是自动追逐“更先进”架构。

**入口与输入。** Requirement/Decision、DecisionCapsule/Transactions/Attempts、Repository/Knowledge Graph snapshot、
acceptance/gate/review/QA evidence、defect/incident、SLO/cost/outcome、usage/user feedback、dependency lifecycle、ADR compliance、
debt portfolio、actual vs estimate。

**必须执行。**

1. 运行风险分级 Reflection：每次 R0、L2 R1、L3/L4 R2；Critic 第一遍只看目标/决策/current artifact/真实 Evidence，
   第二遍才把 Executor rationale 当低权重 Claim 查证；
2. 审计 goal alignment、需求/角色/异常完整性、所有 assumptions、原方案/新替代/反事实、over/under-design、architecture/
   data、failure/recovery、安全/隐私、性能/可靠性、维护/运维、UX 和未来变化成本；
3. 每个 required reflection finding 生成 owner/validation 与 `ReflectionRoutingReceipt`，需要返工时通过
   `CHANGES_REQUESTED(rework_target=...)`，发布后则开 Incident/New WorkIntent；
4. 对 code、dependencies、security、performance、reliability、architecture drift、tests、operability、documentation/
   knowledge 做证据扫描；
5. 计算 Software Health 的各维度分数、证据覆盖、unknown、hard blocker 和趋势；
6. 识别 debt principal/interest：变更时长、重复缺陷、touch/change coupling、incident、例外老化；
7. 检查 ADR 与当前代码/运行是否符合、是否被新事实淘汰、是否触发 revisit；
8. 归因重复 failure/rework，记录 delayed outcome/confounder/drift，不把相关性或代理指标直接写成根因；
9. 为每个问题形成 hypothesis、counterfactual、方案选项、预期收益、实验、guardrail、rollback；
10. 用价值、风险、紧迫性、利息、可逆性和学习价值排序，不以综合健康分掩盖红线；
11. 将候选分类为 fix、refactor、migration、experiment、research、policy/skill improvement；
12. 提交 Debt close、ADR/Claim supersede、stale Context、CandidateRule/Eval/Knowledge proposal；只有 Governance Kernel/
    对应权威的 applied/rejected receipt 能改变账本。所有实现候选都以新 WorkIntent 交回 00。

**能力。** meta-reflection、assumption-audit、adversarial/alternative review、evidence-scan、software-health、technical-debt、
architecture-fitness/drift、dependency-lifecycle、causal-analysis、experiment-design、counterfactual-reasoning、
portfolio-prioritization、knowledge-curation、process/skill-evaluation。

**必交付物。** `ReflectionReport`、`AssumptionAudit`、Decision Path Audit、`ReflectionRoutingReceipt[]`、
`SoftwareHealthSnapshot`、Trend/Drift Report、TechnicalDebt/ADR/Claim/Rule/Eval/Knowledge proposals、ADR Compliance、Knowledge
Freshness Report、`EvolutionCandidate[]`、prioritized Evolution Proposal、actual-vs-estimate lessons。

**健康度规则。** 维度至少覆盖 Architecture、Code Quality、Security/Privacy、Performance、Reliability、Test、Operability、
Documentation/Knowledge；权重按系统类型配置并总和为 100，Performance/Reliability 保留独立 verdict，即使某维权重为零也
保留证据结果。每次显示 evidence coverage、unknown、trend、hard blockers 和 `release_eligible`；Critical blocker 使 release
ineligible；全 N/A/UNKNOWN 不得绿色，不跨不同测量口径比较绝对分。

**技术债规则。** debt 必须包含 evidence、root cause、deferred reason、principal、interest signal、risk、owner、
due date/trigger、remediation、acceptance 和 waiver expiry。无 owner/触发器/验收不得进入 accepted；安全/数据完整性
Critical debt 不能用普通 backlog 无限延期；TODO 只有转为注册 debt 或立即解决两种合法去向。

**反事实规则。** 对“未来多公司/国际化/组织树”等至少比较：现在预留、以后兼容迁移、完全不支持三种路径及当前成本。
只有低成本且不会污染当前模型的 seam 可提前保留；不得仅因模型能想象未来就添加 `tenant_id`、事件总线或抽象层。

**禁止与权限。** Reflection/Evolution 默认观察/提案；Critic 不改代码/Truth/Rule/ADR/production，不因审美做大重写、
不把“800ms”直接推成“加 Redis”、不把预测收益当已实现事实、不借健康优化取得生产/代码写权。一次 outcome 不能自动
晋升规则；策略先经 shadow/champion-challenger、drift/reward-gaming guardrail 和 rollback。

**升级。** 健康持续下降、Critical dependency EOL、重复 incident、ADR/业务事实冲突或 debt 超阈值时升级给 CTO/
业务 owner；需要改变工程宪法时必须独立决策与人审。

**退出与写回。** 所有 required finding 有 RoutingReceipt，候选进入正式 backlog/RFC/ADR 或被有理由拒绝，且 proposal 均有
applied/rejected/deferred receipt 后才从 `REFLECTING→LEARNING→CLOSED`。实施必须新建 WorkIntent，从 00 重新做 impact/
authority，不得在 Evolution 原 Run 中隐式扩权。

## 跨节点持续闭环

```text
Release identity
  → Runtime observation window
  → SLO / incident / user outcome
  → R0/R1/R2 Reflection + Assumption/Decision Path audit
  → governed Claim validation or invalidation receipt
  → Health + Debt + ADR compliance
  → Evolution hypothesis and experiment
  → prioritized WorkIntent
  → fresh Impact / Design / Build / Review / Release
```

闭环的停止条件是“当前候选有明确处置、知识已更新、权限已撤销”，不是“继续 N 轮”。同一问题连续回流时，总控必须
检查需求、上下文、能力、门禁或系统约束是否错误，而不是只更换模型或增加 Prompt。
