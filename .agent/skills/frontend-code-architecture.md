# Skill: frontend-code-architecture

> 让同一业务变化原因的代码聚集，让不同业务只经最小稳定契约协作，并以可执行边界和证据阻止结构腐化。

## 职责与触发 (Responsibility & triggers)

用于 React/TSX、Vue、Flutter/Dart、React Native 或其他 client 的 route/page/widget/feature/entity/shared
边界、依赖方向、公开入口、状态/数据/effect 所有权、目录与 API 预算、God File 风险和渐进重构。

在以下任一情况激活：新增或移动 route/feature/module；跨 feature import；新增 shared store/query/cache/API client；
公共 hook/component contract 变化；出现循环/反向依赖、God Page、垃圾桶目录；普通变更跨越三个以上 client 边界。
纯文案、叶子样式或不改变依赖与职责的局部修复可记录 N/A。

本 Skill 是独立的条件化治理入口，但不创建第二套 Capability ownership：

- client 实现仍由 `frontend-client-engineering` 主拥有；
- 系统级边界和低可逆决策升级 `architecture-tradeoff`；
- fresh-context 裁决归 `code-review`；
- God Object 的行为保持式拆分复用 `god-object-refactoring`；
- IA/业务 flow/state 与视觉/token/a11y 分别归现有前端设计 Skill。

## 输入契约 (Inputs)

- 需求、验收、非目标、计划和预计 change surface。
- 现有目录树、import graph、模块 README/ADR、public entry、测试与构建命令。
- 项目声明的 architecture profile；没有声明时只能提出候选，不能把目录名启发式冒充架构事实。
- server/URL/form/workflow/cache/local/view state、数据和 effect 所有权。
- 当前复杂度、历史变更耦合、运行风险与例外记录。

## 执行 SOP (Procedure)

1. **重建事实**：读取现有结构、相邻实现、ADR 与真实 import；区分当前合同、历史遗留和推断。
2. **确定变化轴**：用一句话说明能力、每个文件和模块为何存在；列出独立变化原因。
3. **选择最小 profile**：复用既有结构；只有明确采用时才使用 feature-sliced、Flutter clean feature 或项目自定义矩阵。
4. **画边界**：输出 module → responsibility → owner → public API，并标出 internal surface。
5. **分配状态与 effect**：server、URL、form、workflow、cache、local view 各归能完整拥有生命周期的最小边界；IO 经 adapter。
6. **检查依赖**：方向、同层跨 slice、深层 import、barrel、cycle、global store/event bus/service locator 绕界逐项检查。
7. **评估复杂度**：先看多信号 God Risk 和变更放大，再看行数/Hook/handler/API/export/目录预算；阈值只是触发器。
8. **设计最小改动**：优先守卫语句、小纯函数、显式 policy/mapper、判别联合或必要状态机；不为降行数机械切碎。
9. **渐进迁移**：公共契约或高变更半径使用 adapter/facade、兼容窗口、逐 slice 迁移和删除路径。
10. **验证**：运行 architecture、cycle、complexity、type/lint/test/build；fresh Reviewer 依据代码与证据裁决，不采信实现者自述。

## 自动硬约束 (Automatic hard constraints)

仅在项目显式声明或被无歧义识别的 architecture profile 上自动阻断：

- 循环依赖；
- 声明依赖矩阵中的反向边；
- 声明禁止的同层跨 slice 依赖；
- 跨模块绕过 public entry 的深层 import；
- 无效、过期或缺少 owner/reason/evidence 的边界例外。

自动检查只能证明可解析 import/path 事实，不能证明语义内聚、业务流程数量或设计质量。未激活 profile 时必须报告
`not_configured`，不得把未检查写成通过。

## 审查启发式 (Review heuristics)

以下信号用于触发审查，不单独证明违规：

- 文件或组件无法用一句具体职责描述；
- UI、API、业务 policy、mapping、validation、routing、permission、upload/export 中出现三个以上独立责任；
- 多个不相关 flow/dialog、五类以上状态、频繁跨区修改；
- 过长文件/函数、过深嵌套、过多 Hook/handler/API/export 或同级文件；
- `common/utils/helpers/manager` 等目录承载未命名语义；
- 一个简单变化需同时改 page、global store、shared util、API、router、permission 和 config；
- 模块公开项过多、star/recursive barrel 泄露 internal surface。

审查必须区分观测信号与语义结论。评分达到阈值只生成 review finding；只有多个高置信信号和独立证据满足项目
block policy 时才阻断。

## 模块与公共 API 规则

- 默认按业务能力/变化原因组织，不按 components/services/hooks/types 横向堆积；不得强制既有项目迁移到单一目录流派。
- route/page 负责 composition、路由与页面错误边界；feature 负责一项用户能力；entity 表达稳定业务概念；shared 无业务语义。
- 跨模块只依赖公开入口及必要输入输出类型；internal 可以自由重构，不能通过 deep import、全局 Store 或事件总线偷渡依赖。
- 进入 shared 需要至少两个真实跨业务使用方、稳定无业务语义 API 和更低总维护成本；“以后可能复用”不是证据。
- public API 预算是审查触发器；超过预算需检查职责过宽或实现泄露，不得为满足数字制造更多 facade。
- 状态放在最小完整生命周期 owner；可派生值不得重复存储，服务端数据不得伪装成永久 client truth。

## God File 与反过度架构

God Risk 必须使用多信号而非单行数。静态代理可统计 lines、functions、Hook/composable、handler、remote-call、state、export、
dependency 等；业务流程数、责任数和 change amplification 必须由 Reviewer 用 diff/history/契约证据判断。

拆分依据是独立变化原因，不是把一个 400 行内聚实现切成十个透传文件。禁止为单一局部实现预建 interface/factory/plugin/event bus，
也禁止用 `any`、lint disable、global singleton 或复杂类型技巧掩盖边界问题。

## 例外合同 (Exceptions)

任何可豁免边界 finding 必须包含：`id/ruleId/findingFingerprint/source/target/reason/riskOwner/approver/`
`compensatingProofs/debtLink/createdAt/expiresAt/removalTrigger/maxOccurrences`。例外范围必须精确且最长 90 天；过期、通配整个仓库、
无证据或自批例外失败关闭。依赖方向、所有权、循环与配置完整性不可豁免；偏好差异不构成例外理由。

## 输出契约 (Outputs)

- Architecture profile 与事实/假设来源。
- `module → responsibility → owner → public/internal API` 地图和依赖图。
- state/data/effect ownership 表、计划/实际 change surface 与兼容迁移方案。
- 自动 hard findings、审查 heuristic findings、God Risk 的逐信号证据及有效例外。
- architecture/type/lint/test/build 的真实回执、未执行项和 residual risk；不输出自签完成裁决。

## 完成条件 (Completion criteria)

- 激活 profile 的硬边界、循环和 public-entry 检查通过；例外有效且可追踪。
- 变更主要集中在正确业务边界，新增公共 API 最小，状态/effect 有唯一 owner。
- 没有用 shared/global bus/deep import 或规则关闭绕界，也没有为数字进行机械拆分。
- 高风险 heuristic finding 已关闭或由独立 Reviewer 记录时限、owner 和后续动作。
- 受影响行为有测试保护，命令证据与当前 source revision 绑定；最终完成仍由 `forge accept` 决定。

## 直接参考 (References)

- `.agent/engineering/frontend-code-architecture.yml`
- `.arch/frontend-architecture.v1.json`
- `docs/design/ai-engineering-os/frontend-code-architecture-standard.md`
- `.agent/skills/frontend-client-engineering.md`
- `.agent/skills/architecture-tradeoff.md`
- `.agent/skills/code-review.md`
- `.agent/skills/god-object-refactoring.md`
