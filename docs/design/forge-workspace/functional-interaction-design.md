# Forge Workspace 功能与交互设计

> 状态：**Proposed / 产品设计，非当前 UI 声明**
> 日期：2026-08-21
> 依赖：[目标架构](architecture-plan.md) · [总体实施计划](implementation-plan.md)

## 1. 体验目标

用户进入产品后应持续得到四个答案：

1. 系统正在完成什么 Outcome；
2. 当前为什么处于这个状态；
3. Agent 实际执行了什么、修改了什么；
4. 用户现在是否需要做决定。

产品不是聊天窗口的放大版。对话是输入方式之一，主信息结构必须围绕 Space、Objective、Change、WorkItem、Attempt、Action 和 Evidence。

## 2. 信息架构

```text
Space
├── Overview
├── Projects
├── Objectives / Changes
│   └── Change Cockpit
│       ├── Goal & Acceptance
│       ├── Impact & Unknowns
│       ├── WorkGraph
│       ├── Run Timeline
│       ├── Artifacts & Evidence
│       └── Decisions & Approvals
├── Knowledge Graph
├── Evolution Queue
└── Policy & Budget
```

全局导航只呈现用户对象。协议、digest、raw envelope 和低层 graph handshake 放在 Developer/Debug 视图。

## 3. 全局状态语言

### 3.1 状态

| 状态 | 用户语义 | 主动作 |
|---|---|---|
| Draft | 目标尚未形成可执行计划 | 补充目标、生成计划 |
| Planning | 正在分析范围、影响和未知项 | 查看分析、暂停 |
| Awaiting approval | 已有明确决策需要用户确认 | 审批、拒绝、修改 |
| Ready | 所有前置满足，等待调度 | 开始、调整自治级别 |
| Running | Agent 正在执行 | 查看 Timeline、暂停 |
| Verifying | 正在独立验证结果 | 查看 checks |
| Blocked | 缺输入、依赖或策略不允许 | 解决阻塞、replan |
| Failed | 已确认失败，无未裁决 effect | 重试、replan |
| Uncertain | 可能发生 effect，但结果不确定 | 人工裁决，不自动重试 |
| Completed | 当前 Snapshot 上验收满足 | 查看 Outcome、创建后续工作 |

颜色不是唯一编码方式；所有状态同时使用文本、图标和原因。`Uncertain` 与 `Failed` 必须视觉和操作上明显不同。

### 3.2 证据等级

| Badge | 含义 |
|---|---|
| Observed | 来自确定性工具、Runtime 或 Harness 的有界观察 |
| Inferred | Agent 推断，附 evidence/confidence/expiry |
| Declared | 用户、配置或外部系统声明，尚未独立认证 |
| Unknown | 未观察、无法解析或 freshness 不足 |
| Verified | 指定检查对指定 Snapshot/Artifact 给出有效 Receipt |

`Verified` 不等于整个 Change 完成；界面必须显示它验证了什么、没验证什么。

## 4. 首次使用流程

### 4.1 创建 Space

步骤：

1. 用户选择“新建 Space”；
2. 输入名称和本地数据目录；
3. 选择默认自治等级、预算和 Agent；
4. 系统展示本地存储、网络和凭证边界；
5. 创建后进入空 Space Overview。

失败状态：目录不可写、已有 owner、schema 不兼容、磁盘空间不足。不得静默切换到其他目录。

### 4.2 注册项目

步骤：

1. 选择本地 Git 项目；
2. 系统只读预检路径、仓库状态、语言和构建入口；
3. 显示将观察的范围、排除项和 Unknown；
4. 用户确认后捕获 ProjectSnapshot；
5. 后台生成初始项目关系和健康摘要。

项目卡片显示：路径别名、HEAD/dirty 状态、snapshot 时间、主要语言、graph coverage、开放 Change、阻塞和成本。不把目录可读误写成项目完整或无 secret。

## 5. Objective 创建与规划

### 5.1 输入面板

用户填写：

- 想实现的 Outcome；
- 目标项目或“由系统分析”；
- 必须满足和明确不做的内容；
- 可选参考文件/Issue/Artifact；
- 风险、时间、预算和自治级别。

系统返回可编辑的 Planning Brief：目标重述、验收标准、受影响项目、Unknown、初步风险和建议下一步。用户确认的是结构化目标，不是同意 Agent 任意执行。

### 5.2 规划结果

Change Cockpit 的规划视图包含：

- Objective 与 AcceptanceCriteria；
- 当前 Snapshot 和 freshness；
- 直接/传递影响与 evidence path；
- Unknown 和需要用户回答的问题；
- WorkGraph、关键路径和可并行 wave；
- 每个 WorkItem 的 Agent、权限、预算、验证和预计风险；
- 明确的“不在范围内”。

审批卡必须展示计划 digest、授权 effect、写路径、网络、预算、有效期、验证要求和计划变化。只显示“Approve?”属于不合格交互。

## 6. Change Cockpit

### 6.1 Header

固定展示：Change 名称、状态、当前 WorkItem、ProjectSnapshot、预算消耗、自治等级、最后事件时间、Pause/Resume/Cancel。

当 Snapshot 漂移、事件流中断或 Runtime 不可用时，Header 出现阻断 banner，不允许仅在日志中提示。

### 6.2 WorkGraph

节点显示：名称、owner、状态、风险、依赖、Attempt 次数、验证摘要。用户可切换 DAG、列表和关键路径视图。

交互：

- 点击节点进入 WorkItem detail；
- 查看 ready 原因或 blocked 原因；
- 查看 predecessor Artifact/Receipt；
- 对 Draft/Blocked 节点请求 replan；
- 不允许用户拖动节点后绕过依赖；结构修改必须形成新 graph version。

### 6.3 Decision Queue

把所有需要人工处理的事项集中展示：

- Change approval；
- Tool/network/write approval；
- 风险接受；
- Snapshot drift/replan；
- uncertain effect adjudication；
- Evolution promotion。

每个 Decision 显示请求者、作用范围、前因、风险、替代方案、失效时间和批准后的下一步。

## 7. Run Timeline

### 7.1 分层视图

默认层级：

```text
Attempt
  Session
    Turn
      Action
        Output chunks / Artifact / Receipt
```

Timeline 默认显示 durable semantic event；用户可打开“实时输出”查看 live chunks。断线后界面显示 replay 状态和 cursor，不把缺失时间段伪装成无事件。

### 7.2 Action 卡片

每个 Action 至少显示：

- 类型和状态；
- Agent/Tool actor；
- 请求与批准范围；
- 开始/结束/耗时；
- 文件、命令或网络目标；
- 输出摘要与 Artifact 链接；
- cost/token；
- causation 和相关 WorkItem；
- 失败、取消或 uncertain 原因。

模型的计划和决策只展示宿主公开输出或明确生成的摘要；界面不得暗示能访问隐藏思维链。

### 7.3 文件变化

提供按项目、文件、WorkItem 的 diff 视图：

- 写入前后 Snapshot/Artifact；
- Agent 和 Action 来源；
- 未跟踪文件、删除、权限变化和大文件提示；
- 当前 diff 与已验证 diff 的区别；
- 验证后继续修改时立即标记 Receipt stale。

## 8. 审批交互

### 8.1 审批类型

- Plan approval：允许进入执行；
- Effect approval：允许特定写、命令、网络或工具；
- Risk acceptance：接受已声明残余风险；
- Recovery decision：裁决 uncertain effect；
- Evolution promotion：允许新策略生效。

### 8.2 审批动作

用户可以：Approve once、Approve for scoped duration、Reject、Request changes。默认不提供无限期全局允许。

批准后任何目标、参数、Snapshot、digest、预算或 scope 变化都使旧审批失效。UI 必须明确显示失效原因。

## 9. Verification 与 Outcome

Verification 页面按 check 分类显示：命令、环境、Snapshot、开始/结束、原始 Artifact、结果和 N/A/Not executed 原因。

Outcome 页面回答：

- 哪些 AcceptanceCriteria 已满足；
- 由哪些 Artifact/Receipt 支持；
- 修改了哪些项目和接口；
- 剩余 Unknown、风险、Debt 和后续建议；
- 总耗时、模型/工具成本和人工干预；
- 如何复验或回滚。

“Completed”按钮不由用户或 Agent直接设置；它是 Control Plane 对当前证据的 join 结果。用户可接受残余风险，但必须形成单独记录。

## 10. 跨项目 Knowledge Graph

### 10.1 Space Overview

展示项目关系、活动 Change、阻塞传播、graph coverage 和 freshness。默认优先显示与当前 Change 相关的子图，避免完整图变成视觉噪声。

### 10.2 Impact Explorer

用户选择项目、文件、API、数据库或 Change 作为 seed。结果分层：direct、transitive、unknown boundary，并展示最短 evidence path。

过滤项：edge type、project、snapshot、observed/inferred、confidence、freshness、owner。推断边默认不能驱动硬审批或自动扩大写权限。

## 11. Evolution Queue

每个提案显示：观察问题、样本、基线、假设、建议改变的版本化资产、目标指标、风险、shadow 结果和回滚。

操作：Run shadow evaluation、Compare、Approve bounded experiment、Reject、Promote、Rollback。禁止“一键让系统自我优化”这种无边界入口。

## 12. CLI 设计

### 12.1 稳定产品命令

```text
forge space create|list|show
forge project add|list|snapshot|show
forge objective create|show
forge change plan|show|start|pause|resume|cancel
forge run list|show|timeline|interrupt
forge decision list|show|approve|reject
forge graph impact|show
forge evolution list|show|evaluate|promote|rollback
forge artifact show|verify
```

默认输出面向人，`--json` 输出版本化 read model。长命令返回 operation/change ID；`--follow` 订阅同一事件流。

### 12.2 内部命令

现有 `graph-node-*`、schedule/dispatch/receipt 等进入：

```text
forge protocol ...
forge debug ...
```

兼容期保持旧别名并输出 deprecation，不在普通 help 首屏展示。

## 13. TUI 设计

布局：左侧 Space/Change 导航，中间 WorkGraph 或 Timeline，右侧 Context/Decision detail，底部快捷键和连接状态。

核心快捷键：`g` Space、`c` Change、`r` Run、`d` Decisions、`/` 搜索、`f` 过滤、`p` Pause、`?` 帮助。Approve/Reject 必须进入确认面板，不能单键立即生效。

窄终端降级为单栏导航；颜色关闭后仍可理解状态；持续输出支持暂停滚动、定位未读和回到实时位置。

## 14. App 路由建议

```text
/spaces
/spaces/:spaceId
/spaces/:spaceId/projects
/objectives/:objectiveId
/changes/:changeId
/changes/:changeId/graph
/attempts/:attemptId/timeline
/graph/impact
/evolution
/decisions
/settings/policy
```

URL 只包含逻辑 ID，不暴露本地绝对路径或数据库 row id。刷新页面后由 Query API 重建视图。

## 15. 空、错、慢和断线状态

- Empty：解释为何为空并提供下一主动作；
- Loading：区分首次加载、事件追赶和 Runtime 等待；
- Error：显示稳定错误码、影响范围、可重试性和诊断 ID；
- Offline：保留最后 durable cursor，禁止提交看似成功的写命令；
- Partial：显示哪些项目/事件/图谱缺失；
- Stale：显示 Snapshot/Receipt/Approval 失效的具体变化；
- Slow：长时阶段显示当前 action、heartbeat 和预算，而不是伪百分比。

## 16. 功能验收场景

1. 用户注册两个项目，能区分 observed、inferred 和 unknown 关系；
2. 用户创建 Objective，修改并批准明确的 AcceptanceCriteria/WorkGraph；
3. Agent 请求写文件时，用户看到路径、diff、风险和审批 scope；
4. Timeline 断线重连后无 durable event 丢失或重复显示；
5. Runtime 在命令后崩溃，界面进入 uncertain 而不是自动重试；
6. Harness 失败后 Change 保持 verifying/blocked，不被 Agent success 覆盖；
7. 验证后文件变化，Outcome 立即标记 stale；
8. 用户在 CLI、TUI、App 看到相同状态、原因和 Receipt；
9. Evolution 提案只能做 shadow evaluation，未经审批不能生效；
10. Agent Host 未公开内部信息时，界面诚实显示不可观测边界。
