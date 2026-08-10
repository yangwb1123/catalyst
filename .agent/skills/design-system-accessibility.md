# Skill: design-system-accessibility

> 将场景 Profile 映射为受治理的 token、组件和可访问性合同；区分标准事实、平台契约与视觉启发式。

## 职责与触发 (Responsibility & triggers)

用于新 UI pattern/component/token/theme、场景 Profile、视觉验收、国际化/RTL、键盘/焦点、对比、缩放或辅助技术变化。
它唯一拥有 visual-design、design-system 和 accessibility；token/profile、组件状态、a11y 和 visual review 是条件化 lens。
业务 flow/IA 由 `information-interaction-design` 提供，框架实现与性能由 `frontend-client-engineering` 负责。
涉及页面构图时调用 supporting `ui-geometry`，本 Skill 仍拥有 token、shape language、光学判断、a11y 和 visual review。

## 输入契约 (Inputs)

- 已确认的主任务、IA、flow、state/action/permission、目标平台与风险。
- 项目批准的 token、组件库、品牌、locale/RTL 支持范围、无障碍目标和既有视觉基线。
- 外部标准必须带版本和平台；组织规则必须带 owner/作用域；缺 token 或组件事实时不得臆造为“现有设计系统”。
- 只读取当前 canonical Profile、Pattern 和平台相关参考；不相关规则不得同时装载。

## 执行 SOP (Procedure)

1. 将规则标为 `standard/platform_contract/organization_policy/profile_heuristic/advisory`，解决冲突时按权威与适用范围处理。
2. 优先采用带 source claim 的项目显式 Profile；没有项目绑定时，才从
   `cms_editorial/oa_workflow/erp_mes_dense/crm_relationship/analytics_decision/commerce_transaction/mobile_task/`
   `marketing_conversion/immersive_story/data_wall/ai_agent_workspace/generic_saas` catalog routing heuristic 中选择候选，并说明密度、
   视觉层级和动效为何服务主任务。
3. 将颜色、间距、排版、圆角、阴影、尺寸、动效和响应 context 映射到批准 token；Profile 数值不得冒充跨平台标准。
4. 复用批准组件并定义适用的 default/hover/focus/disabled/loading/empty/error/success/offline/partial states；不为不适用页面制造假状态。
5. 为 Web 或 native 平台定义语义、名称、键盘/焦点、辅助技术、对比、缩放、target 和 status announcement 义务。
6. 检查颜色非唯一表达、长文本、locale、RTL、高对比与 reduced-motion context；高风险 flow 增加人工/辅助技术复核。
7. 形成 token/component delta、accessibility matrix、visual acceptance 和 evidence plan，交实现 Skill。
8. 审查 composition 中 axis/group/negative-space/stroke/shape/optical 引用是否来自项目 token/policy/Profile，并用截图判断
   视觉重心、阅读动线和光学关系；DOM 矩形不得替代该判断。

## 输出契约 (Outputs)

- `FrontendDesignPackage.classification.profile_id/density/motion_level`：每项使用完整 `classified_value`，并给出替代项、来源层和理由；例如
  `profile_id: { value: erp_mes_dense, claim_type: inference, confidence: 0.90, proof_claim_id: "", assumption_id: assumption-profile }`。
- `profile_overrides`：无偏离时为 `[]`；偏离固定 Profile 的 `density` 或 `motion_level` 时逐字段记录
  `default/selected/reason/scope/risk/compensating_proof_claim_ids/reviewer_id/expires_at`。
- token context/alias/delta、component reuse/delta、Interaction Visual State Matrix。
- Accessibility/i18n/RTL matrix：标准或平台条款、适用 flow、验证方式、人工义务。
- Visual acceptance、baseline requirements、advisory findings 和未验证项。
- 对 `business_ui_composition` 的 token/shape/optical/visual Review；几何总分保持 advisory。
- 不输出视觉总分式完成判定，也不修改业务状态或权限。

## 规则、禁止与权限 (Rules & boundaries)

- 8pt/4pt 网格、14px、44px、390/1024/1440、固定动效时长均是可能的 Profile/组织值，不是跨平台不变量。
- `profile_overrides` 只能覆盖 `density/motion_level` 启发式，必须精确覆盖偏离并由独立 reviewer 与补偿证据约束；不得借此覆盖标准、权限或业务规则。
- Web target 按目标 WCAG 条款，Flutter/RN 按各自平台语义；不能混用 CSS px 和 logical pixel。
- APG 是非规范性 pattern guidance，DTCG 是 Community Group Report；只有项目明确采用后才形成项目合同。
- 禁只用颜色表达状态、用 ARIA 修补错误语义、为视觉一致性牺牲缩放/RTL/键盘或重复造组件。
- 本 Skill 无权写生产 UI、修改全局 token source、批准设计系统变更或宣称无障碍合规。

## 自动化与验收 (Automation & acceptance)

- 仅在项目实际配置时运行 token validator、contrast/axe、keyboard/focus、screenshot/golden 或视觉回归；保存环境、版本、source
  和结果。自动工具不能覆盖全部无障碍或审美判断，缺失项标 `not_executed`。
- 验收要求：每个数值有 token/profile 来源；token 引用可解析；批准组件优先；语义/键盘/焦点/对比/缩放/RTL/reduced-motion
  义务可追踪；pixel diff 与 Vision finding 不冒充业务正确性。
- 结果只供独立 Review 和现有完成流程消费。

## 直接参考 (References)

- `docs/design/ai-engineering-os/frontend-design-standard.md#2-五层规则权威`
- `docs/design/ai-engineering-os/frontend-design-standard.md#8-信息架构与-design-system-治理`
- `docs/design/ai-engineering-os/frontend-design-standard.md#9-accessibilityresponsive-与-motion`
- `.agent/skills/ui-geometry.md`
- W3C, [WCAG 2.2](https://www.w3.org/TR/WCAG22/) 与 [ARIA APG](https://www.w3.org/WAI/ARIA/apg/)
- W3C DTCG, [Format Module 2025.10](https://www.w3.org/community/reports/design-tokens/CG-FINAL-format-20251028/)
