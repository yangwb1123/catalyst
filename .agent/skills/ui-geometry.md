# Skill: ui-geometry

> 先把既有业务、角色、任务和状态关系编排成可追踪的空间约束，再将约束交给前端实现与真实渲染验证。

## 职责与触发 (Responsibility & triggers)

用于新建/重排页面、表单、表格、工作台、共享布局、响应式、数据密集、品牌、沉浸式、多角色或高风险 UI。它是
`information-interaction-design`、`design-system-accessibility` 与 `frontend-client-engineering` 之间的 supporting procedural
Skill，不拥有新的 fine capability，也不建立第二套业务、权限、状态或 Design Token 真值。

- 上游业务目标、Actor、flow、state/action/permission 和页面状态由 `information-interaction-design` 提供；
- 本 Skill 编排 load-bearing region/axis/group/negative-space/stroke/shape/reflow 关系；
- token、组件、光学与独立视觉判断仍由 `design-system-accessibility` 主拥有；
- DOM/platform measurement、截图和代码由 `frontend-client-engineering` 在项目真实工具链内执行。

纯文案、非布局的叶子行为修复可 N/A。缺少主任务、关键状态、权限权威、数据语义或项目 token 时，不得用通用模板填空。

## 输入契约 (Inputs)

- 当前 `FrontendDesignPackage` 的 classification、flows、state model、action guards、readiness 和 source-bound evidence。
- 角色工作模式、主问题、任务频率/重要性/风险、信息优先级和适用 page state；这些只能引用权威来源，不得发明业务真值。
- 数据呈现语义：business fact / computed judgment / AI recommendation / derived display，以及 source、definition、unit、time basis、
  freshness、access、uncertainty、explanation、confirmation 和 null semantics。
- 项目/Profile 的 token、组件、viewport/window/input matrix、真实浏览器或平台测试命令。
- 未知、不可用、无权限、不适用、未计算与数值 `0` 必须保持不同语义；未知 load-bearing 输入按 AFDS assumption policy 阻断。

## 执行 SOP (Procedure)

1. **绑定业务**：从既有 flow/action/state 中选择本 surface 的 Actor、work mode、判断问题和 load-bearing task，不复制其定义。
2. **分配信息**：区分首屏、操作时、异常时、按需和审计信息；数据库/API 字段顺序不得直接成为 IA。
3. **划分区域**：定义 region、父子边界、语义目的和优先级；每个区域说明服务的 view/flow/action/data。
4. **建立坐标系**：复用少量 page/module axis，定义 group 与主要 alignment；区分数学对齐和需要审查的光学修正。
5. **表达关系**：用 token-backed negative space、stroke 和 shape family 表达关系；优先留白，其次背景/阴影/分隔/边界。
6. **追踪元素**：只登记 load-bearing element，并映射到 flow/action/data/page feedback；禁止登记整个 DOM 造成伪精确合同。
7. **设计重排**：每个响应环境显式声明 region present/deferred/omitted-with-reason；响应式重新排序任务，不只缩放桌面。
8. **编译实现**：实现者只从项目 token/policy/profile 取值；原始像素、负 margin 或 transform 例外必须有明确来源和原因。
9. **测量渲染**：项目有真实 runner 时，绑定 composition/source/build/fixture/environment，输出原始 observation、policy tolerance 和逐条结果。
10. **双重审查**：DOM/platform report 只审数学几何；截图与 fresh Reviewer 审视觉重心、阅读动线、留白、光学和业务适配。

## 输出契约 (Outputs)

- `application/vnd.forgeos.business-ui-composition+json` source artifact，版本
  `forgeos.business-ui-composition/v1`，由 `layout_component_composition` 决策的 `business_ui_composition` proof 精确引用。
- Artifact 至少包含 views、data semantics、page states、regions、axes、groups、spacing relations、strokes、shape rules、responsive
  variants、load-bearing elements 和显式 optical adjustments。
- 项目实际执行时可附 `application/vnd.forgeos.ui-geometry-report+json` tool output；它必须绑定同一 capture case 的 composition digest、
  source tree、build、fixture 和 environment，并包含 runner、原始 observations、policy-sourced tolerance 与 required result。
- Geometry finding、未执行项、残余风险和独立 Review；不得输出 `geometry_passed/approved/completed/verdict` 或用总分代替 finding。

## 规则、禁止与权限 (Rules, prohibitions & authority)

- 每个可见 load-bearing 元素必须归属 visual group，并能追溯到角色任务、业务判断、action、data 或必要 feedback；多角色 view/flow 配对必须有这样的空间 trace，不能只在合同中声明幽灵角色。
- Primary flow 与高风险 action 必须存在 load-bearing UI trace；action 可用性仍由业务状态 × permission × data guard × system state 决定。
- `axis.member_refs` 是精确成员集，必须与 region/element 的 `axis_refs` 以及 group 的 `primary_axis_ref` 双向一致，并满足 region containment。
- 带 action/recovery 的 page state 必须列出非空 canonical `business_state_ids`；纯等待/展示 state 可在业务对象出现前留空。声明 recoverable 时，每个覆盖状态至少有一条 source state 匹配的 executable recovery action。
- `authentication_or_payment` 必须解析到高风险 action 和可恢复 risk state；纯只读 `safety_critical_surface` 不伪造 action，但仍提供可恢复 risk/non-normal page state，若存在高风险 action 则继续要求 load-bearing feedback trace。
- 所有 spacing/stroke/shape/optical/tolerance 值引用 `token:/policy:/profile:`；本 Skill 不发布全局 8pt、断点、圆角、线宽或像素容差。
- 每条线必须声明 boundary/separator/guide/relationship/emphasis 目的和有效 anchor；不得用 Card/border 包裹所有内容。
- Computed judgment 和 AI recommendation 不得伪装为事实；AI recommendation 必须人工确认，且不能因视觉强调获得执行权限。
- 禁随机像素、无解释负 margin、用 transform 隐藏结构错误、不同区域各造坐标系、只缩放不重排、只给 geometry score。
- 本 Skill 只产出设计/审查合同；没有安装依赖、启动外部设计服务、生产写入、发布或自行批准的权限。

## 自动化与验收 (Automation & acceptance)

- ForgeOS 的 shadow validator 只能确定严格 JSON、ID/引用、region parent cycle、token ref、flow/action trace、响应式处置与
  report/capture context 一致性；它不能判断视觉美感或证明声明的 runner 真实执行。
- DOM Web 测量、Flutter/RN golden/platform geometry 必须使用项目已配置 runner。没有 runner 时记录 `not_executed` 并把相应 readiness
  标为 not-ready；不得伪造 `getBoundingClientRect()`、截图或命令输出。
- Report 至少包含一条 required assertion；只有 required assertions 全部 passed 才能支持 visual readiness，其 failed/inconclusive/not-executed 不能被平均分、截图相似或 Visual Review 抵消；业务任务、权限、数据丢失和
  accessibility finding 始终优先。
- 验收同时检查：业务可追踪、引用闭合、关键状态与恢复、项目 token、响应重排、原始测量、对抗数据、独立业务/视觉 Review。
  结构有效仍只是 `FrontendDesignPackage` 的 Review 输入，完成权仅归 `forge accept`。

## 直接参考 (References)

- `.agent/engineering/frontend-design-gates.yml`
- `.agent/eval/frontend-design-package.schema.yml`
- `.agent/skills/information-interaction-design.md`
- `.agent/skills/design-system-accessibility.md`
- `.agent/skills/frontend-client-engineering.md`
- `docs/design/ai-engineering-os/frontend-design-standard.md#7-business-ui-geometry-contract`
- `harness/frontend_design/composition.py`
- `harness/frontend_design/geometry.py`
