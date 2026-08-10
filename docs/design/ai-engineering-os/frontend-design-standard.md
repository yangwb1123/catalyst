# AI Frontend Design Specification（AFDS）

> 状态：Frontend Engineering 的 **shadow 合同切片**。已交付 byte-pinned policy、Profile/Pattern catalog、
> FrontendDesignPackage Schema、三张 canonical owner Skill + `ui-geometry` supporting Skill、Context route、shadow
> detector/checker 与 scaffold/upgrade 继承；尚未新增
> 自动 diff compiler、可信 capture/reviewer runtime、编码前授权或完成裁决。没有真实工具回执的检查只能标为
> `not_executed`；最终完成权仍只属于 `forge accept`。决策见
> [ADR-0042](../../adr/0042-frontend-design-decision-contract.md)。

## 1. 目标与适用边界

AFDS 约束的不是某一份 TSX、Vue SFC、Dart 或 React Native 代码，而是从业务目标到可验证界面的决策链：

```text
business intent
  → user/task/context
  → information architecture
  → flow/state/action/permission contract
  → scenario profile × page pattern
  → design-system tokens/components
  → business-bound geometry composition
  → platform adapter
  → implementation
  → behavior/accessibility/visual/performance evidence
```

以下变化应使用本标准：新页面或关键流程、导航/信息架构、业务状态与权限操作、表单或数据密集界面、设计系统、
响应式/多平台映射、动效或沉浸体验、可访问性、视觉回归和前端性能。纯文案或无行为变化的机械修改可以缩短分析，
但跳过必须有任务范围内理由；Agent 不能自行把高风险流程降为“只改样式”。

本规范不要求一个 Agent 同时充当产品、UX、设计和实现者，也不要求所有项目采用相同组件库、网格或视觉风格。
它固定决策顺序和证据义务，不固定最终界面形状。

## 2. 五层规则权威

每条前端规则都必须声明来源层、作用域、触发条件、适用例外和验证方式。数字相同不代表权威相同。

| 层 | 含义 | 示例 | 处理方式 |
|---|---|---|---|
| `standard` | 外部规范中的可测试要求 | WCAG 2.2 Success Criterion | 按版本与平台精确引用；不得被 Profile 覆盖 |
| `platform_contract` | 项目明确采用的平台/框架语义与行为契约 | HTML 语义、project-adopted APG 键盘模式、Flutter Semantics、RN accessibility props | 仅在对应平台且项目已采用时触发 |
| `organization_policy` | ForgeOS 或项目批准的强制边界 | 组件白名单、token 来源、禁止泄露敏感数据 | 需 owner、版本与例外流程 |
| `profile_heuristic` | 为特定场景优化的可调默认值 | ERP 紧凑密度、营销页品牌动效 | 可被有证据的场景决策覆盖 |
| `advisory` | 尚不能确定性证明的建议 | 视觉平衡、审美评分、候选优化 | 进入 Review，不冒充自动 Gate |

规则记录至少应能表达：`id/kind/source/version/scope/trigger/applicability/requirement/exceptions/verification/owner`。
本 shadow 切片只定义概念；仓库当前没有 AFDS Rule Registry 或自动路由器，不得声称这些字段已被 runtime 执行。

### 2.1 固定数值不是跨平台事实

- `4/8pt grid`、字号阶梯、圆角、阴影和 100/150/200/300ms 动效是可用的 Design Profile，不是 W3C 或所有框架的硬标准；
- “字体不得小于 14px”不是 WCAG Success Criterion。Web 应验证对比度、缩放、reflow、语义与可读性；字号由内容、语言、
  观看距离和平台 Profile 决定；
- “所有按钮至少 44×44”不是 WCAG 2.2 AA。Web AA 的 Target Size (Minimum) 为 24×24 CSS px 或满足间距条件，
  且有例外；Flutter 官方指南则建议可点击目标至少 48×48 logical pixels；
- `390/1024/1440` 是测试样本或 Profile 断点，不是通用响应式标准。Web 还必须考虑等价 320 CSS px reflow，
  组件容器边界、缩放、长文本、RTL 和输入方式；
- “所有页面都有 loading/empty/error”也要按触发条件应用。异步数据面需要相应状态，纯静态页面不能被迫制造假状态。

组织可将上述数值设为 token/profile 默认值，但必须标成 `organization_policy` 或 `profile_heuristic`，不得伪装成外部标准。

## 3. 编码前决策顺序（canonical 16 步的 10 步宏观压缩视图）

1. 确认业务目标、主要 Actor、主任务、成功结果、非目标和风险；
2. 识别入口、出口、上下文保留、设备/输入方式、使用频率、数据密度和运行环境；
3. 建立信息架构、对象命名、主次层级、导航、搜索与筛选关系；
4. 定义主流程、替代流程、错误/取消/恢复流程和不可逆操作；
5. 定义业务状态、状态转移、action guard、权限与系统状态；
6. 选择场景 Profile 和页面 Pattern；两者不决定业务规则；
7. 解析项目 Design Token、批准组件、品牌、国际化与平台限制；
8. 形成平台无关 UI Decision Contract 和业务可追踪的几何构图，再映射到 Web/Flutter/React Native；
9. 实现最小可验证纵向切片，保持副作用和异步状态显式；
10. 用真实行为、可访问性、截图、性能和人工审查证据收敛。

缺少主任务、关键状态、权限来源、危险操作后果或平台事实时应记录 `blocked/assumption`，不能用视觉稿静默补全业务。

## 4. UI Decision Contract

适用任务的设计产物至少覆盖以下语义；它们是合同角色，不要求机械生成同名文件：

```yaml
classification:
  product_type: { value: erp, claim_type: fact, confidence: 1.0, proof_claim_id: claim-product, assumption_id: "" }
  business_domain: { value: production_planning, claim_type: fact, confidence: 1.0, proof_claim_id: claim-domain, assumption_id: "" }
  page_pattern: { value: workbench, claim_type: inference, confidence: 0.86, proof_claim_id: "", assumption_id: assumption-pattern }
  profile_id: { value: erp_mes_dense, claim_type: inference, confidence: 0.90, proof_claim_id: "", assumption_id: assumption-profile }
  platform: { value: web_desktop, claim_type: fact, confidence: 1.0, proof_claim_id: claim-platform, assumption_id: "" }
  density: { value: compact, claim_type: inference, confidence: 0.90, proof_claim_id: "", assumption_id: assumption-profile }
  motion_level: { value: 0, claim_type: inference, confidence: 0.90, proof_claim_id: "", assumption_id: assumption-profile }
  operation_frequency: { value: high, claim_type: fact, confidence: 1.0, proof_claim_id: claim-frequency, assumption_id: "" }
  data_density: { value: high, claim_type: fact, confidence: 1.0, proof_claim_id: claim-density, assumption_id: "" }
  risk_level: { value: high, claim_type: inference, confidence: 0.82, proof_claim_id: "", assumption_id: assumption-risk }
  primary_user: { value: production_planner, claim_type: fact, confidence: 1.0, proof_claim_id: claim-user, assumption_id: "" }
  primary_task: { value: create_and_confirm_schedule, claim_type: fact, confidence: 1.0, proof_claim_id: claim-task, assumption_id: "" }
  rationale: >-
    Requirement evidence identifies an ERP production-planning desktop task;
    the workbench/profile/risk choices remain explicit inferences until reviewed.

profile_overrides: []

flow:
  entry: schedule-list
  preconditions: []
  main: []
  alternatives: []
  errors: []
  recovery: []
  cancel: []
  exit: []
  context_preservation: []

state_model:
  states: []
  transitions: []
  terminal_states: []

action_policy:
  actions: []
  permission_source: server-authoritative
  stale_and_conflict_behavior: []

presentation:
  information_hierarchy: []
  component_tree: []
  token_contexts: [light, dark, reduced-motion]
  responsive_rules: []

proof_obligations:
  behavior: []
  accessibility: []
  visual: []
  performance: []
```

`classification` 的十二个值都使用同一个 `classified_value` 结构。`fact` 必须 `confidence: 1.0`、引用
`classification_fact` proof claim 且不引用 assumption；`inference` 的置信度必须严格位于 0 与 1 之间、只引用可解析的
assumption，不能把推断伪装成事实。Profile、Pattern 与 Platform 分别只接受 catalog 中的 canonical ID。

`profile_overrides` 是必需的顶层列表，不是任意 theme patch。选择值与固定 Profile 的 `density/motion_level` 默认值相同时填
空列表；发生偏离时，列表必须恰好覆盖每个偏离字段，并记录 `field/default/selected/reason/scope/risk`、补偿 proof claim、
独立 reviewer 和失效日期。它只能覆盖这两个 Profile 启发式，不能覆盖标准、权限、业务状态或其他分类字段；结构通过也不证明
例外理由真实或 reviewer 身份已认证。

```yaml
profile_overrides:
  - field: motion_level
    default: 0
    selected: 1
    reason: Preview transition needs continuity feedback.
    scope: schedule-preview
    risk: medium
    compensating_proof_claim_ids: [claim-profile-override]
    reviewer_id: reviewer-02
    expires_at: 2026-12-31
```

其中补偿 claim 的 subject 必须是 `<profile_id>:<field>`，proof type 为 `profile_override_review`，且 `reviewer_id` 必须与 package
独立 Review 一致。override 的 `risk` 参与有效风险与 L1–L4/W1–W3 floor 计算，不能用较低的顶层分类掩盖高风险偏离。
到期或 Profile 默认值变化后必须重新评估，不能把旧例外永久继承。

事实、组织策略、设计推断、假设和建议必须分开。UI Contract 不得包含 `accepted/completed/approved/verdict`，
也不得把 Agent 自评或 screenshot score 铸造成完成状态。

## 5. 场景 Profile 与页面 Pattern

`Scenario Profile` 回答“在什么环境中以何种密度、风险和视觉语言工作”；`Page Pattern` 回答“任务如何组织”。
同一 Pattern 可应用多个 Profile，同一 Profile 也可组合多个 Pattern：

```text
UI solution = domain contract × scenario profile × page pattern × design system × platform adapter
```

### 5.1 场景 Profile

| Profile | 优先目标 | 常见特性 | 特别义务 |
|---|---|---|---|
| `cms_editorial` | 编辑、预览、审核、发布 | 标准密度、编辑主区、设置侧栏、版本状态 | 草稿保护、未保存提醒、预览与版本恢复 |
| `oa_workflow` | 低学习成本、流程可见 | 克制视觉、待办/期限/当前节点突出 | 原因、流向、撤回/驳回、审计反馈 |
| `erp_mes_dense` | 高效率、低错误、密集数据 | desktop-first、表格/工作台、低装饰动效 | 批量结果、并发冲突、编辑保留、键盘效率 |
| `crm_relationship` | 关系、阶段和下一行动 | 客户摘要、负责人、时间线 | 阶段原因、跟进闭环、资源范围权限 |
| `analytics_decision` | 理解口径与趋势 | 指标、筛选、图表、下钻、明细 | 单位/口径/更新时间、筛选范围、空值与故障区分 |
| `commerce_transaction` | 交易正确、承诺透明 | 商品、购物车、结算、订单状态 | 价格/库存真值、重复提交、支付恢复 |
| `mobile_task` | 聚焦移动任务 | 舒适密度、触摸、原生导航与表单 | safe area、方向、离线和平台语义 |
| `marketing_conversion` | 解释价值并转化 | 宽松密度、品牌叙事、一个主要 CTA | 目标清楚、表单最小化、移动首屏和提交后续 |
| `immersive_story` | 探索与叙事 | 高动效、滚动/3D/声音可选 | skip/pause、reduced motion、加载/失败/性能降级 |
| `data_wall` | 远距离掌握固定屏状态 | fixed-canvas、指标、趋势、告警 | 刷新稳定、数据陈旧提示、轮播暂停与降级 |
| `ai_agent_workspace` | 表达目标和控制长任务 | 会话、工具状态、阶段进度、结果卡片 | 中断/修正、来源、等待用户、高风险确认 |
| `generic_saas` | 在未知但有界场景完成主任务 | 标准密度、导航、内容、action、反馈 | 状态可见、失败可恢复、响应式行为 |

Profile 中的 `product_types` 与 `preferred_patterns` 是路由启发式，不是硬兼容矩阵；有 source-bound 项目 Profile 时项目事实优先。
Profile 中的 spacing、density、motion、shape 和 breakpoint 都是可配置 token/context，不是通用事实。

### 5.2 页面 Pattern

- `list`：筛选、工具栏、结果、批量操作、分页/游标和上下文恢复；
- `detail`：对象身份、状态、关键 action、关联信息、时间线与审计；
- `form`：语义分组、校验、草稿/提交区分、未保存保护和失败后内容保留；
- `workbench`：多面板、密集操作、选择上下文、快捷键、并发/脏状态；
- `wizard`：步骤、前置条件、中间保存、返回和最终复核；
- `editor`：内容面、设置、保存状态、预览和版本恢复；
- `approval`：请求上下文、当前节点、证据、决策 action 与历史；
- `dashboard`：全局筛选、口径、指标、下钻、异常和明细；
- `landing`：价值、解释、证明、主要转化和提交反馈；
- `immersive`：加载、引导、探索、可访问内容、降级和退出/转化；
- `data_wall`：数据新鲜度、关键状态、趋势、告警与运行上下文；
- `agent_chat`：消息、工具/任务状态、暂停/修正、来源和结果；
- `visual_editor`：资源、画布、属性、工具、撤销/重做、保存和恢复；
- `timeline`：范围、有序事件、事件详情、筛选与分页/流式加载。

Pattern 是起点，不是页面生成器。Agent 应按主任务删除不适用区段，不为“模板完整”制造无价值卡片或操作。

## 6. Flow、State、Action 与 Permission

### 6.1 Flow 是完整任务，不是 happy path

每条关键 flow 至少声明：`actor/goal/entry/preconditions/main/alternatives/errors/recovery/cancel/back/exit/context_preservation`。
列表返回后的筛选、分页、滚动和选择是否保留；弹窗、抽屉还是独立页面；URL 是否可分享；浏览器/系统返回是否有效，
都属于 flow 合同。

异步操作必须区分“请求已接收、正在执行、等待输入、部分成功、成功、失败、取消、结果未知”。保存失败时保留用户输入；
批量操作报告总数、成功数、失败数和可恢复下一步。不可逆、高影响或法律/金融/用户数据提交应根据风险提供撤销、复核纠正
或提交前确认，而不是对所有按钮机械弹确认框。

### 6.2 State 与 transition

复杂业务使用显式状态与 transition，不用多个互相矛盾的 boolean 拼装。每个 transition 声明：

- source/target state、触发 action、业务 guard、权限、数据条件和系统条件；
- optimistic/pessimistic UI 策略、等待和未知结果；
- 成功/失败/冲突/取消后的状态、通知、焦点和恢复动作；
- 是否可逆、是否需要原因/审计、谁是服务端权威。

前端 action availability 由 `business state × permission × data condition × system state` 得出。前端隐藏按钮不是授权；
服务端仍必须执行最终权限和状态校验。权限加载失败不得默认放行，权限变化和 stale data 必须有明确行为。

## 7. Business UI Geometry Contract

页面不是组件树，而是目标角色完成业务任务的控制面。构图顺序必须是：

```text
authoritative business goal / role / flow / state / action / data meaning
  → information priority and page states
  → regions and visual groups
  → alignment axes and negative-space relationships
  → strokes, shape language and responsive dispositions
  → framework implementation
  → exact-context measurement + independent visual/business review
```

Geometry 只能引用既有业务合同，不能重新定义 Actor、权限、状态转换或数据口径。同一业务对象对操作、监督、审批、分析、配置、
审计等 work mode 可以采用不同 view；这不是按职位复制页面，也不是前端授权。主要 flow、高风险 action 和关键反馈必须有
load-bearing UI trace，装饰元素无需登记成伪精确 DOM 合同。

### 7.1 Versioned composition artifact

适用任务在 `FrontendDesignPackage.evidence_artifacts` 中提供一个或多个 digest-bound `source` artifact：

```yaml
kind: source
media_type: application/vnd.forgeos.business-ui-composition+json
```

内容版本为 `forgeos.business-ui-composition/v1`，并由 `layout_component_composition` 决策的
`business_ui_composition` proof 精确引用。下面只展示字段骨架；为缩短篇幅而写成 `[]` 的集合不是适用任务的合法完成实例，
实际内容必须满足后述 trigger floor、引用闭合和 load-bearing trace：

```json
{
  "api_version": "forgeos.business-ui-composition/v1",
  "id": "production-schedule",
  "surface_id": "schedule-workbench",
  "views": [
    {
      "id": "scheduler-operation",
      "actor": "scheduler",
      "work_mode": "operation",
      "flow_ids": ["create-schedule"],
      "primary_questions": ["哪些订单尚未排程", "当前排程是否存在冲突"]
    }
  ],
  "data_semantics": [],
  "page_states": [],
  "regions": [],
  "axes": [],
  "groups": [],
  "spacing_relations": [],
  "strokes": [],
  "shape_rules": [],
  "responsive_variants": [],
  "load_bearing_elements": [],
  "optical_adjustments": []
}
```

各集合承担不同职责：

- `views` 把 Actor/work mode/primary questions 绑定到已有 flow，不复制角色权限；
- `data_semantics` 区分 `business_fact/computed_judgment/ai_recommendation/derived_display`，声明 authority、definition、unit、
  time basis、freshness、access、uncertainty、explanation、human confirmation 和明确 null semantics；
- `page_states` 表达 loading/empty/partial/stale/denied/conflict/offline/unknown-result 等 UI 状态影响哪些 region、是否保留旧数据、
  对应哪个已有 action/recovery；它不替代业务对象 `state_model`；
- `regions/axes/groups` 表达主工作区、辅助区、视觉组和主次对齐轴；parent 必须无环且引用闭合；
- `spacing_relations` 表达同组件、同组、相邻模块或大区域等语义关系；negative space 是合同内容，不是“随便加 margin”；
- `strokes` 只允许 boundary/separator/guide/relationship/emphasis 目的，并声明起止 anchor；
- `shape_rules` 绑定批准的形状家族；`optical_adjustments` 显式记录非结构性光学校正及独立 reviewer；
- `responsive_variants` 对每个关键 region 声明 `present/deferred/omitted_with_reason`，以任务重排代替机械缩小；
- `load_bearing_elements` 只登记影响任务、判断、action、data 或 feedback 的元素。

空间 ownership 采用 containment：页面级 axis 可以覆盖其 scope region 与后代，但 region/element 不得引用无关子树的 axis；group 的 primary
axis scope 必须包含 group region，group member 必须位于 group region 子树，每个 load-bearing element 至少属于一个 group。
`axis.member_refs` 是精确成员集：它与 region/element `axis_refs`、group `primary_axis_ref` 双向闭合，不是抽样 anchor 列表。这样既保留共享
页面轴，也拒绝单向声明或跨区引用掩盖结构漂移。

`form_or_table/data_intensive/high_risk_action/authentication_or_payment/destructive_user_data/regulated_commitment/safety_critical_surface`
等数据承载触发器要求非空且有空间 trace 的 `data_semantics`；其中 `data_intensive` 至少声明一个可恢复的非正常数据 page state。
带 action/recovery 的 page state 必须显式绑定非空 canonical `business_state_ids`；首次 loading 等纯等待/展示 state 可在业务对象出现前留空。
只要声明 recovery，就必须为每个覆盖业务状态提供 source state 匹配的 executable recovery action。
`multi_role_permission` 必须解析到至少两个既有 flow actor/view、逐 view/flow 的 load-bearing spatial trace 和 denied recovery；
`authentication_or_payment` 必须解析到既有 state-model 高风险 action、可恢复 risk page state 与 load-bearing feedback trace。纯只读
`safety_critical_surface` 不伪造业务 action，但仍需可恢复 risk/non-normal page state；若实际存在高风险 action，同样要求 feedback trace。
它们只验证 canonical 模型的下限，不另建角色、权限或状态 taxonomy。

间距、线宽、圆角、形状、光学修正和测量容差只接受 `token:/policy:/profile:` symbolic reference。ForgeOS 不把 4/8pt、
三条轴、1.5px 容差、特定圆角或 90 分设成跨项目硬标准。项目可采用这些值，但需 owner、范围、版本和验证证据。

### 7.2 数据可信与业务解释

`0`、未知、不可用、无权限、不适用和未计算不得统一显示成 `-`。重要信息说明来源、口径、单位、时间范围、新鲜度和 access；
computed judgment 与 AI recommendation 不能伪装成业务事实。AI recommendation 必须标识依据/不确定性并由人确认，尤其不得通过
视觉强调绕过高风险 action 的既有 permission/data/system guard。

页面优先级由 Frequency × Importance × Risk 决定：高频低风险 action 靠近主工作区；低频高风险 action 与主操作隔离并说明后果、
影响、原因、审计和恢复。按钮位置或组件能力不构成业务理由。所有动效必须能解释其状态、关系、方向或因果；解释不了的装饰动效删除。

### 7.3 Geometry measurement 与审美边界

真实项目有 Web/native runner 时，可在同一个 `capture` verification case 中附加：

```yaml
kind: tool_output
media_type: application/vnd.forgeos.ui-geometry-report+json
proof_type: geometry_measurement_receipts
```

`forgeos.ui-geometry-report/v1` 必须精确绑定 composition digest、case、source tree、build、fixture 和 canonical environment digest，
记录 runner name/version、逐条 subject、原始 observation、由项目 policy 提供的 tolerance、required 标记及
`passed/failed/inconclusive/not_executed`。每份报告至少有一条 required assertion；只给总分、全标 optional、复用另一 case 的报告、漏原始值或用 passed claim 掩盖 required failure 均无效。

报告还必须声明唯一的公共 `coordinate_space`：单位只能是 `css_px`、`logical_dp` 或 `device_px`，原点固定为 capture viewport 左上角，轴向固定为
右/下，并记录 `device_pixels_per_unit`。CSS/native logical unit 的比例必须等于 capture environment 的 DPR，device pixel 的比例必须为 `1`；
报告内所有 observation 和 tolerance 都使用该公共坐标系。不得把 `getBoundingClientRect()` 的 CSS pixel、Flutter/React Native logical unit 与
截图 raster pixel 混在同一报告中比较。

结构 validator 能检查 ID、引用、parent cycle、token ref、业务 trace、响应式处置和 report/capture context 一致性；DOM rectangle 能检查
边缘、轴线、间距、溢出与 anchor。它们不能判断任务是否真的合理、信息顺序、视觉重心、阅读动线、留白美感、非对称平衡、字体/SVG
光学中心或动画的业务意义，这些继续由三个 canonical owner 与 fresh Reviewer 判断。

当前 artifact `provenance` 仍是 declarative：摘要只能证明本地字节一致，不能认证浏览器、工具或 Reviewer。没有真实 runner 时必须
如实记录 `not_executed`；required assertion 的 `failed/inconclusive/not_executed` 必须使相应 visual readiness 为 not-ready。
`STRUCTURALLY_VALID`、截图和 advisory score 均不产生完成权。

### 7.4 真实环境与长期工程经验

审查不能只使用整洁 demo data。按风险覆盖：长/空/缺失/重复/异常字符、极值/负数/多语言、慢网/断网、部分成功、陈旧数据、并发冲突、
权限变化、多个标签页、刷新/返回/中断恢复和支持窗口。URL、草稿、筛选、滚动、选择与焦点是否恢复由 flow/context contract 决定；
Geometry 负责在各状态中保持空间和任务连续性，但不能静默决定业务默认值。

最终优先级为：业务正确 → 安全 → 可理解/可预测 → 可恢复/可追溯 → 高效 → 行为与视觉一致 → 可访问 → 稳定/快速 → 可维护 →
新颖。几何美感来自这些约束共同形成的秩序，而不是额外装饰。

## 8. 信息架构与 Design System 治理

信息架构围绕用户任务和业务对象组织，不照抄数据库表或后端 endpoint。页面只有一个最高优先级主任务；主要、次要、
低频与危险 action 应有稳定层级。颜色不能成为状态或错误的唯一表达。

Design Token 使用固定版本的 DTCG 格式时，应验证 typed `$value`、引用解析、循环、token/group 歧义、deprecated 使用，
并为 light/dark、size、high-contrast、reduced-motion 等适用 context 解析确定值。构建证据应记录 token source、版本、context 和摘要。

DTCG 是 W3C Community Group Final Report，不是 W3C Recommendation。`foundation/semantic/component` 分层、禁止 raw value、
4/8pt spacing scale 等仍是组织策略，需明示 owner、作用域和例外，不能冒充 DTCG 要求。

## 9. Accessibility、Responsive 与 Motion

### 9.1 Web

- 以目标 WCAG 2.2 conformance level 精确追踪 Success Criteria；AA 常见底线包括普通文本 4.5:1、大文本 3:1、
  有意义 UI/图形 3:1、颜色非唯一载体、键盘可操作、焦点可见/有序、状态消息可感知和 320 CSS px 等价 reflow；
- 原生 HTML 优先。布局 `<div>` 合法；禁止的是把非语义元素当按钮/链接却缺键盘、名称、状态和焦点行为；
- APG 是非规范性 pattern guidance，不是生产 Design System。采用 modal/grid/combobox 等 Pattern 时固定其版本和完整
  keyboard/focus contract。普通静态表格不要为了“高级”改成需要作者管理焦点的 ARIA grid；
- 自动扫描只能发现一部分问题。中高风险 flow 还需键盘、焦点、缩放、读屏和人工判断证据。

### 9.2 响应式与国际化

按内容和可用容器空间分支，而非仅按设备名称。测试矩阵覆盖支持范围内的边界值、文本/显示缩放、长翻译、RTL、
横竖屏和 keyboard/touch/pointer。必须二维呈现的数据表或画布可以拥有自己的滚动区域，但页面其余内容仍应 reflow。

### 9.3 Motion

动效必须表达层级、连续性、状态或反馈。响应 `prefers-reduced-motion`/平台 reduced-motion 偏好，减少或替换非必要运动；
这不要求删除所有必要反馈。沉浸式 Profile 还需 skip/pause、非可视暂停、资源渐进加载和低性能降级。

## 10. 平台与框架映射

FrontendDesignPackage v1 的 canonical `platform` 只有 `web_desktop`、`web_responsive`、`ios`、`android` 和
`cross_platform`。React、Vue、Flutter 与 React Native 是实现 adapter/stack，不是 platform ID；例如 React 响应式 Web 使用
`web_responsive`，Flutter 同时面向 iOS/Android 时使用 `cross_platform`，并在项目事实中另行记录 framework/version。

### 10.0 Client Code Architecture

当变更涉及 route/page/feature/shared/public client contract、跨模块 import、God Page 或结构迁移时，条件化装载
`frontend-code-architecture`。先声明或确认项目自己的 architecture profile，再输出 module responsibility/owner/public API、
state/data/effect ownership 和计划/实际 change surface；不得把 feature-sliced、Flutter clean 或任意固定目录层级当作所有项目的标准。

确定的反向依赖、cycle 和 deep import 由独立 shadow detector 报告；God File、目录/API 数量、shared 准入与 change amplification
仍需独立语义审查。完整合同、例外、baseline 与逐步提升门禁条件见
[Frontend Code Architecture Standard](frontend-code-architecture-standard.md)。

### 10.1 React

组件和 Hook 的 render 保持纯；Effect 用于同步外部系统，不用于保存可由 props/state 推导的数据；避免矛盾、冗余和重复 state；
列表 key 使用稳定数据身份，`useId` 只用于可访问性关系而非 list key。页面/route 负责 composition，业务规则不藏在 UI effect 中。

### 10.2 Vue

使用原生语义、landmark、heading、label、button type、autocomplete 和 route focus；先测量再做 code split、virtualization 或
reactivity 优化。Vue 支持 class/style binding，是否禁止 inline style 属项目策略，不是 Vue 正确性规则。

### 10.3 Flutter

按可用 window/local constraints 自适应，不靠设备类型猜布局；为 Semantics、focus traversal、键盘、触摸、TalkBack/VoiceOver、
文本与显示缩放提供验证。Flutter 的 48×48 target 建议属于该平台，不覆盖 Web WCAG 的 CSS px 规则。Golden test 要固定平台、
Flutter 版本和字体环境。

### 10.4 React Native

iOS 与 Android 分别映射 accessibility label/role/state/value/live region/action，验证 VoiceOver 与 TalkBack，并响应 reduced motion。
Pressable hit area、FlatList window/batch 等参数按设备与数据实测权衡，不设置全局“超过 N 行必须虚拟化”的伪标准。

平台无关合同只保存业务语义、状态、action intent 和 proof obligation；各 adapter 对原生语义、输入、导航和辅助技术负责，
不得用最低公共分母抹平平台差异。

## 11. Evidence Pipeline

推荐证据顺序如下；实际项目仅能声明已真实运行的步骤：

```text
UI contract / flow review
  → framework static checks and typecheck
  → component/integration behavior tests
  → Playwright or native end-to-end user flow
  → automated accessibility scan
  → explicit keyboard/focus/AT checks
  → deterministic screenshots or native goldens
  → token/geometry/contrast checks where tooling exists
  → advisory vision review
  → independent human review for material risk
```

Playwright 测试应以用户可见行为、role 和 accessible name 定位，依赖 actionability 自动等待；不以脆弱 CSS/XPath 证明行为。
视觉基线至少绑定 browser/version、OS、font、DPR、viewport、locale、timezone、theme、reduced-motion、数据 seed 和代码摘要；
保存 baseline/current hash、diff、mask/style config 和 trace。生成与比较必须在相同环境，基线变化需审查。

像素 diff 只证明视觉变化，不证明审美、业务正确或可访问。axe 等自动扫描不能覆盖全部 WCAG；Vision Agent 的 90 分也只是
`advisory`。主任务不可达、焦点陷阱、未授权 action、保存丢数据、高风险操作无恢复等必须独立作为 finding，不能被总分抵消。

性能预算按 Profile、目标设备和真实场景设定。Web 以 LCP/INP/CLS 和 field data 为主，实验室数据辅助；React/Vue/Flutter/RN
都应先 profile，再决定 memo、virtualization、lazy load 或动画降级，不能机械优化所有组件。

## 12. Shadow、权限与完成边界

- 本文件、三张 canonical owner Skill、`ui-geometry` supporting Skill、byte-pinned policy/catalog/schema 与 shadow checker 已形成
  机器可校验合同；仍没有
  自动 diff compiler、runtime context selector、可信 screenshot service、vision grader 或载重 pre-code authority；
- v1 对 verification case 的 `source_tree_sha256/build_sha256/fixture_id/environment` 只验证声明形状、允许值、交叉引用和包内一致性；
  每个 verification case 声明的 artifact 集合必须与其 exact-subject claims 的 artifact 集合完全一致；对本地 evidence artifact
  只验证有界字节与声明摘要，对 screenshot 额外验证 PNG chunk/CRC、未知 critical chunk、`PLTE`
  约束、`IDAT` 连续性、受限 zlib 像素流、scanline filter 与 viewport×DPR 尺寸。它不会重新构建代码、
  重建 fixture、认证浏览器/设备/字体环境，也不能证明 PNG 或其他 artifact 确由声明工具、source、build 和环境产生；
- adapter 可以形成设计/审查产物，不能自授予代码、外部系统、生产、发布或规则学习权限；
- Review、截图、无障碍和性能结果必须绑定实际工具/环境与代码版本；工具不存在或未运行时标记 `not_executed`；
- Agent 自评、手写 `passed`、视觉总分和结构合法均不能产生完成权；
- `forge accept` 仍是唯一完成裁决权威，宿主 hook、Skill、报告和 shadow detector 只能提供输入；
- 后续若要从 shadow 进入载重执行，必须另行实现可信 Runner/receipt、自动 routing、provenance 与 authorization，并补充 ADR。

## 13. 一手资料

### W3C / WAI

- [WCAG 2.2](https://www.w3.org/TR/WCAG22/)
- [Understanding Target Size (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum)
- [Understanding Reflow](https://www.w3.org/WAI/WCAG22/Understanding/reflow.html)
- [Understanding Non-text Contrast](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html)
- [Understanding Use of Color](https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html)
- [Understanding Status Messages](https://www.w3.org/WAI/WCAG22/Understanding/status-messages.html)
- [Understanding Error Prevention](https://www.w3.org/WAI/WCAG22/Understanding/error-prevention-legal-financial-data.html)
- [ARIA Authoring Practices Guide](https://www.w3.org/WAI/ARIA/apg/) 与
  [APG Introduction](https://www.w3.org/WAI/ARIA/apg/about/introduction/)
- [SCXML 1.0](https://www.w3.org/TR/scxml/)
- [Authoring HTML for languages with directionality](https://www.w3.org/International/questions/qa-html-dir)

### Design Token

- [DTCG Format Module 2025.10](https://www.w3.org/community/reports/design-tokens/CG-FINAL-format-20251028/)
- [DTCG Color Module 2025.10](https://www.w3.org/community/reports/design-tokens/CG-FINAL-color-20251028/)
- [DTCG Resolver Module 2025.10](https://www.w3.org/community/reports/design-tokens/CG-FINAL-resolver-20251028/)

### Framework 与验证

- React: [Components and Hooks must be pure](https://react.dev/reference/rules/components-and-hooks-must-be-pure)、
  [Choosing the State Structure](https://react.dev/learn/choosing-the-state-structure)、
  [You Might Not Need an Effect](https://react.dev/learn/you-might-not-need-an-effect)
- Vue: [Accessibility](https://vuejs.org/guide/best-practices/accessibility.html)、
  [Performance](https://vuejs.org/guide/best-practices/performance)
- Flutter: [Accessibility](https://docs.flutter.dev/ui/accessibility)、
  [Adaptive and responsive design](https://docs.flutter.dev/ui/adaptive-responsive/general)、
  [Testing overview](https://docs.flutter.dev/testing/overview)
- React Native: [Accessibility](https://reactnative.dev/docs/accessibility)、
  [AccessibilityInfo](https://reactnative.dev/docs/accessibilityinfo)、
  [Optimizing FlatList Configuration](https://reactnative.dev/docs/optimizing-flatlist-configuration)
- Playwright: [Visual comparisons](https://playwright.dev/docs/test-snapshots)、
  [Accessibility testing](https://playwright.dev/docs/accessibility-testing)、
  [Best Practices](https://playwright.dev/docs/best-practices)
- web.dev: [Web Vitals](https://web.dev/articles/vitals)、
  [prefers-reduced-motion](https://web.dev/articles/prefers-reduced-motion)、
  [Animations and performance](https://web.dev/articles/animations-and-performance)

这些资料提供规范或平台事实；本文件中的 Profile、Pattern、流程组合和证据分级是 ForgeOS 工程策略，不能写成外部资料原文。
