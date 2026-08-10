# Frontend Code Architecture and Organization Governance

> 这是前端代码结构治理的规范入口。它补充 AFDS 的产品、交互、视觉与框架映射，不取代现有 Capability ownership，
> 也不把某一种目录流派或一组固定数字冒充所有项目的工程标准。

## 1. 为什么需要独立治理入口

行数只能观测体积，不能观测职责。180 行文件可能同时拥有 API、权限、表单、导出和路由；800 行静态 schema 仍可能只有
一个变化原因。治理目标因此是控制变化传播，而不是追求更小文件数量：同一业务原因尽量聚集，不同业务只经最小稳定契约协作。

`frontend-code-architecture` 是独立的条件化 Skill，但不新增重复 fine capability。client 实现、系统架构、fresh review 和
God Object 拆分仍分别使用 canonical owner；该 Skill 负责把它们按前端代码结构问题编排起来。

## 2. 规则强度

自动检查只处理可确定事实：激活 profile 的反向依赖、循环、禁止的跨 slice 边、跨边界 deep import 和无效例外。
职责数量、业务 flow 数、内聚程度、是否值得 shared、是否过度架构都需要代码、契约、diff 或历史证据，由独立 Reviewer 判断。

当前 detector 为 shadow，不接 `forge accept`；它可以对显式 target 产生确定性 fail，但不能改变完成裁决。配置 target 而解析器缺失、
import 无法解析或 ownership 不完整时输出 inconclusive，不能降级为 PASS。只有在真实项目收集误报、未解析率和耗时之后，确定性规则
才可通过单独治理版本升级为载重门禁。

具体 lines、Hook、handler、API、export、目录深度和同级文件数属于组织预算或启发式。它们触发检查，不单独证明 God File；
也不能通过机械拆文件、制造 facade 或关闭规则来“修复”分数。

## 3. Profile 而非统一目录答案

支持三种决策：

- `feature_sliced_client`：适合明确采用 app/pages/widgets/features/entities/shared 的 Web/RN 项目；
- `flutter_feature_clean`：适合按 feature 组织 presentation/application/domain/data 的 Flutter 项目；
- `custom_declared`：项目给出 source root、layer alias、禁止边、slice 和 public entry。

没有明确声明且目录签名不充分时为 `unconfigured`。系统可以给出候选和语义审查，但必须输出 not configured，不能宣称边界检查通过。
逻辑边界应先于物理迁移；不得为了采用 Profile 一次重写成熟仓库。

## 4. 模块、状态与 Effect 所有权

每个模块记录 responsibility、owner、public API、internal surface 和变化原因。route/page 只做 composition；feature 表达一项用户能力；
entity 表达稳定概念；shared 只承载无业务语义且有真实跨业务消费者的基础能力。

server、URL、form、workflow、cache 和 local view state 分配给能完整拥有生命周期的最小边界。API DTO、业务 policy、mapper、
telemetry 和持久化副作用不能散落在 render/build 方法中；跨模块协作不得借 global store、event bus 或 service locator 隐藏依赖。

## 5. 公共入口与 Shared 准入

跨模块只依赖公开入口。公开入口列出刻意稳定的 UI/operation/type，不使用递归 star barrel 泄露 internal。公开项超过预算时审查
职责和消费者，而不是盲目新增 package。

进入 shared 至少需要两个真实跨业务使用方、不含具体业务语义、无上层依赖、稳定契约以及更低总维护成本。首次出现的业务逻辑
保留在所属 slice；第二个真实场景出现后再比较复制、参数化和共享抽象。

## 6. 多信号 God Risk

机器可以诚实观测 lines、functions、Hook/composable、handler、remote-call、state、exports、dependency、depth 和 sibling count。
它不能从正则可靠得出“有三个业务流程”或“职责不相关”。因此风险报告必须逐项区分：

- `observed_proxy`：可复算的静态计数；
- `semantic_finding`：Reviewer 用责任图、变更和行为证据得出的结论；
- `policy_decision`：项目预算与有效例外后的处理。

默认 0–2 正常、3–4 审查、5–7 拆分计划、8+ 仅在独立语义证据同时成立时阻断。行数不能单独升级成架构 blocker。

## 7. 例外、迁移和删除

主合同、baseline 和 waiver ledger 是项目实例：`forge-init` 只在首次创建时播种，`forge-upgrade` 只补齐缺失实例，
不得用上游空模板覆盖项目已声明的 target、历史债务或已批准例外。通用 policy、Skill、detector 和文档才属于可同步的治理投影。

边界例外必须精确到 path 或 edge，并记录 rule、reason、owner、approver、evidence、expiry 和 removal trigger。仓库级通配、自批、
无期限或过期例外失败关闭。

Baseline 只用于带 owner、debt link 和删除触发器的既有、可豁免 finding；依赖方向、所有权、循环与配置完整性既不能进入
baseline，也不能通过 waiver 绕过。Baseline 中的 rule 与 finding fingerprint 必须精确匹配。

公共接口或大半径迁移使用 adapter/facade、兼容窗口、逐 slice 切换和退出检查。新增 abstraction、flag、barrel 和 shared module 时，
同时说明将来如何删除；Bug fix 不顺手重写全模块。

## 8. 客户端工程条件化 Lenses

本 Skill 只解决“代码放在哪里、依赖如何走、谁拥有状态与 effect”。其他前端系统问题复用现有能力：

- 需求/flow/permission/recovery：`information-interaction-design`；
- token/CSS ownership/a11y：`design-system-accessibility`；
- API/error/cache/retry/build/runtime/release mapping：`frontend-client-engineering`，必要时路由 API、安全、性能、可观测和发布 Skill；
- 独立架构裁决：`code-review`；行为保持式拆分：`god-object-refactoring`。

建议按触发器形成 ClientContractMap、ErrorRecoveryMatrix、PermissionActionMatrix、StyleArchitectureManifest、FrontendBuildManifest 与
FrontendReleasePlan，而不是每次任务固定生成全部文档。

## 9. 证据与完成边界

流程是：重建结构 → 确认 Profile → 责任/所有权地图 → 最小 public API → 实现 → 静态 fitness → 行为测试 → fresh review。
报告必须包含计划/实际 change surface、自动 hard findings、审查 finding、有效例外、真实命令回执和未执行项。

当前自动检测只能证明其解析到的 import/path/metric，不证明业务语义或 Reviewer 身份。`not_configured`、`unparsed` 和
`not_executed` 不得写成 PASS；最终完成权仍只属于 `forge accept`。
