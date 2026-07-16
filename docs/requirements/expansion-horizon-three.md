# ForgeOS: 第三地平线扩展方向分析

> **视角**: 资深架构师/产品经理,基于全局代码库扫描(forge-core 18 Go 包 · harness 39 模块 ·
> 全部 `.agent/{WORKFLOWS,AGENTS,SKILLS,ARCHITECTURE}` 声明 · 31 个 Sprint 完整演进记录 ·
> 80+ 篇已有分析文档交叉验证)。
>
> **核心判断**:前两波分析已聚焦于「引擎落地」(Sprint 5-15)和「边缘可靠性」(Sprint 20-31)。
> 本文瞄准的是**第三地平线**:系统从「单项目自治」进化到「多项目生态」过程中必然出现的结构性缺口。
> 这些方向在当前代码库中没有对应的实现、没有声明、甚至没有设计文档。
>
> **纪律**:不写代码。每条方向附代码级证据,说明为什么没有被已有 40+ 篇分析覆盖。
>
> **日期**:2026-07-09

---

## 当前态势:两个地平线已覆盖

| 地平线 | 核心主题 | 已做 | 代表性分析 |
|--------|----------|------|------------|
| **H1:引擎就绪** | 五引擎落地、中枢旋钮、真点火坐实 | Sprint 5-31 全覆盖 | `high-value-expansion-directions.md`(方向 1-5) |
| **H2:生产就绪** | Prompt QA、契约执法、检查点灾难恢复、静默监督 | 边缘场景分析 | `expansion-production-readiness.md`(方向 1-5) |
| **H3:生态扩展** ⬅ 本文 | 多工作流组合、多仓库联邦、事件驱动、资产升级、跨会话学习 | **零实现、零设计** | — |

**H3 的特征**:这些方向不修现有功能的 bug,不优化现有路径的性能,而是扩展 ForgeOS 的适用范围——
从「一个项目的一个自治循环」到「一群项目的自治生态」。

---

## 方向一:管线工作流组合 —— 从单段脊柱到全自动 Pipeline

### 为什么需要

ForgeOS 的脊柱是 `Discover → Design → REVIEW → Build → Evolve`。这五个阶段各自有独立的工作流文件：

```
.agent/workflows/discover.yml   # P1-P3:需求发现→调研→PRD
.agent/workflows/design.yml     # P1-P2:架构→方案→★Human Approval★
.agent/workflows/review.yml     # P1-P4:安全→分布式→性能→CTO
.agent/workflows/build.yml      # P1-P5:规划→实现→闸门→评审→QA
.agent/workflows/evolve.yml     # P1-P3:扫描→差距→路线图更新→(循环)
```

**每个工作流目前必须手动触发**。虽有 `stop_condition.on_met.next_stage` 声明(见 `design.yml:69`、`review.yml:138-141`、`discover.yml:83`),但这个字段纯声明性——没有任何运行时机制去读 `next_stage` 并自动加载下一个工作流。

### 当前状态

```
# 搜索 next_stage 的消费方
grep -rn "next_stage\|NextStage" forge-core/ --include="*.go"
# → cmd/forge/evolve.go: reportHumanGate 打印它
# → 但没有"自动加载下一个工作流"的逻辑

# asset 结构体: OnApproved 声明了 NextStage 字段
grep -rn "NextStage\|OnApproved" forge-core/internal/asset/ --include="*.go"
# → asset.go:194 OnApproved struct { NextStage string }
# → 有 schema、有声明、零消费
```

用户必须手动运行 `forge run discover` → 读输出 → `forge run design` → 等批准 → `forge run build` → `forge run review` → `forge run evolve`。这不是「自治工厂」,是「手工装配线」。

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| discover 收敛但 confidence=75% | 不应自动进 design | 无自动 pipeline,手动控制,不会误闯 |
| human_gate 卡住(design 等批准) | pipeline 停在 human 边界 | 当前手动,不存在此问题;自动后需 wait/skip 策略 |
| review REJECTED | pipeline 应回退到 design 而非继续 build | 无自动机制 |
| evolve 循环永不收敛 | pipeline 无限循环 | loop 有 max-iter 安全网,但上级 pipeline 无超时 |
| 部分工作流跳过(mode=explorer 跳过 discover) | pipeline 应自动跳过整个阶段而非报错 | 模式已支持单个工作流内 skip,但 pipeline 组合时需传播 skip 语义 |

### 价值

1. **完成「Idea→Production」愿景**:当前用户手动走完 5 个阶段,不是「自动工厂」。一个 `forge pipeline` 命令就能闭合
2. **消除人为遗忘**:用户可能忘记跑 review 就直接 deploy 了——自动 pipeline 按声明强制执行
3. **回退自动化**:review REJECTED 自动回到 design 而非让用户手动重跑

### 与已有分析的区别

已有分析(`expansion-core-five-2026-07-01.md §5`「跨工作流管道编排」)的焦点是将**同一个**工作流的 phase 做条件串联。本文方向一的焦点是**不同工作流文件**之间的状态机编排——`discover.yml → design.yml → review.yml → build.yml → evolve.yml` 作为一个可执行的有向图,这是迄今无人触及的缺口。

---

## 方向二:多仓库联邦治理 —— 从单仓到多仓生态

### 为什么需要

ForgeOS 当前为**一个仓库**提供完整治理:

```
forge-init projectA/   → 继承 .agent/ + harness → forge run/evolve
forge-init projectB/   → 继承 .agent/ + harness → forge run/evolve
```

但真实产品跨多个仓库:

```
frontend/       (React/TS)      — 独立 deploy
backend/        (Go)            — 独立 deploy
shared-lib/     (Python)        — 被前两者依赖
infra/          (Terraform)     — 管理所有基础设施
```

**每个仓库各自独立 evolve——没有跨仓库的视角**:

```
# 没有跨仓库的场景:
# 1. backend/ 的 API 变更 → frontend/ 需要同步更新 —— 无依赖跟踪
# 2. 多个仓库共享一个 budget 池 —— 无统一成本管理
# 3. 组织级的路线图 —— 每个仓库只看自己的 ROADMAP.md
# 4. 统一策略继承 —— frontend/ 和 backend/ 各自有独立的 .agent/policies/
```

### 当前状态

```
# 检查是否有任何跨仓库的概念
grep -rn "federation\|multi-repo\|cross.repo\|monorepo\|workspace" .agent/ --include="*.md"
# → 零 (架构中没有任何多仓库概念)

# asset.Workflow 没有任何 "depends_on_repo" 或 "workspace" 字段
grep -rn "type Workflow\|type Project" forge-core/internal/asset/
# → Workflow 仅有 phases/stop_condition/stage 字段,无多仓库概念

# ROADMAP.md 是项目级,不是组织级
# .agent/project.yml 是单个项目的配置
```

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| 共享库 breaking change | 下游仓库未更新 → 构建失败 | 完全无感知 |
| 跨仓库 API 契约演进 | backend 改 API → frontend 不知 | 无依赖图 |
| 多个仓库同时 evolve 烧预算 | 总成本失控(每个仓库各自有 budget cap) | 无聚合预算 |
| 安全策略统一升级 | 需要在 N 个仓库分别改 .agent/policies/ | 无策略继承 |
| 跨仓库 issue 跟踪 | backend evolve 发现需要 frontend 改 | 无跨仓库任务注入 |

### 价值

1. **真实世界适配**:大多数组织不是单仓。没有联邦治理,ForgeOS 的适用范围被严重限制
2. **依赖感知编排**:API 变更自动触发依赖仓库的 PR/CI,而非等到集成测试失败
3. **统一策略管控**:安全基线、模型路由策略、budget cap 从组织级下推,而非每个仓库各自配置

### 与已有分析的区别

已有分析没有任何文档讨论多仓库场景。所有分析(40+篇)都隐含假设「一个仓库 = 一个 ForgeOS 项目」。方向二是 ForgeOS 从「单项目工具」进化为「组织级平台」的必经之路。

---

## 方向三:外部事件驱动触发器 —— 从拉取到推送

### 为什么需要

ForgeOS 当前完全是**拉取(pull)模式**:用户执行命令,系统响应。

```
# 所有入口点:
forge run <workflow>        # 用户手动触发
forge evolve <workflow>     # 用户手动触发
forge accept                # 用户手动触发
```

但在自治工厂愿景中,系统需要**推送(push)模式**——响应外部事件:

| 事件 | 期望响应 | 当前状态 |
|------|----------|----------|
| GitHub PR opened | 自动跑 `forge run review` 或 `forge run build --incremental` | 不存在 |
| CI pipeline 失败 | 自动创建 gap issue 并入 evolve 循环 | 不存在 |
| 安全 advisory 发布 | 自动触发 `forge run scan` + 创建修复任务 | SCA 框架已就绪,但无自动触发 |
| 定时(每日/每周) | 自动 `forge run evolve` 做持续演化 | 不存在 |
| 部署完成 | 自动跑 `forge run design` 做 post-deploy review | 不存在 |

`cmdEvolve` 需要在终端前台运行。要让它在后台响应事件,当前需要:
```
nohup forge evolve .agent/workflows/evolve.yml --mode balanced &
```

——没有守护进程管理、没有自动重启、没有事件分发。

### 当前状态

```
# 搜索任何 webhook/server/listener 的痕迹
grep -rn "http\.Listen\|webhook\|github.*hook\|cron\|schedule\|trigger\|daemon" forge-core/ --include="*.go"
# → 零

# forge-core 根本不 import net/http
grep -rn "\"net/http\"" forge-core/ --include="*.go"
# → 零

# 唯一的"事件处理"是 SIGINT/SIGTERM 上下文取消
grep -rn "signal\|SIGINT\|SIGTERM" forge-core/cmd/forge/ --include="*.go"
# → evolve.go:25 "os/signal", evolve.go:218 signal.Notify(ch, syscall.SIGINT, syscall.SIGTERM)
# → 仅用于优雅关闭,不是事件循环
```

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| 多个事件同时到达 | 并发运行 vs 排队串行 | 无调度 |
| webhook 重放攻击 | 重复处理同个 event | 无幂等性 |
| 事件处理超时 | webhook caller 超时断开 | 无异步确认(202 Accepted) |
| 认证缺失 | 任何人都能触发工作流 | 无认证层 |
| 定时任务重叠 | 上一个 evolve 还在跑时下一个触发 | 无锁/跳过机制 |

### 价值

1. **真正「自治」**:自动响应外部事件,而非等人敲命令
2. **CI/CD 集成**:PR 自动 review、失败自动分析、安全更新自动修复——不再需要 Jenkins/GitHub Actions 写大量胶水
3. **定时演化**:每日/每周自动做 gap 分析 + 路线图更新,而不是等人想起才跑

### 与已有分析的区别

已有分析(`expansion-core-five-2026-07-01.md §3`「跨厂商模型池」、`expansion-production-readiness.md §5`「静默监督」)讨论的是 agent 执行层面的自动化和观测,不涉及「什么触发执行」。方向三是触发层(trigger layer)的完全缺失,属于预先分析从未覆盖的系统边界。

---

## 方向四:治理资产生命周期管理 —— `forge upgrade-governance`

### 为什么需要

ForgeOS 自身在演进——新的 agent 卡、新的 workflow phase、新的 policy 检查、新的 harness 工具。
但被 `forge-init` 治理的项目**永远停留在初始化那一刻的版本**:

```
# forge-init 创建项目时的治理快照
# projectA/.agent/agents/         ← 2026-05 版本 (9 agent 卡,缺 cto.md review 阶段)
# projectB/.agent/policies/       ← 2026-06 版本 (缺 mode_gating 漂移守卫)
# projectC/harness/               ← 2026-04 版本 (缺 sca.mjs、scorecard-telemetry)
```

当 forge-core 新增检查(`check_mode_priorities`、`check_workflow_mode_gating`)、新增 agent 契约(`cto.md` 的五择一裁决)、新增 harness 工具(`sca.mjs`、`scorecard-update.mjs`)时,**已有项目毫不知情**。

```
# check.py 能检测到治理不完整:
# → "workflow references agent 'cto' but no agent card found"
# 但没有任何机制来修复它——既不能自动升级,也不能列出差异
```

### 当前状态

```
# forge-init 有一个 COPIED_FILES 清单(forge-init 自行维护)
grep -rn "COPIED_FILES\|copiedFiles\|copyList" harness/scaffold/
# → forge-init.mjs 中有完整清单

# 但没有任何"diff against current forge version"的命令
grep -rn "upgrade\|update.*governance\|governance.*version\|diff.*governance\|sync.*governance" forge-core/ harness/
# → 零

# .agent/project.yml 没有任何 forge-core 版本字段
cat .agent/project.yml
# → lifecycle、mode、overrides……没有 forge_version
```

没有版本记录,就无法检测 drift。无法检测 drift,就无法升级。

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| 项目自定义了 agent 卡(扩展而非覆盖) | 升级时可能被覆盖 | 无升级机制,也无合并策略(merge/overwrite/skip) |
| harness 工具接口变化(yaml2json python→Go) | 旧项目仍用 python shim,新特性用不了 | 无兼容性标记 |
| workflow 文件被项目自定义修改 | 升级可能破坏自定义内容 | 无 diff 三路合并 |
| 项目 A 不想升级(稳定性优先) | 应该可以选择性升级 | 无版本锁定机制 |
| 多个项目需要批量升级 | 逐个手动处理 | 无批量操作 |

### 价值

1. **治理可持续性**:没有升级路径的治理系统终将腐化——项目越老,治理越落后
2. **安全补丁**:新的 secret 扫描规则、新的安全策略需要能推送到所有项目
3. **功能采纳**:新 agent 角色、新 workflow 阶段、新 harness 检查——如果无法推送,就等于不存在

### 与已有分析的区别

已有分析(`expansion-production-readiness.md §2`「接口契约强制」)关注的是 **运行时** Go/Python/Node 之间的版本兼容,而非**治理资产的跨项目升级**。这是两个不同的维度——运行时版本兼容是纵向的(同一项目内的不同层),治理资产升级是横向的(同一层跨不同项目)。

---

## 方向五:跨会话修正学习 —— 从单向记忆到可纠正的智能

### 为什么需要

ForgeOS 的 Memory Engine 是累计的追加日志——session 2 能看到 session 1 的记录,但**不能纠正**:

```
# 场景:session 1 做了一个错误决策
# session 1: "use PostgreSQL" — 写入 memory (kind=decision, topic=database)
# session 2: 发现 PostgreSQL 不适合,改为 SQLite
# session 3: 查询 memory → 仍看到 "use PostgreSQL"——没有"deprecated"或"superseded"语义
```

`memory.Entry` 有 `Supersedes` 字段(支持显式取代),但没有任何机制让**人类或后续 session** 标记一条旧知识为「已过时/已纠正」。更关键的是:

1. **没有人类修正反馈回路**:当 `forge approve reject`(当前 CLI 已有 `reject` 占位但未实现)时,拒绝的理由不会被注入 memory 作为学习信号
2. **没有路由决策的负反馈**:如果 user 每次都手动把 `forge route` 选的 Opus 降级为 Sonnet(因为任务没那么复杂),这个偏好从未被记住——下次同样的 agent+mode 仍是 Opus
3. **没有收敛失败的模式学习**:如果 build.yml 的 loop-back 总是死在 implementer→harness-gates 之间(agent 每次都写坏测试),Memory 中没有任何记录来标记这个模式,以便下次自动调整 prompt 或切换模型

### 当前状态

```
# memory.Entry 结构体
grep -rn "type Entry struct" forge-core/internal/memory/
# → memory.go: type Entry struct { Format, Kind, Topic, Detail, Confidence, Supersedes, ... }
# Supersedes 字段已存在,但从未在主路径中使用

# 搜索 Supersedes 的实际消费
grep -rn "Supersedes" forge-core/ --include="*.go"
# → memory.go: 定义 + decode/encode
# → memory.go: filterSupersedes — 只在 Load 时过滤已被 supersede 的条目
# → 没有任何地方写入 Supersedes

# 搜索 human 修正反馈
grep -rn "reject\|override\|correct\|feedback\|learn" cmd/forge/approve.go
# → 只有 "cmdApprove" 和 "cmdApproveList",没有 reject/override 命令

# cost.go: parseConfidenceScore 已经能读 agent 的自信心
# 但没有任何路径把它和人类的"纠正"关联起来
```

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| Agent 自信但错了(confidence=95,但决策是错的) | 系统重复错误决策 | 无矫正机制 |
| Human 拒绝了一个架构方案 | 下次 evolve 可能再次提出类似方案 | 拒绝被遗忘 |
| 某个 agent 角色(如 implementer)特定模型效果差 | 路由总是选同样效果差的模式 | 路由负反馈不存在 |
| 安全策略变更(之前 allowed,现在 blocked) | 旧的 memory 条目告诉 agent 「这个操作以前可以」 | 无策略变更感知 |
| 多次 loop-back 同一 phase | 系统可能陷入重复错误模式而不自知 | 统计存在(trace),但模式未被识别为学习信号 |

### 价值

1. **避免重复错误**:当前系统每天犯同样的错误——没有「记住上次什么不行」的机制
2. **人类监督产生持久价值**:每次人工干预(approve/reject/override)都应成为系统的训练数据
3. **路线图质量**:如果路线图频繁被 reject,系统应该自适应调整 planner 的输出风格/粒度和范围

### 与已有分析的区别

已有分析中有「学习闭环」(`high-value-expansion-directions.md` 的方向一、`strategic-extensions-v22.md` 的方向四等),但它们关注的是 **agent 自身的执行数据**(成功/失败/延迟/成本)怎么回灌路由。方向五关注的是**人类修正信号**(override/reject/correct)怎么进入跨会话记忆——这是两种完全不同类型的学习信号。前者是自动的数据飞轮,后者是需要显式设计的人机交互学习回路。这个区别在所有已有分析中都没有被识别。

---

## 汇总:五方向的价值评估

| # | 方向 | 核心价值 | 当前成熟度 | 实现量级 | 与已有分析的重叠度 |
|---|---|---|---|---|---|
| 1 | **管线工作流组合** | 完成 Idea→Production 自动流水线 | 声明存在(`next_stage`),运行时零消费 | 中(编排层新增 Pipeline 引擎) | 低(`expansion-core-five §5` 同名单 stage 内串联,非跨文件编排) |
| 2 | **多仓库联邦治理** | 生态扩展——从单项目到组织级平台 | 零——架构、代码、文档均无此概念 | 大(新子系统:Federation Service) | 无——40+ 篇分析没有一篇讨论跨仓库 |
| 3 | **外部事件驱动** | 从拉取到推送——真正的自治触发 | 零——`net/http` 都不 import | 中(webhook listener + 事件队列) | 无——所有分析都假设用户手动触发 |
| 4 | **治理资产升级管线** | 治理系统的自我可持续发展 | `forge-init` 有单向复制,无反向升级 | 中(`forge upgrade-governance` + version pinning) | 低(`production-readiness §2` 关注运行时版本兼容,非资产升级) |
| 5 | **跨会话修正学习** | 人类干预产生持久价值——避免重复错误 | `Supersedes` 字段存在但零消费 | 中(修正注入 + 路由负反馈) | 低——已有学习闭环分析未区分自动数据飞轮 vs 人类修正信号 |

### 推荐优先级

1. **方向一(管线组合)**——最接近完成,`next_stage` 已声明、asset schema 已就绪,只需接线。ROI 最高
2. **方向四(资产升级)**——紧迫性高:每加一个新检查/新 agent,已经 fork 的项目就落后一步。不及时解决会累积大量治理债务
3. **方向五(修正学习)**——区分 ForgeOS 和「哑巴编排工具」的关键特性。没有学习,24h 自治 vs 24h 重复犯相同错误
4. **方向三(事件驱动)**——真正自治的前提。但对外依赖较重(GitHub webhook、cron 语法),可以先从定时触发开始
5. **方向二(多仓库联邦)**——价值最高但规模最大,需等方向一运转稳定、治理资产升级机制成熟后再投入

---

## 附录:被排除(或有意识推迟)的方向

| 方向 | 排除原因 |
|------|----------|
| **Web UI** | 已确定为 v3,与 CLI 优先的产品定位冲突 |
| **Firecracker 沙箱** | 外部阻断(v3+KVM/特权),框架已就绪但需触发条件 |
| **LiteLLM 跨厂商池** | 外部阻断(v3+多厂商 key),框架已就绪但需触发条件 |
| **agent-os 子仓库** | ADR 0003 设计就绪但用户决策 pending |
| **Temporal durable_wait** | v2 目标,当前 `.forge/*.approved` 够用 |
| **Embedding 语义检索** | 镀金,当前 TF-IDF 对现有 corpus 够用 |
| **Prompt Snapshot/Golden File** | 已在 `production-readiness.md` 中覆盖,本文不重复 |
| **契约履约率监控** | 已在 `production-readiness.md` 中覆盖,本文不重复 |
| **OPA/Rego 策略引擎** | north-star 的 Policy/Gov 服务,v3 目标 |
| **运行时版本兼容性** | 已在 `production-readiness.md` 中覆盖,本文不重复 |
| **多维模型路由自动化** | 已在 `high-value-expansion-directions.md` §1 覆盖 |
| **并行调度引擎** | 已在 `high-value-expansion-directions.md` §2 覆盖 |
| **内存规模演进** | 已在 `high-value-expansion-directions.md` §3 覆盖 |
| **多进程并发安全** | 已在 `high-value-expansion-directions.md` §4 覆盖 |
| **运行诊断/根因分析** | 已在 `high-value-expansion-directions.md` §5 覆盖 |
