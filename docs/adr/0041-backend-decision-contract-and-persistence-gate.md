# ADR-0041 — 后端决策合同与持久化前置关卡

- 状态：已接受（2026-08）
- 范围：后端实现前的工程决策、证据与审查合同；当前为 shadow，不授予执行或完成权限
- 关联：ADR-0037（能力中心化组织）、ADR-0040（机器可读 Agent Engineering）、
  `.agent/engineering/backend-decision-gates.yml`

## 背景

现有 AI Engineering OS 蓝图已经列出 Domain、Data、API、Backend、Security、Performance 与 Operations 能力，但它们
主要是规划目录和审查知识。普通 Coding Agent 仍可能直接从 HTTP DTO 生成 ORM Entity，把未验证假设当成事实，或在没有
明确身份、唯一性、状态、历史、事务、访问路径和迁移策略时创建持久化结构。这类错误往往比局部代码错误更难逆转。

另一方面，把 DTO、Command、Domain、Persistence、Read Model 和 Response 机械拆成固定文件，也会给简单 CRUD 制造映射
爆炸。因此需要的是基于变化原因和风险的决策关卡，而不是新的框架教条。

## 决策

### 1. 建立一个有边界的 BackendDecisionPackage

任何命中后端、核心领域、持久化、公共/事件契约、外部副作用、并发、多租户、安全、性能、后台任务、分布式边界、
资金/认证核心、生产数据或破坏性迁移的变更，都生成覆盖 14 个决策维度的 package。局部后端行为最低 L1/W1；领域、数据、
契约、并发和分布式变更最低 L2/W2；租户/安全最低 L3/W3；资金、认证核心、生产数据和破坏性迁移最低 L4/W3。每个维度只能是：

- `addressed`：有来源事实、选择、理由和 proof refs；
- `not_applicable`：有理由和证据，且未声明一个伪决策；
- `blocked`：保留开放问题，不能假装已经选择。

触发器直接要求的维度不能以 N/A 逃避。关键未知项阻断实现，不能静默转成默认值。Package 禁止
`completed/accepted/approved/verdict/gate_result`，完成权仍只属于 `forge accept`。

### 2. 固定决策顺序，不固定实现形状

决策顺序为：目标 → 不变量 → 边界/所有权 → 持久化 → 契约 → 算法 → 一致性 → 外部依赖 → 安全 → NFR 预算 →
运维 → 实现 → 验证 → 发布/演进。顺序要求 Agent 在选语言、ORM 或模式前先理解问题；它不是一个必须生成 14 份文档的
瀑布流程。

Request DTO、Command、Domain Model、Persistence Model、Read Model、Response DTO 和 External Service Model 是语义角色，
不是强制目录。默认仅在 owner、变化原因、安全分类或公共/持久化耦合不同时分离；简单内部 CRUD 可复用较少角色，但公共
契约不得直接暴露 ORM 对象。任何复用例外都要说明相同 owner/变化原因/安全分类以及为何不会形成外部耦合。

### 3. 将持久化视为架构，而不是 ORM 输出

持久化变更必须在编码前回答：业务/内部/外部/幂等身份、字段语义、金额/单位/时间/NULL、状态和历史、快照和血缘、
约束和并发、查询和索引、租户/隐私、保留/删除/修复，以及 expand–migrate–contract 兼容路径。数据库约束是并发下的最终
完整性防线；应用预检查只改善错误体验。

PostgreSQL 的 `CHECK` 对 NULL 的行为、`UNIQUE` 对 NULL 的默认处理，以及外键不会自动为引用列建立索引，均说明 Agent
不能从“约束存在”推断完整语义；索引也必须来自访问路径而非字段清单。

### 4. 把可靠性和演进纳入同一个设计产物

外部调用要记录端到端 deadline、唯一重试责任层、幂等/未知结果、连接与并发预算、背压和恢复。事务重试必须重放完整
事务，并隔离不可重复的外部副作用。性能选择需要真实基线、查询计划或代表性 benchmark；10×/100× 只记录失效阈值和
演进触发器，不提前实现幻想规模。

低可逆的主键、数据所有权、公共契约、事件语义、租户/权限模型、服务边界和分区策略必须比较至少两个候选，记录迁移
成本、爆炸半径、ADR、reviewer 与重新评估条件。架构同时考虑团队所有权、认知负载、可运营性、总拥有成本和删除路径。

### 5. 用十个密集 Skill adapter 承载能力，不制造十九个空壳

首批 adapter 为 backend-engineering、domain-modeling、data-modeling-transactions、data-migration-lifecycle、
api-contract-design、distributed-reliability-design、performance-capacity、observability-engineering、secure-coding 和
architecture-tradeoff。每张卡都有触发、输入、SOP、输出、边界、自动化/验收和直接参考；OOP、FP、DI、AOP、算法、网络、
数据库等作为这些能力内的条件化决策 lens，而不是永久 Agent 或孤立知识页。

### 6. 当前只启用合同验证和 shadow 路由

`harness/backend_decision_check.py` 以 byte digest 锁定完整 policy/schema，验证 Skill 结构，并可验证一个 BackendDecisionPackage；
package 的 evidence 使用有大小上限的仓库内 regular-file locator，CLI 流式重算内容 digest，并按 proof type、source/tool/review
class 与唯一 subject 约束事实、决策、N/A、独立 review 和十维 readiness。Readiness 还必须满足对应 decision dependency；低可逆
或触发器指定的不可逆 decision kind 必须解析真实 ADR 并使用包级独立 Reviewer。Context registry 对
数据/契约和后端运行路径装载相应 Skill 与 policy。detector 明确标为 `shadow`、`load_bearing:false`，尚未自动从 diff 生成
package，也未在 coding 前强制阻断。`harness/check.py` 只证明治理资产和接线没有漂移，不能证明某个方案本身正确。

## 后果

**正面。** 后端设计得到统一、可审计的决策表；事实、假设、证据和 N/A 分离；持久化、并发、可靠性、运维与演进不再是
编码后的补充清单；Skill 能按路径/能力路由，并随 init/upgrade 继承。

**成本。** 后端变更增加与风险相称的前置分析和独立 review 成本。当前 evidence 虽会解析文件、重算摘要并校验 proof
类型/subject/class，但 producer/Reviewer 仍是 digest-bound 声明，不是可信 Evidence Ledger 的身份签发；完整 source tree/context digest 也未由 runtime 重算，Context route 仍是 shadow policy。要成为真正 pre-code gate，后续必须由 runtime 生成 source/context digest、
把 package 绑定到授权与代码快照，并让 required finding 在状态机中失败关闭。

## 被拒方案

1. 一个“20 年后端经验”巨型 Prompt：无法按风险路由，也无法验证 N/A、证据和漂移；
2. 为每个学科创建永久 Agent/独立 Skill：增加协调和维护成本，且复制既有 capability ownership；
3. 强制所有 CRUD 使用完整 DDD/六层模型：把边界保护变成样板代码和错误抽象；
4. 让 decision package 自行批准实现：形成第二个完成权威和自我证明；
5. 立即接入 runtime enforce：当前尚无可信 Evidence/Claim ledger、ContextPackage 和授权状态机，宣称强制会失真。

## 一手资料

- PostgreSQL, [Constraints](https://www.postgresql.org/docs/current/ddl-constraints.html)
- PostgreSQL, [Transaction Isolation](https://www.postgresql.org/docs/current/transaction-iso.html)
- PostgreSQL, [Multicolumn Indexes](https://www.postgresql.org/docs/current/indexes-multicolumn.html)
- IETF, [RFC 9110: HTTP Semantics](https://datatracker.ietf.org/doc/html/rfc9110)
- IETF, [RFC 9457: Problem Details for HTTP APIs](https://datatracker.ietf.org/doc/html/rfc9457)
- Google SRE, [Addressing Cascading Failures](https://sre.google/sre-book/addressing-cascading-failures/)
- OpenTelemetry, [Semantic Conventions](https://opentelemetry.io/docs/specs/semconv/)
- OWASP, [API Security Top 10 — 2023](https://owasp.org/API-Security/editions/2023/en/0x11-t10/)
- Protocol Buffers, [Language Guide (proto3)](https://protobuf.dev/programming-guides/proto3/)
