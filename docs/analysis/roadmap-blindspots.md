# ForgeOS — 路线图盲点：项目自己没标注的缺口

> **第八次扫描**，这次用**项目自己的路线图作为标尺**
> —— 比对 ROADMAP.md 的声明与当前实现，找出项目没识别的盲点。
>
> 不写代码，只做策略判断。

---

## 目录

1. [项目已知的缺口 vs 未被识别的缺口](#1-项目已知的缺口-vs-未被识别的缺口)
2. [盲点 1：forge-core 架构无 ADR 记录](#2-盲点-1forge-core-架构无-adr-记录)
3. [盲点 2：Go YAML 库迁移无计划](#3-盲点-2go-yaml-库迁移无计划)
4. [盲点 3：性能基线缺失](#4-盲点-3性能基线缺失)
5. [盲点 4：forge-core 的贡献与开发体验](#5-盲点-4forge-core-的贡献与开发体验)
6. [盲点 5：中枢旋钮的用户文档缺失](#6-盲点-5中枢旋钮的用户文档缺失)
7. [每个盲点的推荐方向](#7-每个盲点的推荐方向-从立即行动到策略储备)

---

## 1. 项目已知的缺口 vs 未被识别的缺口

### 项目 ROADMAP v2 明确标注的缺口

ROADMAP.md 的 v2 节诚实标注了五个"明确遗留缺口"：

| 项目已知缺口 | 状态 | 项目标注 |
|-------------|------|---------|
| Agent 默认 dry-run | ✅ 已有真执行器，默认 dry 安全 | 诚实标注 |
| YAML python shim 转码 | ⚠️ 79 行 Python 脚本，79 个 sprint 未动 | "临时脚手架" |
| 独立 agent-os 仓库 | ❌ 未开始 | "推荐暂缓" |
| Eval→记分卡→Router 闭环 | ⚠️ `forge route` 有 HistoryTiebreak，但 non-automatic | "仍待" |
| ADR/RAG | ❌ 未实现 | "仍待" |

### 未被识别的盲点（本次分析发现）

| 盲点 | 类型 | 影响 | 严重性 |
|------|------|------|--------|
| forge-core 架构无 ADR | 架构文档化 | ADR-0001 已被 Superseded，但无后继 ADR 记录 forge-core 的设计决策 | 中 |
| Go YAML 库无迁移计划 | 技术债务 | python shim 拖了 79 个 sprint，无迁移时间表、无评估标准、无候选库 | 中 |
| 无性能基线 | 质量保证 | trace 记录 latency/cost，但无可比基准线、无回归检测 | 中-高 |
| forge-core 无贡献指南 | 开发者体验 | forge-core README 只讲 build/test/run，无架构总览、无开发流程 | 低-中 |
| 中枢旋钮 48+ 状态无用户文档 | 采纳体验 | mode×lifecycle 的 16 组合 × 3 子系统的交互效果无集中文档 | 低-中 |
| forge-core 自观测缺失 | 可运维性 | 能观测 agent 的行为，但不能观测 orchestrator 自身的性能 | 低 |
| memory 有效性未度量 | 功能验证 | memory.go 和 retrieve.go 已建，但无人验证它们是否实际改善了 prompt 质量 | 中 |

---

## 2. 盲点 1：forge-core 架构无 ADR 记录

### 问题

ADR-0001（ride Claude Code v0-v1）**已被标记为 Superseded**，取代条件是"核心循环在 CC 上验证稳定"——该条件已由 url-shortener dogfood 触发，forge-core 也已落地。

但**没有任何后继 ADR 记录 forge-core 的架构决策**：

- 为什么 orchestrator 用 `for` 循环 + 索引跳跃实现 loop-back，而不是状态机模式？
- 为什么 `Engine` 用注入回调（OnGateResult、AgentVerdict、BudgetExhausted）而不是 channel/event bus？
- 为什么 checkpoint 用 `rename(2)` 原子写入而不是 WAL/事务日志？
- 为什么 `memory` 用 JSONL append-only 而不是 SQLite/bolt？
- 为什么 `trace` 用同步写（mutex）而不是异步 buffer + batch flush？

这些决策都有充分的理由（零外部依赖、纯标准库、可测试性），但**文档中没有记录"为什么选择这个方案而不是另一个"**。当一个新贡献者（或六个月后的原作者）看到这些代码时，他们需要重新推导这些决策。ADR 的价值就在于省去这种重新推导。

### 对比

| ADR-0001 | forge-core（当前） |
|----------|------------------|
| 状态: Superseded | ❌ 无 ADR 记录 |
| 决策: "骑 Claude Code，推迟自研运行时" | 决策: "用 for 循环 + 索引跳跃实现 loop-back" |
| 取代条件触发 → forge-core 开建 | 哪份文档记录了这个设计选择？ |

### 建议

```
docs/adr/0004-forge-core-orchestrator-architecture.md

应记录：
- 为什么 loop-back 用 for + index manipulation 而不是状态机
- 为什么 Engine 用回调注入而不是 event bus
- 为什么 checkpoint 用 rename(2) 原子写入
- 为什么 memory 用 JSONL append-only
- 为什么 trace 用同步 mutex 写
- 为什么入参选择 0-value = back-compat（零值向后兼容）
```

---

## 3. 盲点 2：Go YAML 库无迁移计划

### 问题

ROADMAP 标注了 "YAML 经 python shim 转码" 作为已知缺口，注释说"未来可换 Go YAML 库——属 architect/cto 的依赖决策"。

但"未来"是何时？79 个开发 sprint 过去了，这个 python shim（79 行）仍然是 forge-core 的唯一外部依赖桥梁。没有任何：

- 迁移时间表（v2？v2.x？v3？）
- 评估标准（什么 YAML 库？goccy/go-yaml？ghodss/yaml？标准库在 Go 1.26 仍然没有 YAML）
- 升级触发条件（PyYAML 安全漏洞？性能瓶颈？）
- 降级影响评估（如果迁移失败，回退路径是什么？）

### 风险

**shim 在当前运行良好**——79 行，确定性输出，有 `test_yaml2json.py` 测试。在 python3 + PyYAML 可用的环境中没问题。

但问题在于：**forge-core 宣称"零外部依赖"**——`go.mod` 无 require 块。实际上它运行时依赖 python3 + PyYAML。这是一个**不纯的零依赖声明**。

### 建议

```
评估: 在 ROADMAP 中给 python shim 一个具体版本目标
  - v2.1: 选 Go YAML 库 + 迁移 + 保留 shim 作为 fallback
  - 或明确接受: python shim 是永久桥接,forge-core 仅"零外部 Go 依赖"
    （不是"零任何外部依赖"），更新 ROADMAP 措辞以反映真相
```

---

## 4. 盲点 3：性能基线缺失

### 问题

ForgeOS 有**完整的性能数据管道**——trace 系统记录每个 agent phase 的 `duration_ms` 和 `cost_usd_micros`，scorecard 计算 p95 latency 和 avg cost。管道端到端已验证（Sprint 26 真 claude 坐实了 latency=2640ms、cost=$0.1841 进入 scorecard）。

但**没有基线、没有比较、没有回归检测**：

```
当前: trace → scorecard → (人类阅读)
所需: trace → scorecard → 基线比较 → (如果 p95 比上周慢 2x → 报警)
```

具体缺失：

- **无历史基线**：scorecard 的 `avg_iterations`、`rework_rate`、`p95_latency_ms`、`avg_cost_usd` 都只记录当前值，不与过去比较
- **无回归阈值**：如果一次代码更改导致 agent latency 从 2.6s 跳到 5.2s，没有人会知道
- **无跨版本比较**：无法比较 v2.3 与 v2.4 的平均成本或成功率
- **无场景隔离**：所有 trace 混合在一起（不同的 workflow、不同的 task_type、不同的 mode），无法单独评估"build 在 engineering 下的平均 cost"

### 建议

```
低成本的起点：forge scorecard diff 命令
  比较当前 scorecards.json 与上一次提交的 scorecards.json
  报告：哪些 (model, task_type) 对的 quality_score/p95/avg_cost 变化超过 20%
  这不需要任何基础设施——只比较两个 JSON 文件

中期：trace 基线数据库
  将每次 forge evolve 的 trace 摘要存入一个轻量级基线文件
  下次运行时比较：latency delta、cost delta、convergence rate
```

---

## 5. 盲点 4：forge-core 的贡献与开发体验

### 问题

forge-core 有 86 个 Go 文件、20k LOC、13 个内部包。它的架构是整洁的（`internal/` 包有明确的依赖方向），但**没有为贡献者准备的指引**。

forge-core/README.md 只包含：

```
- 构建/测试/运行说明（6 个 go 命令）
- 目录清单（一行一个包）
- 编排器的简短说明
- 诚实局限性
- 路由安全底线
```

没有：

- **架构概览图或说明**——包之间的依赖方向（当前只能通过 `go list` 或阅读源码推导）
- **"如何添加一个新子命令"**——`forge <new>` 需要在 `main.go` 的 `run()` 中注册、创建函数、定义 flag
- **"如何添加一个新的 gate 类型"**——需要在 `harness/acceptance.mjs` 和 `internal/converge` 中添加
- **"如何添加一个新的 orchestration feature"**——需要在 `Engine` 中加字段 + `RunFrom` 中处理 + `LoopEngine` 中传播
- **开发流程**——如何运行特定的测试子集、如何调试 engine 行为、如何模拟 gate 结果

### 对比 forge-init 生成的项目

一个通过 `forge-init` 创建的新项目获得了完整的治理文档（AGENTS.md、ARCHITECTURE.md、PROJECT.md、ROADMAP.md、CURRENT_SPRINT.md），但 **forge-core 自身**——运行时的核心——没有同等级别的开发文档。

### 建议

```
forge-core/CONTRIBUTING.md

内容：
1. 包依赖图（ASCII 图或简短描述）
2. 添加新子命令的步骤（5 步清单）
3. 测试策略（什么时候写 pure unit test vs cmd subprocess test）
4. 调试技术（如何用 --executor echo 测试 prompt 构建）
5. 如何运行特定测试子集
6. 架构决策的记录位置（ADR）
```

---

## 6. 盲点 5：中枢旋钮的用户文档缺失

### 问题

中枢旋钮（`mode × lifecycle`）是 ForgeOS 最强大的抽象——一个设置驱动三处子系统：
Router 档位、Harness 严格度、Workflow 深度。

当前有 **16 种组合 × 3 个子系统 = 48+ 种行为状态**。但用户的文档入口是：

- `modes.yml`（140 行 YAML + 注释）——权威但需要阅读整个文件来理解叠加规则
- `ARCHITECTURE.md` 的"中枢旋钮"段（约 15 行）——概述但不包含细节
- `forge run --help`（一行 `--mode` + 一行 `--lifecycle`）——只列了合法值没有解释

**没有任何地方说"对于你的场景，应该使用什么组合"**。例如：

```
场景                          推荐组合
"我在验证一个想法，只关心能跑"   mode=explorer, lifecycle=idea
"我在开发特性，需要质量保障"     mode=balanced, lifecycle=mvp
"我在做安全关键系统"            mode=engineering, lifecycle=production
"我只想写架构文档，不写代码"     mode=cto, lifecycle=any
```

| lifecycle → mode ↓ | idea | mvp | growth | production |
|-------------------|------|-----|--------|------------|
| **explorer** | 最快路径，跳过一切 | 快但基本测试 | ❌ 少见组合 | ❌ 矛盾（快 vs 严）但受 production override 强制执行 |
| **balanced** | 轻松探索 | ✅ 大多数项目的起点 | 收紧中 | 全闸门 |
| **engineering** | 太重 | ✅ ForgeOS 自身 | 标准全开 | 最严全开 |
| **cto** | 只产出分析 | 只产出分析 | 只产出分析 | 只产出分析（build halt） |

### 建议

```
在 ARCHITECTURE.md 或独立的使用指南中加入:
- 场景→推荐组合表（如上）
- "mode 覆盖 lifecycle"和"lifecycle 收紧 mode"的规则示例
- 常见反模式（例如 explorer+production 的矛盾组合及其实际效果）
- forge run 的 --mode/--lifecycle 标志的详细说明

最小的改动：forge run --help 的输出增加场景建议
```

---

## 7. 每个盲点的推荐方向（从立即行动到策略储备）

| 优先级 | 盲点 | 行动 | 成本 | 影响 | 分类 |
|--------|------|------|------|------|------|
| 🥇 | **性能基线** | 增加 `forge scorecard diff` 命令 | 低-中 | 中-高：防止无声回归 | 运维增强 |
| 🥇 | **forge-core ADR** | 写 `docs/adr/0004-forge-core-architecture.md` | 低 | 中：捕获已做决策 | 文档补充 |
| 🥈 | **Go YAML 库计划** | ROADMAP 中给 python shim 加退出版本 | 低 | 低-中：明确方向 | 策略澄清 |
| 🥈 | **中枢旋钮文档** | 合并场景表到 ARCHITECTURE.md | 低 | 低-中：改善采纳体验 | 用户体验 |
| 🥉 | **forge-core 贡献指南** | `forge-core/CONTRIBUTING.md` | 中 | 低-中：为多人协作铺路 | 开发者体验 |
| 🔮 | **memory 有效性度量** | develop a "with/without memory" test to compare prompt quality | 高 | 中：验证功能有效性 | 质量验证 |

---

**路线图盲点的共同主题**：项目在 **功能交付**上非常自律（sprint 1-26 交付了难以想象的工作量），但在 **文档化已做决策**、**建立性能基线**、**为贡献者铺路** 方面存在盲点。这些都不是功能缺口，而是**工程成熟度缺口**——与 v0 止血和 v1 闭环的务实精神一致，它们是**下一个阶段的自然关注点**。

*分析日期：2026-06-29 | 基于第八次全量扫描（ROADMAP vs 现实差距视角）*
