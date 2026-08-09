# Skill: frontend-client-engineering

> 把已审查的 UI 决策合同映射为 Web 或 native client 实现，并用真实行为、交互、响应式和性能证据验证。

## 职责与触发 (Responsibility & triggers)

用于 React/TSX、Vue、Flutter、React Native 或其他 client 的页面、组件、状态、API adapter、响应式、动效、交互验证和性能变化。
它唯一拥有 frontend-engineering/client-engineering；framework mapping、responsive/motion/performance 和 interaction validation
是按目标栈激活的 lens。缺 IA/flow/state 合同时先调用 `information-interaction-design`；缺 token/a11y 合同时先调用
`design-system-accessibility`。涉及 route/feature/shared/public client contract、跨模块 import、God Page 或结构迁移时，必须先调用
`frontend-code-architecture`；它是独立治理入口，但不改变本 Skill 对 client implementation 的 primary ownership。

## 输入契约 (Inputs)

- 需求/验收、schema-shaped `FrontendDesignPackage.classification`、IA、TaskFlow、state/action/permission matrix 和 proof obligations；
  分类中的 `profile_id/page_pattern/platform` 必须是 canonical ID，不接受 legacy alias 或裸字符串。
- 项目 stack/version、现有组件/token、API/事件契约、目录约定、测试命令、浏览器/设备支持矩阵和性能预算。
- module/public-entry 与 state/data/effect ownership、API/error/cache mapping、锁文件/工具链/runtime config 和 release/rollback 事实。
- source/context 与权限范围；未确认的 API、组件或框架能力不得虚构。高风险权限/状态未知时停止实现。
- 按目标框架只读取 AFDS 相应 adapter 段和官方资料，不同时装载 React、Vue、Flutter 与 RN 细节。

## 执行 SOP (Procedure)

1. 搜索现有 feature、component、token、API adapter、状态和测试模式，确定最小 change surface。
2. 将 `web_desktop/web_responsive/ios/android/cross_platform` 平台 intent 映射到原生语义、导航、focus/input 和生命周期；
   React/Vue/Flutter/React Native 是 adapter/stack，不是 platform ID，服务端数据/权限保持权威。
3. 建显式 client state：区分 idle/loading/success/empty/error/offline/partial/stale/conflict/cancelled 中实际适用状态，避免矛盾 boolean。
4. React 保持 render 纯、Effect 只同步外部系统；Vue 使用语义模板和受控响应性；Flutter/RN 使用各自 Semantics/accessibility、
   constraints、focus 和列表组件，不用 Web 假设覆盖 native。
5. 实现响应式/locale/RTL/reduced-motion 和失败后数据保留；异步/批量 action 防重复并展示部分结果和恢复路径。
6. 按测量处理 bundle、render、network、memory、list virtualization 和动画；不机械 memo/lazy/virtualize。
7. 运行项目已有静态、类型、组件、E2E、a11y、screenshot/golden 和性能检查；审查 diff 并记录未执行义务。
8. API/Event DTO、error code、permission、pagination、cache invalidation、retry/cancel/idempotency 经显式 adapter/matrix 映射；
   不能让后端字段、HTTP client 或服务端授权责任散落在组件。
9. 构建/发布相关变更记录 runtime、package manager、lockfile、compiler/bundler、环境配置、产物 digest、Feature Flag、缓存兼容、
   telemetry、灰度、回滚和清理 owner；前端环境变量一律视为公开信息。

## 输出契约 (Outputs)

- 授权范围内的 feature components、state/API adapters、styles/token usage 和必要测试。
- changed-file/contract manifest、真实命令与环境回执、截图/golden/trace locator、性能结果和未执行项。
- requirement/flow/transition → test/evidence trace、残余风险、兼容和回滚说明。
- 保留 schema-shaped 分类，例如
  `platform: { value: web_responsive, claim_type: fact, confidence: 1.0, proof_claim_id: claim-platform, assumption_id: "" }`，
  不把 `tsx/vue/dart/rn` 写入 AFDS platform 字段。
- 输出不包含自签 `accepted/completed/approved/verdict`。

## 规则、禁止与权限 (Rules & boundaries)

- 禁把前端权限判断当最终授权、API 直接散落在展示组件、业务计算藏在 render/effect、随机/不稳定 list key、保存失败丢输入。
- 禁非语义交互元素缺键盘/名称/焦点、硬编码未批准视觉值、无界并发/列表、无测量性能优化和低风险任务全局重写。
- 禁每次请求新建 client、无 timeout/cancel 的请求、非幂等自动重试、任意全局 cache invalidation、敏感数据进入 URL/storage/log；
  CSS cascade/scoping/layer、SSR/hydration、browser support、weak-network/realtime/multi-tab 仅在触发时装载相应 lens 并验证。
- inline style、具体断点、memo 或 virtualization 是否禁止由项目 policy/证据决定，不冒充框架官方规则。
- 只能修改任务授权路径；安装依赖、调用外部服务、真机、生产和发布需要相应权限。

## 自动化与验收 (Automation & acceptance)

- 从项目事实发现并运行适用 lint/typecheck/unit/component/E2E/build；Playwright/native golden/axe/AT/performance 仅在真实配置和环境可用时运行。
- 视觉基线记录 browser/runtime、OS/device、font、DPR/scale、viewport、locale、theme、reduced-motion、seed 与 source digest；
  环境不一致不得伪装为稳定 diff。
- 验收要求：主/替代/错误/恢复 flow 与适用状态可执行，permission/冲突安全，语义和输入方式可用，预算有证据；
  未运行项明确，Vision score 或自动 scan 不抵消 hard finding。最终裁决仍归 `forge accept`。

## 直接参考 (References)

- `.agent/skills/frontend-code-architecture.md`
- `docs/design/ai-engineering-os/frontend-code-architecture-standard.md`
- `docs/design/ai-engineering-os/frontend-design-standard.md#9-平台与框架映射`
- `docs/design/ai-engineering-os/frontend-design-standard.md#10-evidence-pipeline`
- React, [Components and Hooks must be pure](https://react.dev/reference/rules/components-and-hooks-must-be-pure)
- Vue, [Accessibility](https://vuejs.org/guide/best-practices/accessibility.html) 与 [Performance](https://vuejs.org/guide/best-practices/performance)
- Flutter, [Adaptive and responsive design](https://docs.flutter.dev/ui/adaptive-responsive/general) 与 [Testing overview](https://docs.flutter.dev/testing/overview)
- React Native, [Accessibility](https://reactnative.dev/docs/accessibility)；Playwright, [Best Practices](https://playwright.dev/docs/best-practices)
