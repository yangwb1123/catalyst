# 后端智能体计算机科学与工程决策规范

> 状态：Agent Engineering v1 的 shadow 设计规范。它规定编码前如何形成决策证据，不能自行授权执行、发布或完成。

## 1. 目标与适用边界

后端 Agent 的职责不是“把需求翻译成框架代码”，而是在业务、数据、协议、并发、网络、运行和演进约束中选择最低充分复杂度，并用真实证据校正选择。

以下任一变化触发 `BackendDecisionPackage`：后端行为、核心领域、持久化 Schema、公共/事件契约、外部副作用、并发、多租户、安全隐私、容量、后台任务、分布式边界、资金/认证核心、生产数据或破坏性迁移。普通、局部且可逆的后端行为最低 L1/W1；领域、数据、契约、并发和分布式变更最低 L2/W2；多租户/安全最低 L3/W3；资金、认证核心、生产数据和破坏性迁移最低 L4/W3。具体映射由 policy 锁定，不能由 Agent 自行降级。

纯文案、只读调查或无行为变化的机械重命名不要求该包。跳过仍是一项可审计判断，必须记录理由、来源与 Reviewer，不能靠 Agent 自由选择 N/A。

## 2. 决策顺序

后端实现必须按下列因果顺序收敛：

1. 理解用户问题、角色、价值、范围、异常和验收；
2. 识别业务不变量、权限、重复提交和部分失败；
3. 明确限界上下文、数据所有权、聚合与一致性边界；
4. 设计身份、持久化、历史、快照、查询、索引和生命周期；
5. 设计 API、事件、错误、版本、消费者和兼容窗口；
6. 按访问模式选择数据结构、算法和函数/OOP 组织方式；
7. 设计事务、隔离、并发、幂等、顺序与未知结果；
8. 设计网络、缓存、消息、后台任务、背压和故障恢复；
9. 设计认证、授权、租户、隐私、供应链和滥用边界；
10. 分配延迟、容量、可用性、恢复和成本预算；
11. 设计日志、指标、Trace、审计、Runbook 和人工修复；
12. 实现最小纵向切片；
13. 运行静态、单元、集成、契约、竞争、故障和性能验证；
14. 灰度、观察、回滚/前滚修复、复盘并更新知识提案。

“选 NestJS、Go、Rust 或 ORM”出现在业务和数据决策之前，属于顺序错误。

## 3. 事实、假设、决策与证据

- `Fact` 必须绑定代码、Schema、契约、文档、运行数据或用户确认来源。
- `Assumption` 必须有置信度、错误影响、可逆性和验证计划。
- `Decision` 必须写选择、替代方案、理由、影响和重访触发器。
- `Evidence` 必须是结构化记录；v1 CLI 会解析仓库内 regular-file locator、以 8 MiB 上限流式重算内容摘要，同时锁定 policy/schema 摘要。每条证据还必须声明 proof type、source/tool/review class 和唯一 subject，决策、readiness、N/A 与独立 review 只能引用类型和 subject 相符的证据。“看起来正确”和旧运行结果不是证据。
- `Unknown` 若承载主键、权限、数据所有权、兼容或不可逆效果，必须阻断实现。

N/A 是一个可证伪的结论，不是省略。`not_executed` 表示义务存在但没有运行验证，永远不能等同 PASS。

诚实边界：当前 shadow CLI 尚未重算整个工作树或 ContextPackage，`source_tree_sha256/context_sha256` 仍是声明字段；`evidence_class`、`producer_id` 和 Reviewer 身份也是 digest-bound 声明，而不是受信 runtime 签发的 attestation。它能证明逐项文件字节、proof 类型/subject/class 关系及 policy/schema 版本一致，不能密码学证明工具或 Reviewer 身份。完整 tree/context provenance、运行回执和独立身份必须等待 Evidence/Context/Identity runtime。

## 业务领域与模型边界

### 4. 先选择领域建模强度

| 情形 | 默认模型 |
|---|---|
| 单表、低风险、少规则的内部 CRUD | 事务脚本 + 清晰契约 |
| 多状态、多规则、并发修改 | Entity/Value Object + 显式状态机 |
| 多对象共同维护不变量 | Aggregate + Application transaction owner |
| 跨上下文、独立演进 | Context Map + Anti-corruption Layer |

DDD、事件驱动、CQRS 和 Event Sourcing 只有在对应变化压力、一致性或审计需求被证明时才启用。

每条核心不变量至少回答：来源是什么、谁拥有、在哪个边界保持、数据库是否能兜底、并发下是否仍成立、哪个反例测试可以推翻实现。

### 5. 模型角色不是目录仪式

语义上区分：

```text
Request DTO → Command → Domain Model → Persistence Mapper → Persistence Model → Database
Database → Projection/Persistence Model → Read Model → Response DTO
External Service Model → Adapter/Translator → Internal Model
```

分离的目的在于隔离变化原因、所有权和敏感字段，不是机械产生七份同构类。简单内部 CRUD 只有在生命周期、权限、字段和变化轴相同且没有公共/持久化耦合时才可复用；公共接口不得直接暴露 ORM Entity，Read Model 不得作为领域写入输入。

### 6. 编程范式选择

- OOP 用于身份、生命周期、不变量和多态行为；组合优先继承。
- 纯函数用于金额、规则判断、映射、归一化和排序，便于属性测试。
- 高阶函数用于重试、事务、权限、指标等可见包装，但包装顺序和错误语义必须显式。
- 柯里化解决预绑定和组合，不天然提升运行性能；热路径需评估闭包分配和可读性。
- DI 隔离 IO 与基础设施；不得为每个类机械创建接口。
- AOP 只承载可正交移除的横切逻辑；业务状态转换、授权决策和关键副作用不能藏在切面中。

## 持久化设计关卡

### 7. 身份、事实与时间

对核心实体必须分开定义：数据库主键、业务编号、自然键、第三方标识、幂等键。选择自增、Sequence、UUIDv7、ULID 或其他方案时分析生成位置、索引局部性、公开风险、分布式需求和迁移成本。普通 Sequence 不保证无间隙，不能直接承担法律连续编号语义。

金额/数量必须携带币种或单位、精度、舍入、分摊和溢出规则。二进制浮点不得持久化关键金额。时间必须区分时间点、业务日期、区间、计划/实际和有效期；只存 UTC 时间点不足以重建法律时区或本地日程，必要时同时保留 IANA 时区与本地语义。

NULL 必须只有一个明确含义。`unknown`、`not applicable`、`not set`、`cleared` 和业务状态不得混成一个 NULL。状态码发布后不得复用，状态历史不能只靠普通日志恢复。

### 8. 关系、快照、历史与数据来源

- 关系由生命周期、所有权和删除语义决定；带 assigned_at/scope/status 等属性的中间表是业务实体。
- 历史业务文档应保存会变化的名称、价格、税率、单位或地址快照，而非只保留当前外键。
- 原始事实、当前状态、派生值、缓存、搜索、报表和物化视图必须标明权威来源与重建路径。
- 对监管、追溯或纠错场景，评估 valid time 与 transaction time；不需要时不要机械上双时态模型。
- 设计身份合并、拆分、重键、匿名化和引用存活期；“一条记录永不变”通常是未经验证的假设。

### 9. 字段、JSON、约束和完整性

每个字段记录语义、类型、长度/精度、NULL、默认值、单位、来源、可变性、敏感级别、索引、历史和保留。禁止所有字符串 `VARCHAR(255)`、关系逗号串、JSON 逃避核心建模和仅靠应用代码保证唯一。

数据库约束是最终并发防线。需要特别注意：PostgreSQL `CHECK` 为 TRUE 或 NULL 都会通过；默认 UNIQUE 可允许多个 NULL；外键不会自动给引用方列建索引。应用层预检查只用于友好错误。

使用 JSON 前必须回答内部字段是否查询/索引/约束、结构是否稳定、是否独立更新、增长是否有界。JSON 可用于不稳定附加属性、原始第三方报文和审计上下文，不应用于核心关系与高频统计事实。

### 10. 查询、索引和分页

索引从 Query Profile 推导：过滤、Join、排序、范围、分页、频率、选择性、返回量与写入成本。复合 B-tree 通常最有效利用前导等值条件及之后第一个范围条件；不能机械“每列一个索引”，也不能依赖新版本 skip-scan 掩盖错误顺序。

大数据连续读取优先稳定 Cursor；游标需唯一顺序，如 `(created_at, id)`。Offset 适用于小集合或真实跳页需求。每项索引优化必须检查真实数据库版本的 `EXPLAIN`/运行计划和写放大。

### 11. 删除、归档、修复与租户生命周期

硬删除、软删除、失效、归档、匿名化和法规擦除语义必须分开。软删除不是默认：它会改变查询、唯一约束、关联、恢复冲突和表膨胀。

多租户不只是增加 `tenant_id`。必须覆盖查询/唯一键、缓存、消息、搜索、对象存储、审计、导入导出、租户迁移、停用和彻底删除。高风险数据修复应提供预览、幂等、审批、审计、对账和回滚，不能把 DBA 临时改表作为正常流程。

## 契约兼容与错误模型

### 12. API、事件与 Schema 都是协议

契约必须定义字段、默认值、NULL、枚举、错误、授权、幂等、分页、顺序、重复、版本、消费者和废弃。结构兼容不等于语义兼容：新增枚举可能破坏穷举客户端，收紧校验或改变默认值也可能破坏旧版本。

HTTP 错误应使用稳定机器码和适当状态码；RFC 9457 Problem Details 用于公开问题描述，不用于暴露堆栈、SQL 或调试细节。Protobuf 删除字段时保留编号与名称；需要存在性语义时明确 optional/default 行为。

事件表达业务事实而非数据库行。CloudEvents 类 envelope 提供标识和元数据，不自动提供 exactly-once、顺序或重试；顺序使用 aggregate version、分区 offset 或数据库序列，不只靠时间戳。

## 网络分布式与可靠性

### 13. Deadline、取消与连接池

每个远程调用从端到端预算分配 DNS、连接、TLS、池等待、请求头、响应读取与重试；下游继承剩余 deadline，不能每层重新获得完整超时。取消必须让应用停止无价值工作。

复用受限连接池，限制响应/上传/批量大小、重定向和并发；正确关闭响应流。所有第三方响应与用户输入同样不可信。

### 14. 重试、幂等与未知结果

重试必须只有一个责任层，并同时满足：错误临时、剩余预算足够、操作天然幂等或有业务幂等协议。采用有上限的指数退避、抖动和全局重试预算；多层重试会指数放大故障流量。

幂等键至少绑定主体/租户、操作类型、键和请求指纹，原子持久化并定义并发、TTL、失败、终态结果重放和清理。超时后“不知道是否生效”是独立状态，必须查询/对账，不能盲重试不可逆操作。

### 15. 消息、缓存和后台任务

消费者默认面对重复、乱序和重放；定义分区键、提交点、最大尝试、延迟重试、DLQ、毒消息和人工修复。数据库变更与事件发布的双写可用同事务 Outbox，但消费者仍需幂等。

缓存必须说明模式、权威源、一致性窗口、失效失败、击穿/雪崩/热点和重建。后台任务需状态机、租约/心跳、幂等、checkpoint、取消、分片、超时和失联对账，不能只有 `is_running`。

### 16. 背压、隔舱和负载拒绝

所有线程、协程、Promise、连接、队列、缓存和批次必须有界。过载时先 admission control、排队超时、load shedding 和降级，不能只提高超时。非关键报表不得耗尽下单使用的共享池。

按租户和业务优先级设计公平性与 noisy-neighbor 隔离。需要防范“少量输入触发大量数据库/第三方/AI 费用”的成本放大攻击。

## 算法性能与容量

### 17. 从访问模式选择结构

必须区分读写比例、排序/范围/成员判断、静态/动态、精确/近似、内存、并发和跨节点。Bloom Filter、Bitmap、Heap、Tree、Trie、搜索引擎和向量库都需要适用证据；数据库唯一索引通常比进程 HashSet 更适合持久并发唯一性。

树模型在 adjacency list、materialized path、nested set、closure table 之间按子树读取、移动频率、深度和数据库递归能力选择。大文件/查询使用 Stream、Cursor、Iterator 或分批处理并传播背压。

### 18. 测量与 10x/100x

容量模型至少包含数据量/增长、QPS、并发、请求/消息大小、连接、队列、内存、存储、日志、成本、P50/P95/P99 和饱和度。分别说明当前、10x、100x 时最先失效的假设及监控阈值。

性能优化顺序：测量 → 算法 → I/O/查询/索引 → 批处理/流式 → 有界并发 → 缓存 → 运行时/硬件。Micro benchmark 不能证明系统 SLO，平均延迟不能代表尾延迟。

## 安全租户与隐私

### 19. 授权和数据边界

授权顺序：认证 → 功能权限 → 资源对象权限 → 数据范围 → 字段过滤/脱敏。每次基于用户提供 ID 访问资源都要执行对象级授权；仅有角色名不证明可访问该租户/部门/客户数据。

输入与输出均使用字段 allowlist，防止 mass assignment 和过度暴露。对分页、批量、上传、解析深度、并发、执行时间、第三方次数和资金成本设置限制。SSRF、SQL/命令/路径注入及供应链依赖需独立检查。

### 20. 隐私和秘密

按 Public/Internal/Confidential/Restricted 分类，记录最小化、加密、脱敏、访问、日志、保留、删除和备份处理。密钥使用 secret broker/Keychain/HSM 与短期引用，不能进入代码、配置仓库、Prompt、日志或测试快照。

## 可观测性与运营恢复

### 21. 四类证据

- 日志：一次离散事件发生了什么；
- 指标：数量、趋势、SLO 和饱和度；
- Trace：一次请求、任务或消息经过哪些边界；
- 审计：谁以何权限改变了哪个业务事实。

遵守 OpenTelemetry 语义约定，控制 cardinality 和敏感数据。用户、租户、订单与请求 ID 通常属于 Trace/受控日志，而不是指标标签；Baggage 是可传播的不可信输入，不承载秘密或授权结论。

### 22. 运营和恢复是功能

关键模块提供 Runbook、可操作告警、优雅关闭、任务暂停/恢复、死信处理、对账、人工补偿、数据修复和审计。Kubernetes readiness 决定是否接流量，liveness 只在进程不可恢复时重启；终止时停止接收新工作、排空并在 grace period 内保存状态。

备份文件存在不等于可恢复。核心数据明确 RPO/RTO、备份链、加密、保留和恢复顺序，并定期做真实恢复演练与数据校验。

## 迁移恢复与生命周期

### 23. Expand–Migrate–Contract

推荐阶段：新增兼容结构 → 新旧双写/兼容写 → 幂等限速回填 → 验证 → 切读 → 停旧写 → 观察 → 删除旧结构。每阶段必须允许旧新代码共存并有退出条件。

迁移报告包含 DDL 锁、表重写、WAL/磁盘、复制延迟、索引失败残留、运行时长、批次、checkpoint、取消、abort threshold、回滚或 forward-fix 及验证 SQL。在线/CONCURRENT 操作仍有代价和失败状态，不能视为零风险。

## 长期架构与演进

### 24. 管理变化和耦合

衡量一次典型业务变化影响多少模块、表、契约、事件、服务与部署单元。检查代码、数据、时间、顺序、部署、运行、语义和组织耦合；接口存在不等于低耦合。

默认先做模块化单体。只有稳定业务边界、明确 owner、独立发布/扩缩/隔离/安全价值和运维能力同时存在时才拆服务。共享数据库、同步长链和共同发布的“微服务”是分布式单体。

### 25. 可逆性、组织与认知

主键、数据所有权、公共 API/事件、租户/权限、服务和分区边界是低可逆决策，需至少两个替代方案、迁移成本、爆炸半径、ADR 和重访阈值。局部函数与内部目录是高可逆决策，可小步试验。

架构必须匹配团队人数、技能、值班、部署和恢复能力。普通变更需理解的概念、跨越的服务和协调团队数量都是成本。不要设计只有理想团队才能维护的系统。

### 26. 数据重力、控制面与退出能力

计算尽量靠近大数据，避免把千万行拉到应用层或跨区域搬运大对象。区分事务事实、派生投影、缓存和搜索的权威性。控制面负责策略与 desired state，数据面负责有界执行；不能让执行者自行审批和验证自己。

功能、字段、接口、事件、依赖、Feature Flag 和服务都应有退出路径、消费者清单、到期 owner 和删除门禁。长期只能添加不能删除会形成架构熵。

### 27. 经济性和停止条件

总拥有成本包括开发、部署、运维、监控、培训、升级、安全、事故、迁移、资源和供应商锁定。Build vs Buy 应比较核心竞争力、合规、退出成本和团队能力。

架构必须写“够用”条件，例如目标 QPS、P99、可用性和数据增长。达到目标后停止为未经验证的规模加复杂度，只记录下一阶段触发阈值。

## 28. Production Readiness Gate

最终审查聚合十个维度：需求、架构、领域与持久化、算法与性能、并发与一致性、网络与依赖、安全与隐私、可观测性与运营、测试与证据、发布与恢复。

每个维度只能为：

- `ready`：所需 proof type/class/subject 覆盖完整、为正向结果，且依赖的 decision 均已 addressed；当前 shadow 阶段这仍不是受信运行时证明；
- `not_ready`：仍有失败、未知或未执行义务；
- `not_applicable`：触发条件不成立，并有 source-bound 理由。

该聚合器只形成就绪输入。CLI 明确区分 `STRUCTURALLY_VALID`、`VALID_BLOCKED`、`VALID_NOT_READY` 和 `SKIP_REVIEW_REQUIRED`，不会把结构合法打印成设计 PASS。最终完成和接纳仍只属于 `forge accept`；任何手写报告、Agent 自评或 Reviewer 叙述都不能铸造完成状态。

## 29. 主要官方依据

- [PostgreSQL Constraints](https://www.postgresql.org/docs/current/ddl-constraints.html)、[Transaction Isolation](https://www.postgresql.org/docs/current/transaction-iso.html)、[Multicolumn Indexes](https://www.postgresql.org/docs/current/indexes-multicolumn.html)
- [RFC 9110: HTTP Semantics](https://datatracker.ietf.org/doc/html/rfc9110)、[RFC 9457: Problem Details](https://datatracker.ietf.org/doc/html/rfc9457)
- [Google SRE: Addressing Cascading Failures](https://sre.google/sre-book/addressing-cascading-failures/)
- [OpenTelemetry Semantic Conventions](https://opentelemetry.io/docs/specs/semconv/)
- [OWASP API Security Top 10 — 2023](https://owasp.org/API-Security/editions/2023/en/0x11-t10/)
- [Kubernetes Pod Lifecycle](https://kubernetes.io/docs/concepts/workloads/pods/pod-lifecycle/)
- [Protocol Buffers Language Guide](https://protobuf.dev/programming-guides/proto3/)

这些来源提供规范事实；本文件中的设计门禁是基于事实形成的工程策略，不能把策略写成外部标准原文。
