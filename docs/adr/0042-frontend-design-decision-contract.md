# ADR-0042 — 前端设计决策合同与跨平台适配边界

- 状态：已接受（2026-08）
- 范围：业务目标到 Web/Flutter/React Native UI 实现前的设计、交互、可访问性和证据合同；当前为 shadow
- 关联：ADR-0037（能力中心化组织）、ADR-0040（机器可读 Agent Engineering）、ADR-0041（后端决策合同）、
  [AFDS](../design/ai-engineering-os/frontend-design-standard.md)

## 背景

现有 AI Engineering OS 已定义 UX/UI 节点、前端 God Component 约束和通用 Skill 包装原则，但 Coding Agent 仍可能从一句
“生成管理页面”直接进入 TSX、Vue 或 Dart：页面结构照抄数据库，按钮只依赖一个状态字段，缺少取消/失败/恢复流程，
或把 8pt、14px、44px、固定断点和动效时长误写成跨平台标准。

另一方面，把 CMS、ERP、营销页、沉浸式页面和原生移动端塞进同一套固定模板，也会用视觉一致性掩盖任务、密度、输入、
权限和性能差异。截图相似、LLM 视觉高分或自动 axe 结果均不能单独证明用户任务已完成。

因此需要一个平台无关但不抹平平台语义的前端决策合同，并需要明确外部标准、平台契约、组织策略和可调启发式的权威边界。

## 决策

### 1. 建立 AFDS 决策顺序，不建立第二套完成系统

前端任务按“目标/Actor → 信息架构 → flow/state/action/permission → Profile × Pattern → Design System → 平台适配 →
实现 → 多源证据”收敛。设计产物不得包含 `accepted/completed/approved/verdict`；`forge accept` 仍是唯一完成裁决权威。

本 ADR 交付 byte-pinned policy、Profile/Pattern catalog、FrontendDesignPackage schema、三张 canonical Skill adapter、
Context route、shadow detector、对抗 validator 与 scaffold/legacy-upgrade 继承。它没有新增自动 diff compiler、可信截图服务、
Vision grader、Token compiler 或 pre-code runtime authority，也不把 shadow 报告接入 `forge accept`。

### 2. 规则按五层权威表达

规则分为 `standard`、`platform_contract`、`organization_policy`、`profile_heuristic` 和 `advisory`。每条规则应记录版本、
作用域、触发、适用条件、例外、验证与 owner。

WCAG Success Criterion 可作为 Web 标准要求；项目明确采用的 APG pattern、Flutter Semantics 和 React Native accessibility
props 属平台契约；
组件白名单与 token 来源属组织策略；网格、字号、密度、断点和动效时长通常属 Profile；视觉平衡与未校准模型评分属建议。

因此拒绝把以下内容写成跨平台不变量：统一 8pt 网格、最小 14px 字号、所有按钮 44×44、固定 390/1024/1440 断点、
统一动效时长、所有页面机械生成全部异步状态。

### 3. Profile 与 Pattern 正交组合

Scenario Profile 只使用 catalog 的 canonical ID：`cms_editorial`、`oa_workflow`、`erp_mes_dense`、`crm_relationship`、
`analytics_decision`、`commerce_transaction`、`mobile_task`、`marketing_conversion`、`immersive_story`、`data_wall`、
`ai_agent_workspace`、`generic_saas`。Page Pattern 只使用 `list`、`detail`、`form`、`workbench`、`wizard`、`editor`、
`approval`、`dashboard`、`landing`、`immersive`、`data_wall`、`agent_chat`、`visual_editor`、`timeline`。两者通过项目
Design System、领域状态和目标平台组合，任一方都不能决定业务规则或权限。

分类字段不是裸字符串。每个值必须使用 v1 `classified_value`，区分 proof-backed fact 与 assumption-backed inference；例如以下
片段（完整记录仍须包含 schema 要求的十二个分类字段和 `rationale`）：

```yaml
classification:
  profile_id: { value: erp_mes_dense, claim_type: inference, confidence: 0.90, proof_claim_id: "", assumption_id: assumption-profile }
  page_pattern: { value: workbench, claim_type: inference, confidence: 0.86, proof_claim_id: "", assumption_id: assumption-pattern }
  platform: { value: web_desktop, claim_type: fact, confidence: 1.0, proof_claim_id: claim-platform, assumption_id: "" }
```

`profile_overrides` 是必需顶层列表：当 `density` 与 `motion_level` 等于固定 Profile 默认值时必须为空；偏离时必须逐字段记录
固定默认值、选择值、理由、作用域、风险、补偿 proof claim、与 package review 一致的独立 reviewer 及失效日期。它最多覆盖
`density/motion_level` 两个启发式字段，不能覆盖标准、业务规则、权限或其他分类；checker 只校验记录与交叉引用，不认证理由或身份。

### 4. Flow、State、Action 与 Permission 是实现前合同

关键 flow 必须覆盖入口、前置条件、主线、替代、错误、恢复、取消、返回、出口和上下文保留。复杂业务使用显式 transition；
action availability 由业务状态、权限、数据条件和系统状态共同决定。前端只负责呈现和快速反馈，服务端仍是授权与业务状态权威。

异步、批量、并发和不可逆操作必须定义部分成功、未知结果、数据保留、冲突与恢复。确认弹窗不是唯一答案；高影响操作可按
风险使用撤销、提交前复核、原因、审批或明确确认。

### 5. 平台无关合同保留平台特性

核心合同保存 intent、语义状态、action 和 proof obligation。v1 platform ID 仅为 `web_desktop`、`web_responsive`、`ios`、
`android`、`cross_platform`；React、Vue、Flutter、React Native 是实现 adapter/stack，不是 platform ID。Web adapter 负责
HTML/WCAG/APG，React/Vue adapter 负责框架 state/effect 与语义，Flutter adapter 负责 constraints/Semantics/focus/golden，
React Native adapter 负责 iOS/Android 辅助技术、输入和列表性能。Web 的 CSS px、Flutter logical pixel 与原生 hit area 不能混成一个数字。

### 6. Design Token 采用可验证格式但不神化格式

项目若采用 DTCG 2025.10，应验证类型、引用、循环、token/group 歧义、deprecated token 和各 resolver context。DTCG 是
Community Group Final Report，不是 W3C Recommendation；semantic token tiers、raw-value ban 和 spacing scale 仍由组织治理。

### 7. 多源证据替代单一视觉分数

适用项目按真实工具组合静态/类型、组件、E2E、自动无障碍、键盘/焦点/辅助技术、确定性 screenshot/golden、token/geometry、
性能与独立人工 Review。视觉基线绑定环境和代码摘要；像素 diff 只证明变化，axe 只发现部分问题，Vision score 只提供建议。
未运行的步骤必须标 `not_executed`，不能伪造 PASS。

### 8. 沿用三项 canonical ownership 形成渐进加载入口

adapter 严格沿用 `capability-skill-map.v1.yml` 的唯一 ownership：`information-interaction-design` 条件化承载分类、flow、IA 和
state/action/permission lens；`design-system-accessibility` 承载 token、Profile、a11y 和 visual review lens；
`frontend-client-engineering` 承载 framework mapping、responsive/motion/performance 和 interaction validation lens。
三张卡使用现有项目工具，不声称不存在的 runtime 或 detector，也不建立第二套 capability ownership。

## 权限与诚实边界

- Skill 只提供决策与审查方法，不自动获得代码、浏览器、设备、Figma、生产或发布权限；
- APG 是非规范性指导，DTCG 是 Community Group Report，组织采用后才成为项目合同；
- screenshot、golden、axe、Web Vitals 与 Vision review 都必须绑定真实运行环境和 source；
- verification case 与其 exact-subject claim 的 artifact 集合必须完全一致；Profile override 风险参与有效风险 floor；
- 当前 machine checker 只执法合同、路由、引用、声明结构和有界本地 artifact 字节绑定。v1 对 verification case 中声明的
  source/build digest、fixture 和 environment 只做形状、交叉引用与包内一致性检查；对 screenshot 额外验证 PNG chunk/CRC、
  未知 critical chunk、`PLTE` 约束、`IDAT` 连续性、受限 zlib 像素流、scanline filter 与 viewport×DPR 尺寸。它不会重建
  source/build/fixture、认证浏览器或设备环境，也不能证明
  artifact 由声明工具产生；
- 因此结构有效不能声称业务语义、真实工具执行、capture provenance 或 Reviewer 身份已认证；
- 后续载重实现仍需要可信 Runner、append-only ledger、签名 receipt、自动影响识别和 runtime authorization，且不能形成第二完成权威。

## 后果

**正面。** 页面生成从“风格 Prompt”转为可审计的业务与交互决策；不同产品风格可以复用相同 Pattern；跨平台共享语义但保留
原生差异；固定数值回到 token/profile；行为、无障碍、视觉和性能证据不再互相冒充。

**成本。** 中高风险前端变更增加 flow/state/permission 和多环境验证成本。当前 package 和 detector 仍是 shadow，Context route
仍是声明而非运行时选择器，也没有视觉环境固定器或可信 provenance；完整自动化必须另行实现和验证。

## 被拒方案

1. 一个“企业级前端大师”巨型 Prompt：无法按场景渐进装载，也不能证明业务链路与平台语义；
2. 为每种颜色、圆角或技术栈建 Skill：触发爆炸且混淆 Profile 与能力；
3. 所有平台共享同一像素和断点规则：忽略 CSS px、native logical pixel、输入与辅助技术差异；
4. 截图或 LLM 视觉评分达到 90 即完成：无法证明交互、权限、数据保护和可访问性；
5. 立即把 shadow checker 升为载重 runtime gate：当前缺可信 provenance、自动影响识别和授权内核，会把声明绑定伪装成真实执行证明。

## 一手资料

- W3C, [Web Content Accessibility Guidelines (WCAG) 2.2](https://www.w3.org/TR/WCAG22/)
- W3C WAI, [ARIA Authoring Practices Guide](https://www.w3.org/WAI/ARIA/apg/)
- W3C DTCG, [Design Tokens Format Module 2025.10](https://www.w3.org/community/reports/design-tokens/CG-FINAL-format-20251028/)
- React, [Components and Hooks must be pure](https://react.dev/reference/rules/components-and-hooks-must-be-pure)
- Vue, [Accessibility](https://vuejs.org/guide/best-practices/accessibility.html)
- Flutter, [Accessibility](https://docs.flutter.dev/ui/accessibility)
- React Native, [Accessibility](https://reactnative.dev/docs/accessibility)
- Playwright, [Visual comparisons](https://playwright.dev/docs/test-snapshots) and
  [Accessibility testing](https://playwright.dev/docs/accessibility-testing)
- web.dev, [Web Vitals](https://web.dev/articles/vitals) and
  [prefers-reduced-motion](https://web.dev/articles/prefers-reduced-motion)
