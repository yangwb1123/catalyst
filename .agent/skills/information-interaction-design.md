# Skill: information-interaction-design

> 把用户目标转成可追踪的信息架构、任务流程和交互状态合同；不以静态页面代替业务行为设计。

## 职责与触发 (Responsibility & triggers)

用于新建或修改用户流程、导航、表单、列表/工作台、状态操作、高风险提交、异步任务或上下文返回行为。它唯一拥有
information-architecture、interaction-design 和 content-design 能力；任务分类、产品 flow、页面 IA、业务状态机是本 Skill
按需激活的 lens，不再创建平行 Skill。纯视觉 token 调整交给 `design-system-accessibility`，框架代码交给
`frontend-client-engineering`。

## 输入契约 (Inputs)

- 业务目标、Actor、主任务、成功结果、非目标、验收条件和真实研究/运行证据。
- 现有路由、页面、术语、权限来源、业务状态/规则、数据新鲜度与平台环境。
- 缺关键状态、不可逆 action 后果或权限权威时，记录 assumption/open question；高影响未知不得由设计者默认填空。
- 只在需要某项 lens 时读取 [AFDS](../../docs/design/ai-engineering-os/frontend-design-standard.md) 对应章节，避免装载全部 Profile。

## 执行 SOP (Procedure)

1. 以 FrontendDesignPackage v1 的 `classified_value` 分类产品、业务域、主用户/任务、频率、密度、风险、canonical
   `profile_id/page_pattern/platform`；分类只用于路由，不决定业务规则。
2. 围绕用户任务组织对象、内容、导航、搜索/筛选、入口和出口；不得照抄数据库或 endpoint 结构。
3. 为关键任务写 entry/preconditions/main/alternatives/errors/recovery/cancel/back/exit/context-preservation。
4. 对有生命周期的对象建立 state/transition；为 action 写业务 guard、permission、data/system condition、可逆性和未知结果。
5. 设计反馈、焦点、脏状态、并发冲突、部分成功和失败后保留；批量和异步 flow 给出可恢复下一步。
6. 从 `list/detail/form/workbench/wizard/editor/approval/dashboard/landing/immersive/data_wall/agent_chat/visual_editor/timeline`
   中选择最低充分 canonical Pattern，并记录删除的模板区段。
7. 将内容层级、flow、state/action/permission 和 proof obligations 交给设计系统与实现 Skill；重大业务未知回流需求/产品 owner。

## 输出契约 (Outputs)

- `FrontendDesignPackage.classification`：十二个分类值与 `rationale`；每个值都使用
  `{value, claim_type, confidence, proof_claim_id, assumption_id}`，不得输出裸字符串。例如
  `page_pattern: { value: workbench, claim_type: inference, confidence: 0.86, proof_claim_id: "", assumption_id: assumption-pattern }`。
- IA/navigation map、screen inventory、content hierarchy 和对象术语。
- `TaskFlow[]`、`InteractionStateMatrix`、state/transition/action/permission matrix。
- 错误、取消、恢复、上下文保留、焦点交接与可验证 acceptance scenarios。
- 输出是设计草案，不包含 `accepted/completed/approved/verdict`。

## 规则、禁止与权限 (Rules & boundaries)

- 一个页面只有一个最高优先级主任务；主要、次要、低频和危险 action 分层。
- action availability 由业务状态、权限、数据条件和系统状态共同决定；前端隐藏控件不是授权。
- 禁只画 happy path、用 boolean 拼互斥状态、保存失败清空输入、批量失败只报一个通用错误。
- 禁自行修改业务规则、伪造用户研究、把 Profile 默认值冒充需求或外部标准。
- 本 Skill 只写设计/审查产物；不获得产品代码、外部设计工具、生产或发布权限。

## 自动化与验收 (Automation & acceptance)

- 若项目已有 flow/state/schema validator、prototype test 或路由测试，可运行并保存真实命令、版本和结果；不存在则标
  `not_executed`，不得声称 ForgeOS 已提供对应 detector。
- 验收检查：主任务闭合；entry/exit/back 可达；替代/错误/取消/恢复完整；action guard 和权限来源明确；无死路和互斥状态；
  每个关键 transition 可映射到测试场景。
- 该验收只形成 Review 输入，不能替代 `forge accept`。

## 直接参考 (References)

- `docs/design/ai-engineering-os/frontend-design-standard.md#3-编码前决策顺序`
- `docs/design/ai-engineering-os/frontend-design-standard.md#4-ui-decision-contract`
- `docs/design/ai-engineering-os/frontend-design-standard.md#5-场景-profile-与页面-pattern`
- `docs/design/ai-engineering-os/frontend-design-standard.md#6-flowstateaction-与-permission`
- W3C, [SCXML 1.0](https://www.w3.org/TR/scxml/)（仅在项目选择该交换格式时读取）
