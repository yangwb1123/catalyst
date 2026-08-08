# 节点 00–07：探索与设计 SOP

> 本文定义目标态职责。每个节点都受 [统一治理契约](governance-contracts.md) 约束；产物没有证据、版本、
> 状态和来源标识时，只能算草案。

## 00 — Intake & Orchestration（研发总控）

**目的。** 把一个自然语言请求变成有边界、可恢复、可审计的 Change Run；选择最小必要能力集，而不是让所有
Agent 无差别参与。

**入口与输入。** 用户意图；当前 `ProjectSnapshot`；有效工程宪法、ADR、开放技术债、运行健康与权限策略；
已有需求/合同；代码、数据和部署拓扑索引。

**必须执行。**

1. 识别工作类型：问答、缺陷、小变更、功能、重构、迁移、事件处置或架构演进；
2. 归一化目标、范围、Non-Goals、成功信号、期限和外部约束，生成 `WorkIntent`；
3. 冻结代码/配置/知识版本，区分事实、假设、未知与用户已批准决策；
4. 做低成本 `ImpactPreScan`，只用于 materiality、路由与初始权限，不得满足 final design gate；
5. 按影响选择 01–16 节点，构建有向无环执行图、独立审查点和 Human Gate；
6. 为每节点装配最小 `ContextPackage`、预算和路径/工具/effect 权限；
7. 持久化 Run journal、节点状态、artifact manifest、失败原因和恢复点；
8. 每次回流重算受影响节点，不机械重跑全部流程；以证据满足而非轮数判停；
9. 适用的 02–07 产出冻结设计草案后重新进入 Assessment Join，由只读 impact assessor 生成 final
   `ChangeImpactReport`、`ChangeCostEstimate`、`RiskAssessment`、`AssessmentReceipt`；L3/L4 assessor 与主要设计者分离。

**能力。** work-intake、project-snapshot、change-impact-analysis、risk-triage、context-engineering、DAG planning、
policy evaluation、budgeting、provenance、convergence control。

**必交付物。** `WorkIntent`、`ProjectSnapshotRef`、`ImpactPreScan`、`ExecutionPlan`、`ContextPackage[]`、
`GrantRequest[]`、Run journal，以及 Assessment Join 的四项 final 产物；只有非 Agent Governance Kernel/PDP 可签发
`CapabilityGrant[]`。

**规则与门禁。** 所有节点输入可解析；DAG 无环；高/关键风险有独立审查与人审；写节点与批准节点分离；缓存
绑定全部载重输入摘要；未知的 load-bearing 条件失败关闭。

**禁止与权限。** 总控只写控制状态和规划产物；不写产品代码、不替代专业 verdict、不自授予权限、不把调度成功
冒充业务成功。涉及生产、不可逆数据变更、真实费用、外部消息或范围扩张时升级给人。

**退出与写回。** 每个计划节点都有 owner、输入、输出、权限、预算和退出条件后进入执行；写回 Run、影响边和
上下文选择证据。

## 01 — Requirement / Business Analysis（需求分析）

**目的。** 把目标转成可验证的业务需求基线，消除“开发自行补全业务”的灰区。

**入口与输入。** 已确认的 `WorkIntent`、用户/干系人信息、业务词汇、现有行为与合同、研究证据、已知约束。

**必须执行。**

1. 明确目标用户/Actor、待解决问题、业务价值、成功指标和不做什么；
2. 建立统一术语与概念歧义表；
3. 列正常、异常、边界、并发、权限、撤销、超时和恢复场景；
4. 提取业务规则、计算规则、时效、状态转移、不变量和例外优先级；
5. 标记数据分类、保留/删除、审计、地域和合规需求；
6. 把性能、可用性、容量、RTO/RPO、安全、可访问性写成可测 NFR；
7. 用 Given/When/Then 编写验收标准，并建立 requirement → rule → acceptance 可追踪关系；
8. 将未确认内容记录为问题或假设，给出验证 owner、期限及错误成本；
9. 评估需求完整度和置信度，未达阈值时停止下游承诺。

**能力。** requirements-elicitation、business-analysis、scenario-modeling、state-modeling、rule-catalog、NFR、
acceptance-criteria、traceability、ambiguity-analysis。

**必交付物。** `RequirementBaseline`、Actor/Persona 表、场景与状态模型、`BusinessRuleCatalog`、NFR budget、
验收标准、Glossary、Open Questions/Assumptions、Traceability Matrix。

**规则与门禁。** 每个 MUST 有验收证据；每条规则有唯一 ID；冲突规则有优先级/裁决人；术语一致；NFR 可测；
外部事实可追溯。需求不足时返回用户/研究，不以模型猜测填空。

**禁止与权限。** 只写需求/发现产物；不定技术栈、不设计表或接口、不把“常见做法”升格为需求、不伪造用户研究。

**升级、退出与写回。** 法律/合规、跨团队合同、不可逆业务规则或成功指标缺失时升级。MUST/Non-Goals/验收和
重大未知均明确后交 02–07；把确认事实、业务概念和假设写入知识层。

## 02 — Product（产品设计）

**目的。** 在业务需求上形成有价值、可分期、可运营的产品方案。

**入口与输入。** Requirement Baseline、研究证据、产品战略、现有功能、使用数据、商业与支持约束。

**必须执行。**

1. 建立 JTBD、用户故事、价值假设和目标/反目标；
2. 设计端到端用户旅程、服务蓝图和产品状态机；
3. 用价值、风险、依赖和学习速度做 MVP/后续分层，不只按实现容易度排序；
4. 定义功能规则、角色能力矩阵、通知/审批/撤销/导入导出等产品行为；
5. 定义成功 KPI、护栏指标、埋点需求与判定窗口；
6. 规划 feature flag、试点、灰度、迁移、弃用、支持和反馈渠道；
7. 对未来多租户、国际化、组织树等做反事实检查，但只把有证据的低成本扩展点纳入范围；
8. 形成 Epic → Feature → Story 映射，并保持与 Requirement ID 可追踪。

**能力。** product-discovery、JTBD、journey-mapping、prioritization、state-machine、product-analytics、
experimentation、rollout-design、scope-management。

**必交付物。** `ProductBrief`、Feature Map、User Journey、Product State Machine、角色能力矩阵、指标计划、
MVP/Advanced 分层、rollout/deprecation 计划。

**规则与门禁。** 每项功能关联用户问题和成功信号；MVP 闭合一个真实旅程；指标不诱导有害行为；未来性设计有
成本上限与触发器；范围变化回到 01/00 重新评估。

**禁止与权限。** 不选技术实现、不把假想未来需求变成 day-1 复杂度、不以竞品存在替代本产品价值证据。

**退出与写回。** 产品状态、优先级、权限与指标可被 UX/Domain/Architecture 消费后退出；写回已批准产品决策与
被拒选项，不覆盖业务事实。

## 03 — UX/UI Design（体验与界面设计）

**目的。** 让目标用户在真实环境中高效、可理解、可访问地完成任务；不是先生成漂亮页面再补状态。

**入口与输入。** Product Brief、Persona/场景、任务频率、设备和环境约束、现有 Design System、品牌与无障碍目标。

**必须执行。**

1. 只用真实研究/运行数据建立用户画像、痛点和操作频率；缺证据时标为 research gap；
2. 设计信息架构、导航、内容层级、搜索/筛选和对象命名；
3. 为关键任务画 flow、wireframe/prototype，优化高频路径、认知负荷和错误恢复；
4. 定义页面/组件的 default、hover、focus、disabled、loading、empty、error、success、offline、partial states；
5. 对删除、支付、授权等高风险动作设计确认、原因、撤销、权限与审计反馈；
6. 规范 Grid、对齐、间距、排版、颜色、图标、动效和响应式断点，并复用 token/component；
7. 检查键盘、焦点、对比度、语义、屏幕阅读、缩放、动态内容和 reduced-motion；
8. 处理国际化、时区、数字/货币/姓名、长文本与 RTL；
9. 用可用性测试或可复验 heuristic review 验证原型并记录问题。

**能力。** user-research、information-architecture、interaction-design、content-design、visual-design、
design-system、responsive-design、accessibility、usability-testing、design-handoff。

**必交付物。** Research Summary、IA/Sitemap、Task Flow、Screen Inventory、wireframe/prototype、Interaction
State Matrix、Design Tokens/Component Delta、Accessibility Matrix、UX acceptance、handoff annotations。

**规则与门禁。** 所有关键页面状态齐全；关键流程可键盘完成；组件优先复用；高频 ERP 型任务记录点击/键盘预算；
视觉选择不覆盖可用性和无障碍；研究事实与设计推断分层。

**禁止与权限。** 设计节点不写生产 UI 代码、不伪造访谈、不自行改变业务规则、不用像素稿隐藏未决交互。

**退出与写回。** 可交付给 07/09 的交互与状态契约齐备、重大可用性风险解决后退出；写回 persona/任务事实、设计
决策和 Design System 变更。

## 04 — Domain（领域建模）

**目的。** 把业务语言和不变量变成清晰的所有权边界，防止 `UserService`、`OrderService` 吞并所有职责。

**入口与输入。** Requirement Baseline、Business Rule Catalog、Product State Machine、现有领域模型和数据事实。

**必须执行。**

1. 建立统一语言，识别核心/支撑/通用子域；
2. 识别 Bounded Context，并画 upstream/downstream、ACL、shared-kernel 等 Context Map；
3. 定义 Entity、Value Object、Aggregate、生命周期、不变量和事务边界；
4. 把行为放回拥有数据与规则的领域对象/服务，区分 domain service 与 application orchestration；
5. 定义 Command、Domain Event、Policy、Saga/补偿候选及业务幂等键；
6. 明确哪些一致性必须同步原子完成，哪些副作用可异步；
7. 识别跨上下文翻译、防腐层和主数据所有者；
8. 用业务实例、反例和状态迁移测试模型；
9. 评估是否值得使用 DDD 战术模式；简单 CRUD 保持简单。

**能力。** domain-analysis、DDD、event-storming、invariant-modeling、context-mapping、state-machine、
consistency-design、domain-testing。

**必交付物。** Ubiquitous Language、Context Map、Aggregate Catalog、Invariant/Command/Event Catalog、领域状态图、
Context ownership 与 consistency matrix。

**规则与门禁。** 一个业务事实有唯一权威 owner；聚合只保护必要不变量；事件使用业务过去式；跨 context 不引用内部
实现；事件不是逃避事务设计的手段。

**禁止与权限。** 不因文件数量强行造 Aggregate、不为 CRUD 套满 DDD、不选择中间件、不让 ORM schema 反向定义领域。

**退出与写回。** 所有关键规则/状态映射到 owner 与测试场景，冲突由业务裁决后交 05–07；写回领域概念、边界与不变量。

## 05 — Architecture（架构设计）

**目的。** 在当前系统和真实约束下选择可演进方案，控制变化成本，而不是追求抽象完美。

**入口与输入。** 01–04 产物、当前代码/部署拓扑、ADR、技术债、SLO、安全约束、团队与生命周期事实。

**必须执行。**

1. 重建 current-state C4、模块/依赖/数据流和已知妥协，标出禁改区与替换计划；
2. 把驱动因素按业务、质量属性、约束和未知分类；
3. 至少比较“维持/最小改动/结构性改动”等可行选项及成本、风险和可逆性；
4. 定义模块边界、依赖方向、ports/adapters、同步/异步交互和故障边界；
5. 设计安全、可观测、配置、错误、事务和审计等横切策略；
6. 给出 lifecycle-aware 方案与明确演进触发器，不按假想峰值 day-1 微服务化；
7. 设计兼容、迁移、回滚、降级、容量和运营模型；
8. 对非显然权衡写 ADR，记录备选和拒绝理由；
9. 运行架构威胁、故障、性能、数据和可测试性评审。

**能力。** current-state-reconstruction、C4、clean/modular architecture、tradeoff-analysis、ADR、
distributed-systems、resilience、observability-design、migration-architecture、cost-modeling。

**必交付物。** Current/Target Architecture、C4/依赖/数据流、Option Matrix、NFR allocation、演进路线与触发器、
Migration/Rollback outline、ADR、Architecture Fitness Functions。

**规则与门禁。** 需求和领域边界可追踪；依赖向内；每个新增基础设施有驱动和 owner；架构主张有当前证据；
高风险方案经 fresh security/distributed/performance review 与人审。

**禁止与权限。** 不写实现代码、不把技术偏好冒充约束、不未经证据采用微服务/CQRS/Event Sourcing、不修改业务规则。

**退出与写回。** 关键权衡已决定、迁移可分步、风险 owner 明确后交数据/API/计划；写回 ADR、边界、约束和 fitness rule。

## 06 — Data Architecture / DBA（数据设计）

**目的。** 设计数据从创建到删除的完整生命周期，并让 schema、查询、迁移和恢复支持领域不变量。

**入口与输入。** Domain/Architecture 产物、数据分类、读写模式、规模/增长、合规、现有 schema/查询/运行指标。

**必须执行。**

1. 指定数据集 owner、system of record、分类、租户/地域、保留、归档与擦除；
2. 建概念/逻辑/物理模型，定义稳定标识、主外键、唯一/检查约束和空值语义；
3. 先规范化保证正确性，只为有证据的读模型做可管理的反规范化；
4. 设计事务边界、隔离级别、锁顺序、并发控制、幂等和死锁处理；
5. 根据真实查询形状、选择性和写成本设计索引并用执行计划验证；
6. 设计容量、分区、冷热层、备份、恢复、RPO/RTO 和数据质量监控；
7. 使用 expand → backfill → dual/read-compatible → contract 的向后兼容迁移；
8. 量化 backfill 时长、锁/日志/复制影响，准备校验、暂停和 rollback/forward-fix；
9. 对敏感字段定义最小化、加密、脱敏、访问与审计。

**能力。** data-modeling、SQL/schema-review、index/query-analysis、transaction-design、migration-engineering、
data-quality、backup-recovery、privacy-data-lifecycle、capacity-planning。

**必交付物。** ER/Schema Contract、Data Dictionary、ownership/classification、Query/Index Matrix、Transaction Matrix、
Migration/Backfill/Validation/Rollback Plan、retention/erasure 与 recovery plan。

**规则与门禁。** migration 可重复、可观察、兼容当前与前一应用版本；约束在数据边界执行；删除策略与审计一致；
测试覆盖升级/降级/中断/大数据量；索引有 workload 证据。

**禁止与权限。** 设计节点不直接改生产库；禁止无回填策略的 NOT NULL、无界 migration、默认硬删除核心资产、
无证据索引、Service 内散落 SQL。Migration Agent 只生成/验证 SQL，执行仍需独立授权。

**退出与写回。** schema、迁移、数据质量和回滚可验证后交 07–09；写回数据实体、所有权、lineage 与风险。

## 07 — API / Integration（接口与集成设计）

**目的。** 把上下文之间的交互冻结为可演进、可安全消费的契约，而不是把数据库表直接暴露为 API。

**入口与输入。** Requirement/Product/UX、Domain command/event、Architecture/Data、消费者清单与现有合同。

**必须执行。**

1. 识别消费者、用例、协议选择、信任边界、所有权和同步/异步需求；
2. 以 resource 或 command 语义设计 REST/GraphQL/RPC，不泄漏内部 persistence model；
3. 定义请求/响应、错误模型、状态码、验证、并发版本、幂等、分页、过滤和排序；
4. 定义 authn/authz、tenant、scope、rate/quota、审计与敏感字段策略；
5. 定义版本、兼容矩阵、弃用期限、consumer migration 和 contract tests；
6. 对 event/webhook 定义 schema/version、ordering、delivery、dedup、retry、DLQ、replay 和签名；
7. 定义超时、重试、熔断、backpressure 和部分失败语义；
8. 规范 trace/correlation/idempotency ID、指标、日志和支持诊断字段；
9. 提供正反例和 SDK/客户端使用约束。

**能力。** API-design、OpenAPI/AsyncAPI/Proto、compatibility、idempotency、error-modeling、event-contract、
integration-security、contract-testing、resilience。

**必交付物。** API/Event Schema、Consumer/Compatibility Matrix、Error Catalog、Auth Matrix、Idempotency/Retry
Contract、Rate/Quota 与 Deprecation Plan、contract-test cases。

**规则与门禁。** schema lint；向后兼容检查；每个写接口有重试/幂等语义；错误不泄密；事件发布与业务提交有一致性
方案；破坏性变化必须版本化和迁移。

**禁止与权限。** 不在接口层实现业务规则、不用 200 包装所有错误、不无限列表、不把“at least once”写成 exactly-once、
不在无 outbox/幂等/观测设计时用事件解耦。

**退出与写回。** 消费者、失败和兼容行为均明确并有 contract test 后交 Planning/Development；写回 API、事件和依赖边。
