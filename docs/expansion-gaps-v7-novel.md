# ForgeOS — 第七轮扩展方向分析

> 全局扫描当前代码库后，从资深架构师/产品经理角度提出的 **5 个高价值扩展方向**。
> 不写代码，只做诊断。每个方向均不同于前六轮分析已覆盖的内容。

---

## 目录

1. [跨项目知识联邦：从单仓记忆到组织学习](#1-跨项目知识联邦从单仓记忆到组织学习)
2. [运行时模型质量自适应：从离线记分卡到在线调优](#2-运行时模型质量自适应从离线记分卡到在线调优)
3. [Workflow 集成测试框架：将编排本身纳入测试](#3-workflow-集成测试框架将编排本身纳入测试)
4. [多租户安全隔离与并发控制](#4-多租户安全隔离与并发控制)
5. [渐进式告警升级：从二进制中止到分级人机协作](#5-渐进式告警升级从二进制中止到分级人机协作)

---

## 1. 跨项目知识联邦：从单仓记忆到组织学习

### 现状

`internal/memory` 提供了完备的跨会话知识存储（gap / decision / lesson 三种条目），
每次 evolve 迭代写入 `.forge/memory.jsonl`，下次迭代通过 `memoryContext` 注入到
agent prompt。但**这一切都局限在一个仓库的一个 `.forge/` 目录下**。

当组织有多个仓库（微服务生态、多模块 monorepo 拆分为独立仓），每个仓库的知识是
完全隔离的：

- 仓库 A 的架构决策（ADR）不会在仓库 B 的 review 中出现
- 仓库 A 的 `lesson`（"该支付库 mock 框架有坑，注意 X"）对仓库 B 的 implementer 不可见
- 跨仓的全局安全规则（"所有 auth 改动必须走 security review"）需要手动同步

### 为什么需要

ForgeOS 的愿景是 "AI-native 软件工厂"。真正工厂级的治理不能是每个车间独立记笔记。
跨项目知识联邦有以下具体价值：

| 场景 | 当前状态 | 联邦后 |
|------|---------|--------|
| 安全最佳实践传播 | 每个 repo 独立发现 | 一次 lesson，全局继承 |
| 通用架构模式 | 重复 ADR | 一次决策，自动关联检索 |
| 模型质量画像 | 单仓 scorecard 偏置 | 全局聚合，冷启动更准 |
| 迁移治理升级 | 逐个仓手动跑 migrate | 一次策略变更，批量生效 |

### 边界与实现考量

- **隐私/隔离**：联邦 ≠ 全局共享。每个 repo 可以声明 `allow_federation: true/false`，
  敏感项目（支付核心）可 opt-out。
- **冲突解决**：当两个 repo 的同一 topic lesson 矛盾时，以时间戳或优先级裁决。
- **性能**：跨项目检索需要全局索引（Lucene-like 或 embedding），不能每次 N+1 读文件。
- **增量丰富**：一次 lesson 被多个项目验证后，confidence 自动提升（当前 memory 的
  `Confidence` 字段已预备但仅靠 caller 设置，无自动演化机制）。

### 当前代码中的就绪点

- `internal/memory` 的 Entry 结构已包含 `Topic` / `Confidence` / `Supersedes` —
  语义上完全适合联邦去重与合并
- `prompt.Retrieve` 的 BM25-lite 检索器可直接扩展到跨项目文档集
- `internal/prompt/cache.go` 的 ContextCache 可作为联邦缓存入口

---

## 2. 运行时模型质量自适应：从离线记分卡到在线调优

### 现状

`scorecard_wind.go` 在每次 `forge run` / `forge evolve` 结束时将真实成本+延迟写入
`scorecards.json`。`routing.HistoryTiebreak` 在下一次 route 时读出，做历史择优。
但这是**批次离线**的：

- 当前 run 内，即使观察到 reviewer 的 rework 率突然升高（opus 不如预期），也不会
  在当前 run 的中后期调整模型分配
- `BudgetAdjustTier` 只响应**成本消耗比**，不响应**质量退化**
- 没有异常检测：一次预算耗尽导致 down-tier 后，无法自动回升

### 为什么需要

真点火（Sprint 24-26）暴露了一个关键事实：**模型质量不是静态的**。同一个模型在不同
prompt 构造、不同任务类型、不同阶段的表现有显著波动。Sprint 25 的 implementer 在
`acceptEdits + 无 Bash` 组合下表现与 reviewer 的纯判断任务完全不同。

当前架构缺少一个**运行时品质面板**：

| 信号 | 离线（现有） | 在线（缺失） |
|------|------------|------------|
| rework 率 | scorecard avg_iterations | 当前 run 内 reviewer 通过率 |
| 成本异常 | avg_cost_usd | 某 phase 成本偏离预算曲线 |
| 延迟异常 | p95_latency_ms | 当前 phase 超时趋势 |
| 质量退化 | 无（仅 reviewer 单次判断） | 连续多个 REQUEST_CHANGES |

### 边界与实现考量

- **反馈延迟**：在线信号必须足够快才能在同一个 run 内生效。reviewer 的一次
  REQUEST_CHANGES 大约需要 1-2 个 phase 周期（~30s），比 scorecard 风干快 N 个数量级。
- **过度反应**：一次偶发 rework 不应触发降档。需要一个质量滑动窗口（最近 3 次 phase
  检查），窗口内的趋势 > 阈值才动作。
- **副作用**：在线降档可能影响最终产出质量。需要一个 "超驰" 机制——如果预算允许，
  最终阶段的 reviewer 可临时跳回原档以作最终质量把关。
- **与现有护栏重叠**：`BudgetAdjustTier` 已经是成本驱动的在线调整；质量驱动的调整
  是正交维度，两者应取更严。

### 当前代码中的就绪点

- `Engine.OnGateResult` 和 `Engine.AgentVerdict` 回调已经是实时信号通道
- `Engine.BudgetExhausted` 的 puller 模式可直接扩展为 `Engine.QualityDegraded`
- `trace.Event` 已携带每个 agent phase 的 `DurationMs` + `CostUsdMicros`，
  实时计算滑动窗口只需从 trace 流式读取

---

## 3. Workflow 集成测试框架：将编排本身纳入测试

### 现状

ForgeOS 的测试覆盖很扎实：

- **Go 包测试**：每个 `internal/*` 包都有单测（~40 个 test 文件）
- **Harness 测试**：`harness/test_*.mjs`（~28 个自测）
- **App 测试**：`examples/` 下的真实应用测试（39 个）

但**没有任何一个测试断言**：

> "当 workflow `build.yml` 在 mode=engineering 下运行时，
> 预期的 gate 集合是 [lint, test, build, complexity, arch, security]，
> reviewer phase 必须运行，discover stage 必须 skip。"

换句话说，编排逻辑本身的正确性——mode 矩阵 × gate-set × phase 路由 × stop condition
——只靠手动 `forge run` 验证，没有自动化。

### 为什么需要

这个缺口是战略性的。随着中枢旋钮（`mode.Policy`）的维度增加到 7 个（gates / reviewer /
evolve / discover / design / adr / enforce），手工验证每个组合的成本指数增长。

| 测试类别 | 示例 | 当前覆盖率 |
|---------|------|-----------|
| 组件测试 | `mode.Effective("explorer","mvp").Gates == [lint,build]` | 已有（但分散） |
| **编排测试** | `engine.Run(build.yml, "explorer")` 跳过 discover、gate 只跑 lint+build | **零** |
| 集成测试 | `forge run build --mode explorer --executor dry` 输出包含 "discover skipped" | **零** |
| 组合矩阵 | 4 mode × 4 lifecycle = 16 组合，验证 gate-set/reviewer/ discover 排列 | **零** |

### 边界与实现考量

- **可运行的工作流 fixture**：需要一组最小工作流定义（非真实 YAML，而是
  `asset.Workflow` Go 字面量）覆盖所有 stop type / phase 配置变体。
- **断言面**：
  - 哪些 phase 被执行/被跳过（mode gating 正确性）
  - 每个 gate phase 调用了哪些 gate（`gate-set ∩ required_gates`）
  - loop 在什么条件下收敛/中止
  - loop-back 跳转到正确的 target_phase
  - human_gate 在无 approval 时永不收敛
- **不需要真 agent**：`DryRunExecutor` 本身就是为测试设计的——它只叙述、不执行。
  编排测试正好利用这一点：验证哪些 phase 被 narrate 了，哪些被跳过了。
- **并行模式覆盖**：`RunParallel` 在 dependency wave 下的 fail-fast、cancellation
  传播、lock-order 无死锁——这些只能用编排级测试暴露，单测覆盖不到。

### 当前代码中的就绪点

- `asset.LoadWorkflowJSON` 可用 Go 字面量直接构造 Workflow，无需 YAML 文件
- `Engine.RunGate` 可在测试中注入假 gate（`ok` / `fail` / `NA` 可控）
- `DryRunExecutor.Log` 可捕获 phase 叙述作为断言依据
- `Engine.OnGateResult` / `Engine.AgentVerdict` 均可注入测试 spy

---

## 4. 多租户安全隔离与并发控制

### 现状

`forge-core` 的所有持久状态被写入 `.forge/` 目录：

- `checkpoint.json`（检查点/恢复）
- `trace.jsonl`（可观测事件流）
- `memory.jsonl`（跨会话记忆）

这些文件**没有进程间隔离**。如果两个 `forge` 进程同时在同一个 repo 上运行（例如
一个手动 `forge run` 与一个后台 `forge evolve` 冲突），会导致：

- trace 交错损坏（`O_APPEND` 写入的 JSONL 会被交叉）
- checkpoint 被覆盖（`os.Rename` 原子但一对一的最后写入者赢）
- memory 交叉污染

### 为什么需要

这个缺口在当前 "一人一个 repo" 的使用模式下不触发，但只要走向多用户或多进程就会
成为安全漏洞。

| 场景 | 风险 |
|------|------|
| CI runner 与开发者的 `forge run` 同时执行 | trace 流损坏，scorecard 数据错乱 |
| 两个 evolve loop 同时探索不同方向 | checkpoint 覆盖，一个 loop 被视为另一个的 resume |
| 多用户共享 dev server | memory 中混入另一个用户的 gap/lesson |
| 后台 `forge evolve` 与前台 `forge run --parallel` | checkpoint 读取 half-written 状态 |

### 边界与实现考量

- **轻量锁**：不需要分布式锁。单个文件系统上的 `flock`（BSD locks）最适合——
  获取锁成功则运行，失败则 `WARN` 退出（"another forge process is active"）。
- **会话 ID**：每个 `forge run/evolve` 实例生成一个 UUID，写入 `.forge/.session`
  文件，包含 PID、start time、workflow name。退出时清理。异常退出时（crash），
  锁文件残留可用 stale-detection（PID not alive）自动回收。
- **命名空间 trace**：trace 行可以追加 `session_id` 字段，即使多个进程交错写入
  同一个文件，后续分析工具也能按 session_id 分离。
- **用户级目录隔离**：在共享环境（SSH server）中，`.forge/<username>/` 作为可选
  命名空间前缀。

### 当前代码中的就绪点

- `persist.Save` 的原子写（temp + rename）是单进程安全的；多进程保护需要在
  `forgeDir` 层级加锁
- `internal/memory` 的 `Append` 使用 `O_APPEND` + 单行写入。即使多进程同时
  append，内核保证每行不交错——但 trace 的 `Emit` 使用 `t.mu.Lock()` 只在
  进程内有效，多进程需要文件级锁
- `internal/trace.Tracer` 的 `mu sync.Mutex` 明确标注了并行设计的双重目的：
  "future phases run in parallel" + "concurrent writes would corrupt"

---

## 5. 渐进式告警升级：从二进制中止到分级人机协作

### 现状

当前错误处理的哲学本质上是**二进制的**：

| 失败模式 | 结果 | 是否可恢复 |
|---------|------|-----------|
| gate FAILED (无 on_fail) | abort（fail-closed） | 否 |
| gate FAILED (有 on_fail, 预算耗尽) | abort（fail-closed） | 否 |
| loop-back 预算耗尽 | abort（fail-closed） | 否 |
| 无进展 tripwire | 中止（fail-open） | 否 |
| max-iter 安全上限 | 中止（clean stop） | 否 |
| 预算耗尽 | 停止（clean stop） | 否 |
| agent 503 overload | retry（可配置次数） | 是 |
| agent timeout | retry 或 abort | 取决于配置 |

关键问题是：**所有不可恢复的失败都是等价的**。一个 "opening payment gateway"
的 gate FAIL 与一个 "linter warning in unused module" 的 gate FAIL 产生相同
的 abort 行为——没有分级、没有降级模式、没有 "呼叫人类" 的中间状态。

### 为什么需要

Sprint 6 的 Human-Approval 闸门建立了最高杠杆的人机协作点（design → build）。
但运行时的**操作性失败**（非设计决策，而是执行失败）缺少类似的升级阶梯：

| 失败严重度 | 当前行为 | 期望行为 |
|-----------|---------|---------|
| 轻微（lint 警告） | abort run | 继续运行，报告警告 |
| 中等（gate FAIL 但在 on_fail 循环内） | 继续重试 | 若 N 次后仍红，**标记需人工审视但不中止** |
| 严重（critical gate FAIL / budget 耗尽） | abort | 当前正确 |
| 未知（agent 输出无法解析） | 视为 REQUEST_CHANGES 继续 | **暂停并请求人类裁决** |
| 致命（panic / OOM / 磁盘满） | 进程崩溃 | watchdog 检测并通知人类 |

### 边界与实现考量

- **升级阶梯定义**：需要一个声明式的 escalation policy（YAML），为每种 gate/phase
  定义其 "严重度层级" 及对应行为（忽略 → 记录 → 降级模式 → 呼叫人类 → 紧急中止）。
- **退化模式**：某些 gate 失败后，下游 phase 可以降级运行而非中止（例如 complexity
  检查失败后，reviewer 可收到 "note: complexity gate had an issue (N/A)" 而非不运行）。
- **人类通知渠道**：v1 可以写一个 `～/forge-alert` 脚本（email / Slack / webhook），
  由 forge 在 "呼叫人类" 升级点 shell out。这是一个外挂通道，不内建通知系统。
- **可恢复窗口**：一旦人类介入并修复根因（修复 linter 配置、安装 missing tool），
  应该可以从**失败点**恢复运行（resume），而不是将整个 run 标记为失败。
  `PhaseIndex` checkpoint 已经为这个能力铺好了基础。

### 当前代码中的就绪点

- `asset.OnFail` 的 `Action` 字段可以扩展为 `Action: "escalate"` 新类型，
  区别于当前的 `loop_back`
- `LoopEngine.NoProgress` 已经是分级行为（不是立即 abort，而是计数 → 触发）
- `cmd/forge` 的 `resolveAutoRisk` 已有一个 `--risk` 标志可作为严重度输入的
  命令行入口
- `forge doctor` 子命令可以作为 "呼叫人类时的诊断报告" 的输出点

---

## 总结：优先级建议

| 方向 | 杠杆效应 | 实现成本 | 建议优先级 |
|------|---------|---------|-----------|
| 1. 跨项目知识联邦 | 组织级学习，G4/G5 核心 | 中 | ★★★★ |
| 2. 运行时模型质量自适应 | 真点火质量提升，G3 深化 | 中 | ★★★ |
| 3. Workflow 集成测试 | 长期维护安全，防止回归 | 低（大量就绪点） | ★★★★★ |
| 4. 多租户隔离 | 生产环境安全前提 | 低 | ★★★★ |
| 5. 渐进式告警升级 | 人机协作最高杠杆的后半段 | 高 | ★★ |

**建议立即启动方向 3（Workflow 集成测试）**，因为：
1. 当前代码中几乎所有基础设施已就绪（`DryRunExecutor`、mock gate、spy callback）
2. 低投入高产出，且是方向 1/2/5 的质量护栏
3. 中枢旋钮的维度增加使矩阵组合测试成为刚需

方向 4（多租户隔离）是生产环境部署的安全前提，**建议在运行多个 forge 实例前**
必须实现。当前阶段（单用户 CLI）可暂缓。

方向 5（渐进式升级）虽然杠杆高但影响面大（需要重新设计错误处理哲学并添加
escalation policy schema），建议在 v2 后期或 v3 核心讨论。
