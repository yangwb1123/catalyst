# 工程实现与重构规范

> 原则：指标负责触发调查，职责与变化原因决定设计。当前 ForgeOS 硬门仍是文件 ≤500 行、函数 ≤50 行、循环依赖=0
> 及 `.agent/AGENTS.md`/harness 的其它规则；本文中的其它数值是**可配置 review trigger**，不是跨语言真理。

## 1. 代码健康信号

### 建议默认阈值

| 对象 | 建议信号 | 处理 |
|---|---:|---|
| 文件 | 300 行 warning；当前项目 500 行 hard gate | 先做 Responsibility Map，不按行平均切 |
| 函数 | 30 行 review；当前项目 50 行 hard gate | 检查抽象层级、分支、effect 和命名 |
| Cyclomatic Complexity | 1–5 简单；6–10 可接受；11–20 重构评估；>20 原则阻断/限时例外 | 先拆决策或状态，不只抽私有函数 |
| Cognitive Complexity | >15 review | 关注嵌套、跳转和读者心智栈 |
| 嵌套深度 | >4 review | guard clause、状态/策略或流程重构 |
| 参数 | >5 review | 检查缺失的 Value Object/command，不用万能 options 掩盖 |
| 类/组件 | >300 行或 >15 public methods review | 识别多变化原因和不稳定依赖 |
| 构造依赖 | >8 review | 检查职责污染；不要用 service locator 隐藏 |
| Import/外部依赖 | >20 / >12 direct dependency review | 区分稳定内部依赖和外部 effect |
| Duplication | >5% 或同一业务规则出现两处 | 找唯一 owner，生成代码不与手写混算 |
| 循环依赖 | 0 | hard block |
| 测试覆盖 | risk-based；80% 可作项目目标，不作正确性证明 | 同时看关键路径、mutation、negative 和 assertion |

阈值按语言、框架、generated/vendor/test fixture、声明式表格和系统 lifecycle 配置。生成代码应通过来源/再生验证，而不是
强行按业务代码阈值拆。任何例外都需 evidence、owner、expiry 和 debt link。

### 系统认知复杂度预算

代码行合规不代表人能维护。Capability/Rule/Workflow 模块另设可配置预算：建议以 `public capabilities >20`、
`direct dependencies >15`、单次决策激活 `soft rules >30`、workflow/transaction graph 同层节点 >20、exception/waiver 持续增长、
一个 owner 需要理解过多状态/事件作为 review trigger。硬规则和载重合同不因预算被抑制；超限时应先输出 capability/
responsibility map、rule activation/suppression、dependency fan-in/out、change coupling、owner 和 context/token cost，再决定拆分、
分层、聚类或保留。禁止为了数字把一个概念拆成大量薄 wrapper，或把依赖藏进 registry/service locator。

Capability Registry、Rule Field 和 ContextPackage 必须报告预算使用与趋势；模块新增长期 exception、重复路由或同一变更需要
跨过过多 adapter 时进入 Technical Debt/Architecture Fitness，而不是只靠函数复杂度门禁。

### God File / God Class 判定

单一 LOC 超限已经触发当前 hard gate，但结构性 God risk 应综合：

- size/complexity：文件、方法、分支、认知复杂度；
- responsibility diversity：UI、用例、领域、数据、通知、审计等职责聚类；
- change coupling：不同需求经常同时触碰同一文件，或该文件与大量文件共同变化；
- cohesion：字段/方法共享程度低，方法围绕不同数据群；
- fan-in/fan-out：既被大量调用又依赖大量模块；
- effect diversity：DB、网络、文件、队列、权限、日志、事务混杂；
- ownership：多个团队/上下文在同一文件相互踩踏；
- test pain：只能靠巨大 mock setup 或 E2E 才能验证局部规则。

至少输出 `Responsibility Map`：职责、占比/位置、变化原因、依赖、状态 owner、公共合同和建议归属。禁止把
`order.service.ts` 机械改成 `order1.service.ts/order2.service.ts`。

## 2. 标准重构流程

```text
Detect → Baseline → Responsibility analysis → Target seams
       → Migration plan → Small extracts → Verify each step
       → Remove old path → Architecture/knowledge update
```

1. **冻结新增。** 命中硬门后暂停向目标继续堆功能；明确本次重构 scope/non-goals。
2. **建立基线。** 运行当前 tests/gates；缺失行为安全网时先加 characterization、golden 或 contract tests；记录性能/接口基线。
3. **静态与历史分析。** 采集上述 metrics、依赖、调用、变更耦合、owner 和 effect；工具不可用诚实 N/A。
4. **识别变化原因。** 按业务能力/用例/领域边界/外部 effect，而不是按技术段落或行数分类。
5. **选择目标边界。** 给每个单元命名、owner、输入/输出、不变量、依赖方向和测试；保留稳定 facade 以兼容调用方。
6. **比较方案。** 最小抽取、模块化、策略/状态、多 use case、event 解耦或延后；写成本、风险和不适用原因。
7. **设计迁移。** 定义 seam、adapter、双路径/feature flag（如需）、提交顺序、回退点和删除条件。
8. **渐进抽取。** 一次移动一个职责；保持公共行为；每步运行受影响 tests、architecture gate 和必要 benchmark。
9. **收口。** 消除旧路径/重复、收紧可见性、更新 imports/docs/contracts；不永久保留“双实现”。
10. **独立复审。** fresh reviewer 验证行为、职责、依赖、测试和 threshold；更新 Knowledge Graph、ADR/debt/health。

若无法建立安全网、公共合同不清或重构会与功能变更纠缠，先做 seam/strangler，或把重构拆成独立 Change，不做 big bang。

## 3. SOLID 与模块规则

### Single Responsibility

按“只有一个主要变化原因/owner”判断，而非“一个类只能一个方法”。Application service 可协调一个 use case，但不拥有
定价、通知、PDF、文件和审计的全部实现。

### Open/Closed

当同一业务维度持续增加且变化独立时，用策略/规则表/多态扩展。不要为一次性两分支先造抽象；先保留清晰条件和测试，
到扩展压力出现时重构。

### Liskov

替代实现必须维持输入前置、输出后置、错误/副作用、事务和性能语义；“类型能编译”不代表可替换。无法维持时优先组合。

### Interface Segregation

接口围绕消费者/use case 能力设计，例如 `CreateEmployee`、`DeleteEmployee`，避免万能 `UserService`；也避免每类一个无意义
单实现接口。接口属于使用者/内层 port，而非自动复制实现的全部方法。

### Dependency Inversion

稳定内层定义需要的 port，外层 adapter 实现；Presentation → Application → Domain 依赖向内，Infrastructure 在 composition
root 装配。不要让 domain import ORM/HTTP/container，也不要用 `utils/common/manager/impl` 隐藏边界。

## 4. OOP、组合与函数式选择

**使用 OOP 当：** 对象有稳定身份/生命周期/不变量；状态和行为必须封装；存在真实可替换的行为族；领域语言以对象协作表达
更清楚。

**使用纯函数/数据 pipeline 当：** 无状态转换、校验、计算、解析、排序和映射；透明输入输出比对象生命周期重要。

**默认组合优于继承。** 继承只用于稳定的 is-a 合同并通过 Liskov tests；不要用基类共享偶然代码。不要制造 anemic entity +
巨大 service，也不要把每个函数包进 class。

## 5. DI（依赖注入）

**适用：** DB、clock、random、network、filesystem、queue、external SDK 等 effect/易变边界；需要替换实现、测试隔离或集中管理
生命周期；多部署环境有真实 adapter。

**规范：** 显式 constructor/function injection；必需依赖不可空；composition root 唯一装配；domain 只依赖所需最小 port；
生命周期/线程安全/transaction ownership 文档化。

**禁止：** 业务层 `new Database/Redis/XxxClient`；Service Locator/global singleton 隐藏依赖；container 泄漏进 domain；
interface-per-class；为纯稳定算法制造 mock seam；20 参数 constructor 后用 DI 容器掩盖 God Class。

## 6. AOP / Middleware / Interceptor / Decorator

**适用：** 真正统一且与业务意图正交的 trace、metrics、审计 envelope、认证上下文、通用授权 enforcement、事务边界、
限流、缓存协议。

采用前必须明确：join point、执行顺序、同步/异步、上下文传播、错误映射、retry/transaction 交互、幂等、性能成本、
可观测与独立测试。优先 framework 明确 middleware/decorator，而非运行期魔法织入。

**禁止：** 把核心业务规则、价格/状态变更、隐式 DB 查询、不同用例语义不一致的权限或不可见控制流藏在 Aspect；
未知顺序的多个 interceptor；用 AOP 消除少量重复却增加调试黑盒。

## 7. DDD 与领域模式

**启用条件：** 业务规则复杂/频繁变化；语言冲突；多个 owner/模型；一致性边界是核心；领域知识的长期价值高。

**保持简单：** 短生命周期工具、纯 CRUD/reporting、规则稀少、单 owner 时用 modular transaction script/use case 即可。不得为目录
形式创建 Entity/Repository/Domain Event 仪式。

**Aggregate。** 由必须原子保护的不变量定义，保持小；跨 aggregate 用 ID，不加载整图；transaction 不越界。Repository 面向
aggregate/use case persistence，不做通用万能 CRUD。

**Domain Service。** 只放不自然属于单个 Entity/VO 的领域规则；Application Service 负责编排、权限上下文、transaction 和
外部 port，不吞领域规则。

## 8. 模式选择规则

| 信号 | 候选模式 | 采用前必须证明 | 常见误用 |
|---|---|---|---|
| 同一业务维度持续增加分支 | Strategy + Factory/registry | 行为可独立替换、合同一致 | 三个分支就强拆类 |
| 对象状态决定合法行为 | State / 显式状态机 | 转移、不变量、无效事件明确 | 用 class 数量隐藏简单 enum |
| 外部模型与领域不同 | Adapter / Anti-Corruption Layer | 翻译 owner、错误/版本语义 | 只换方法名 |
| 一个意图需排队/撤销/审计 | Command | 生命周期、幂等、结果明确 | 每个函数都 Command object |
| 多独立消费者响应已发生事实 | Domain/Integration Event | 一致性允许、协议完备 | 用事件替代清晰同步调用 |
| 横切行为保持相同合同 | Decorator/Middleware | 顺序、effect、错误透明 | 隐藏业务规则 |
| 复杂条件组合查询 | Specification/Query Object | 组合/测试收益真实 | 简单 WHERE 也抽象 |
| 新旧系统渐进替换 | Strangler + Adapter | 流量/数据/回退边界可控 | 无限期双写 |

## 9. 事件驱动门槛

只有在一个已发生事实有多个自治消费者、需要时间解耦/独立伸缩/审计重放，并接受相应一致性成本时采用。必须同时定义：

- event owner、业务过去式名称、schema/version/compatibility；
- transaction → outbox、consumer inbox/dedup/idempotency；
- ordering/partition、delivery guarantee 和重复/乱序行为；
- retry/backoff、poison message、DLQ、replay、retention 和 reconciliation；
- trace/correlation、lag/error metrics、schema registry 与消费者清单；
- PII/minimization、访问、删除/保留；
- consumer failure 不回滚 producer 时的业务语义。

需要立即结果或跨对象强一致不变量时优先同步/同事务。禁止只因“超过三个副作用”自动上 Event Bus；这个信号只触发评估。

## 10. 数据访问与迁移规范

- Controller/UI 不直接查库；application 调用拥有数据语义的 repository/query port；
- SQL 可存在于专用 adapter/query object，不为 ORM 纯洁性牺牲明确查询；
- command/write model 保护不变量，复杂 reporting 可用独立 read model；不要无证据 CQRS；
- index 从 workload/query plan 推导；cache 从测量推导并定义失效/一致性；
- migration 采用 expand → backfill → compatible switch → contract；每步有 checksum/progress/abort/rollback；
- 删除核心资产默认先评估 soft delete、审计、唯一键、查询过滤、恢复、retention 与最终擦除；软删除也不是无条件答案；
- Service 内 transaction ownership 明确，外部调用不在长事务内；需要一致发布用 outbox/补偿协议。

## 11. 前端上帝组件规范

God Component 识别不仅看 `.vue/.tsx` 行数，还看 data fetching、domain calculation、permission、form state、modal、table、
export、navigation、side effects 是否混杂。按 feature/用户任务与变化原因拆：

```text
OrderPage (route composition)
├── OrderFilter       # query intent
├── OrderTable        # rendering/selection
├── OrderForm         # editing/validation UI
├── OrderDetail       # read presentation
├── useOrderQuery     # server-state adapter
└── order.api/types   # contract boundary
```

页面/route 负责 composition，不成为业务 service；hooks/composables 只在状态/effect 真复用时抽取；领域计算留在可共享纯模块或
后端权威；组件 props/events 小而明确；避免把每段 JSX 拆成无语义组件。拆后必须保持 loading/empty/error/offline/permission/
accessibility、focus 和 performance behavior。

## 12. 生成前自检与 Review 输出

Coding Agent 在生成前回答：职责 owner 是否唯一？是否已有相同业务规则？依赖方向是否正确？外部 effect 是否有 seam？
错误/取消/上限是否明确？兼容/迁移如何？哪些模式真的满足条件？测试与遥测怎样证明？

Review/Refactor 输出至少包含：metrics、Responsibility Map、violated rule/evidence、pattern options、selected/rejected reason、
migration steps、risk/rollback、test plan、exception/debt、fresh verdict。没有行为保护时只能提出 seam 计划，不能宣称重构完成。
