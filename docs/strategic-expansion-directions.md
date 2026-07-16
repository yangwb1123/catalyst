# ForgeOS — 战略扩展方向分析

> **视角**: 资深架构师 / 产品经理 · **方法**: 全局代码库扫描 + 北极星架构对标 + 增长瓶颈分析
> **不写代码**,只做判断与优先级排序。
> **基线**: v2 forge-core (Go stdlib, 零外部依赖, 13 包, 全绿)

---

## 扫描发现总览

### 已实现的核心能力

| 域 | 成熟度 | 关键文件 |
|----|--------|---------|
| 工作流编排引擎 | ✅ | `orchestrator/orchestrator.go` (RunFrom/RunParallel, loop-back, retry, checkpoint) |
| 收敛引擎 | ✅ | `converge/converge.go` (human_gate, roadmap, gates green, signal-based) |
| 模型路由 (Claude-only) | ✅ | `routing/routing.go` (TierForScore, budget guard, history tiebreak) |
| 风险分类器 | ✅ | `risk/risk.go` + `risk/risk_diff.go` (路径启发式) |
| 中枢旋钮 (mode × lifecycle) | ✅ | `mode/mode.go` + `policies/modes.yml` |
| 上下文组装 | ✅ | `prompt/` (Gather, Retrieve, ADR 注入) |
| 跨期记忆 | ✅ | `memory/memory.go` (JSONL, 置信度, 撤回, 裁剪) |
| 结构观测 | ✅ | `trace/trace.go` (JSONL, Span, cost/latency) |
| 完整 CLI 安全护栏 | ✅ | recursion + budget + timeout + output-cap + agent-call count |
| 治理闸门套件 | ✅ | gate/check/accept + SCA + secret-scan + arch-check + 函数长度 |

### 北极星对标缺口 (15 服务目录)

| 北极星服务 | v2 状态 | 差距 |
|-----------|---------|------|
| API Gateway / BFF | ❌ 不存在 | CLI-only, 无 HTTP API |
| Agent Registry / Scheduler | ❌ 不存在 | agent 选择 = workflow YAML 硬编码 |
| **Runner / Sandbox** | **❌ 不存在** | **agent 在宿主机裸跑 — 无隔离** |
| **Cross-Vendor Model Router** | **⚠️ Claude-only** | **仅 3 个 Claude 档, 无跨厂商** |
| Knowledge Engine | ❌ 不存在 | memory 是文件存储, 无向量/语义检索 |
| **Discover 自动化** | **⚠️ 骨架** | **workflow 定义存在, 引擎未实现** |
| Web UI / Dashboard | ❌ 不存在 | CLI-only, 无可视化 |

---

## 方向一: Runner/Sandbox 隔离层 (最高优先级)

### 为什么需要

当前所有 agent 执行 (`CommandExecutor`) 直接在宿主机裸跑——`claude -p` 接受的 `--allowedTools` 中的
`node --test`/`npm install`/`go build` 等命令拥有宿主机的完整文件系统、网络和环境变量访问权。
一个被 prompt 误导的 agent (或恶意输入) 可以:

- 读取 `~/.ssh/*`, `~/.config/*`, 环境变量中的 API key
- 写入任意路径, 覆盖源码或系统文件
- 通过网络对外连接 (数据外泄)
- 消耗宿主机全部 CPU/内存 (资源挤占)

**这直接阻止了 ForgeOS 在以下场景的使用**:
- 多租户 (SaaS / 团队共享 Runner)
- CI/CD 流水线 (与生产凭据同环境)
- 不受信任的第三方代码审查
- 24h 无人值守 (当前本质上是"信任 agent 不犯错")

### 当前代码中的相关锚点

```go
// forge-core/internal/orchestrator/command_executor.go
// CommandExecutor 直接 os/exec 出 agent CLI, 无任何隔离层
type CommandExecutor struct {
    Dir     string        // 工作目录
    Timeout time.Duration // 超时
    // ...
}
```

Builder 模式 (`engine_build.go`) 中的 `--executor=command` 就是必经入侵点——
它已经是一个 `AgentExecutor` 接口实现, 沙箱只需要在此接口下替换为隔离实现:

```
AgentExecutor (interface)
  ├── DryRunExecutor          (当前默认: 只叙述, 不执行)
  ├── CommandExecutor         (当前实际: 宿主机裸跑)
  └── SandboxedExecutor       (未来: 在 Firecracker/gVisor 中执行)
```

### 建议的实现路径 (三阶段, 非破坏性)

| 阶段 | 范围 | 价值 |
|------|------|------|
| **P1 — 文件系统隔离** | 每个 run 创建临时 worktree (`git worktree add`), 通过 `--root` 注入, agent 只能接触该 worktree | **立即消除源码/凭据污染风险**。纯 CLI 改造, 无需 KVM |
| **P2 — 容器化执行** | 每个 agent 调用在一次性 Docker 容器内执行, 挂载 worktree, 限制网络/内存/CPU | **资源隔离 + 网络出站控制**。复用现有容器基础设施 |
| **P3 — microVM (北极星)** | Firecracker microVM, 无持久存储, 出口防火墙, 零控制面凭据 | **多租户安全**。硬件级隔离, 但需要 KVM 支持 |

### 为什么不是"v3 再做"

项目 ROADMAP 说 Sandbox = v3, 但从 Sprint 24-26 的真点火测试来看, **Sprint 25 就已经在
真 `claude --agent-cmd` 上跑通多 agent 闭环了**。当前节点是:

> ★ 真 agent 已经能在宿主机上全权写文件、跑测试、安装依赖。
> 安全隔离不是"下一次架构升级", 而是**当前使用姿势的必备护栏**。

P1 只需要约 **200 行 Go + git CLI 调用**, 零新依赖, 两周内可落地。

---

## 方向二: 自动化需求发现引擎 (Discover Workflow)

### 为什么需要

ForgeOS 的最高论点是 **"需求探索 > 代码实现"** (`.agent/PROJECT.md`), 但当前 discover
workflow 的 three phases (requirement-discovery → market-research → product-designer)
只有 YAML 定义和 mode-gating 的 skip/full 切换, **没有任何自动执行的引擎**:

```yaml
# .agent/workflows/discover.yml — 定义了骨架, 但没有引擎消费
phases:
  - name: requirement-discovery
    agent: product-manager
    description: 从用户 idea 推导完整需求
    confidence_metric: requirement_confidence
  - name: market-research
    agent: researcher
    description: 竞品/技术/能力矩阵
    optional_for: [balanced]
  - name: product-designer
    agent: product-manager
    description: MVP/高级分层, 产出 PRD
```

`converge.go` 已经预留了 `RequirementConfidence` 信号和 `evalRequirementConfidence` 的收敛评估,
但 **RequirementConfidence 的来源 (discover 阶段的 agent 输出) 从未被连接**。

### 当前代码中已就绪的消费端

| 组件 | 位置 | 就绪状态 |
|------|------|---------|
| `converge.Signals.RequirementConfidence` | `internal/converge/converge.go` | **字段已定义**, 但信号源为空 |
| `evalRequirementConfidence()` | `internal/converge/converge.go` | **逻辑已实现**, 阈值比较 |
| `Phase.ConfidenceMetric` | `internal/asset/asset.go` | **数据模型已定义** |
| `mode.Policy.DiscoverDepth` | `internal/mode/mode.go` | **中枢旋钮已就绪**, explorer=skip, engineering=full |
| `StopCondition` `all_of` | `asset/asset.go` | **收敛条件已支持 `requirement_confidence`** |

### 需要构建的部分

| 模块 | 说明 | 估计量 |
|------|------|--------|
| Discover 提示模板 | 让 product-manager agent 做行业/竞品/能力矩阵推导的专用 prompt | 3-4 个 `.md` 文件 |
| 置信度提取器 | 从 agent 输出解析 `confidence: 85/100` 的结构化提取 | ~150 行 Go/Python |
| 能力矩阵骨架 | 产出结构化 `capability-matrix.json` 供下游架构师消费 | ~200 行 Go |
| Discover → Design 链路 | confidence ≥ 80% 才出 discover, 注入 `project.yml` | ~100 行 + CI 适配 |

### 产品价值

这是 ForgeOS **与其他编码 CLI 的根本差异**——让 AI 在写一行代码之前先做系统性的需求推导。
没有这个引擎, ForgeOS 就是一个"更会调 Claude"的编排器; 有了它, 才配叫"AI-native 软件工厂"。

---

## 方向三: 跨厂商模型路由池 (Model Router v3)

### 为什么需要

当前 `internal/routing` 是针对 Claude 三档 (Haiku/Sonnet/Opus) 的硬编码:

```go
// forge-core/internal/routing/routing.go
var ModelMap = map[string]map[string]string{
    "anthropic": {
        Haiku:  "claude-sonnet-4-haiku",
        Sonnet: "claude-sonnet-4",
        Opus:   "claude-opus-4",
    },
}
```

这产生三个问题:

1. **厂商锁定** — 无法在 deepseek 便宜时切换, 无法在 GPT-4o 特定任务好时选用
2. **成本无优化空间** — 只有一条成本曲线 (Claude 定价), 无法做跨厂商的性价比路由
3. **韧性差** — Anthropic 中断 = 整个 ForgeOS 不可用

### 当前架构中的就绪部分

`routing.go` 的设计已经为跨厂商做了大量准备:

| 组件 | 位置 | 状态 |
|------|------|------|
| `CandidatesForTier()` | `routing.go` | **已实现**: 返回同 tier 候选列表 [opus, sonnet, haiku] |
| `HistoryTiebreak()` | `scorecard.go` | **已实现**: 从 scorecard 选 quality 最高的候选 |
| `ResolveModel(provider, tier)` | `routing.go` | **已实现**: provider+tier → 模型名 |
| `BudgetAdjustTier()` | `routing.go` | **已实现**: 预算守卫降档 |
| `scorecard.schema.yml` | `.agent/routing/` | **已定义**: model + task_type + quality_score |
| `cross_vendor_pool_v3` | `policy.yml` | **占位**: 声明未激活 |

### 需要构建的部分

| 模块 | 说明 | 估计量 |
|------|------|--------|
| LiteLLM 网关集成 | 统一 OpenAI/Anthropic/Google/DeepSeek 的 API 格式 | ~300 行 Go (走 HTTP) |
| 模型目录 | 各厂商模型的能力/价格/延迟注册表 | ~200 行 YAML + Go |
| 性价比评分 | 将 cost_per_token + latency + quality 纳入路由 | ~200 行 (扩展 Score 维度) |
| 回退链 | 主模型失败 → 降级到次优模型 | ~100 行 (已有 retry 基础) |

### 产品价值

跨厂商路由是 ROADMAP 的 v3 目标, 但当前市场生态中 **DeepSeek/Qwen/GPT-4o-mini 的
价格-质量曲线已经与 Claude 形成有意义的竞争**。拖到 v3 再启动意味着:

> 在 6-12 个月里, 每次 DeepSeek 降价用户都会问"为什么 ForgeOS 不能用?"

---

## 方向四: 可观测性 Dashboard + Human-in-the-loop 界面

### 为什么需要

ForgeOS 目前是 **纯 CLI + JSONL 文件** 的可观测性:

```bash
.forge/
  trace.jsonl     # agent 运行的结构化记录
  checkpoint.json # 断点恢复
  memory.jsonl    # 跨期知识
```

对于 24h 自治运行, 人类需要:

1. **实时监控** — 当前 run 在哪个 phase? 花了多少钱? 还剩多少预算?
2. **历史回溯** — 昨天凌晨 3 点的 run 为什么 converge 了? 哪个 gate 失败了?
3. **成本可视化** — 本周跑了多少次? 各 model 各花了多少? 趋势?
4. **人审界面** — `design.yml` 的 human_gate 当前是 `--approved` flag, 需要一个 Web 界面做审批
5. **告警** — budget 即将耗尽 / converge 卡住 / agent 反复 retry

### 当前代码中的就绪部分

| 组件 | 位置 | 状态 |
|------|------|------|
| `trace.Event` | `internal/trace/trace.go` | **全量事件**: iteration/gate/agent/converge, 含 duration+model+cost |
| `scorecard.mjs` | `harness/scorecard.mjs` | **p95 latency + avg cost** 统计 |
| `converge.Signals` | `internal/converge/converge.go` | **收敛信号**: roadmap%, gates green, cost, latency |
| `memory.Entry` | `internal/memory/memory.go` | **跨期知识**: gap/decision/lesson |
| `LoopEngine.OnIteration` | `orchestrator/loop.go` | **每次迭代后的钩子** |
| Human gate | `converge.go` + cmd/forge | **approval 信号已经定义** |

### 建议的最小可行产品

| 模块 | 说明 | 估计 |
|------|------|------|
| **事件流 API** | `forge events` / `forge events --watch` — 实时流式输出 trace 事件到 stdout/SSE | ~100 行 Go |
| **Web 仪表盘** | 单页 HTML (无构建步骤), 读 `.forge/trace.jsonl` + `.forge/memory.jsonl`, 展示 run 历史、cost、收敛状态 | ~1 个 HTML 文件 (含内联 JS) |
| **审批端点** | `forge approve <stage>` — 替代 `--approved`, 支持 Webhook / API | ~100 行 Go |
| **告警钩子** | budget 告警 / converge 超时 → call webhook / email | ~150 行 Go |

### 为什么不是"Web UI 偏离 CLI 核心"

观察发现:

> **当前 trace.jsonl + evolve loop 已经产生了足够丰富的数据, 但没有被人类消费。**
> 一个 run 结束后, 没有地方可以看到"这次 evolve 跑了 12 轮, 花了 $8.40, 最后是因为
> budget 耗尽停止, 有 3 个 gate 一直 FAIL"——这些信息在 JSONL 里, 但访问成本太高。

一个**极简的、不依赖任何框架的纯静态 HTML 仪表盘** (嵌入 CLI 或放在 `.forge/`) 就能解决
80% 的可视化需求, 而不违反"CLI/声明式核心"的架构原则。

---

## 方向五: 知识引擎 — 语义化跨项目知识传承

### 为什么需要

当前 `internal/memory` 是 JSONL 文件式的键值存储, 查询只支持精确匹配 `Query(kind, topic)`:

```go
// forge-core/internal/memory/memory.go
func Query(entries []Entry, kind, topic string) []Entry {
    // 只做精确字符串匹配, 不做语义相似度
}
```

在以下场景中, 当前设计无法满足:

1. **跨项目知识共享** — 项目 A 学到了"使用 Postgres 连接池的最佳大小是 20", 项目 B 无法自动引用
2. **语义检索** — agent 问"我们之前处理过数据库死锁吗?", 应该能匹配 `topic="transaction_deadlock"`, `topic="pg_lock_timeout"`, `topic="mysql_innodb_lock"` 等多个条目
3. **自动知识点提取** — evolve 循环中的失败/修复/决策应该被自动摘要并写入知识库, 而不是靠 agent 主动调用 `memory.Append`
4. **知识衰减与演替** — 旧的知识应该随时间衰减, 被新的实践经验覆盖

### 当前架构中的就绪部分

| 组件 | 位置 | 状态 |
|------|------|------|
| `memory.Entry` | `internal/memory/memory.go` | Kind/Topic/Detail/Confidence/Supersedes 已定义 |
| `memory.Prune` | `internal/memory/memory.go` | 裁剪旧条目 |
| `filterSuperseded` | `internal/memory/memory.go` | 撤回机制已实现 |
| `prompt.Gather` | `internal/prompt/prompt.go` | 上下文注入入口 |
| `scorecard` | `internal/routing/scorecard.go` | 记分卡已就绪 |
| `docs/adr/` | `docs/adr/` | ADR 已就绪 |

### 需要构建的部分

| 模块 | 说明 | 估计量 |
|------|------|--------|
| **嵌入索引** | 轻量级 TF-IDF 或 sentence-transformers 嵌入存储 | ~300 行 Go/Python |
| **语义搜索** | `Search(query string, topK int) []Entry` — 按语义相似度返回 | ~200 行 |
| **自动摘要** | evolve 循环结束后, 自动摘要 gap/decision/lesson 写入 memory | ~150 行 (走 agent executor) |
| **跨项目存储** | 可选共享知识库路径 (通过 `--knowledge-store` 或 `FORGE_KNOWLEDGE_STORE` 环境变量) | ~100 行 |

注意: 项目已经明确 **TF-IDF 检索器已工作** (`prompt/retrieve.go`), 所以嵌入层可以复用
已有的 `internal/prompt` 基础设施。

### 产品价值

知识传承是将 ForgeOS 从"单次运行的编排器"升级为"持续学习的软件工厂"的**关键一环**。
没有它:

> **第 100 次 evolve 循环的知识和第 1 次一样少。项目从错误中学到的东西, 每个新 run 都要重新发现。**

---

## 优先级矩阵

| 方向 | 用户价值 | 技术风险 | 依赖数 | 估计工作量 | 北极星对齐 | **优先级** |
|------|---------|---------|--------|-----------|-----------|----------|
| **① Sandbox 隔离** | 🔴 高 (安全) | 🟢 低 (P1 纯 Git) | 0 | P1: 2周, P2: 4周 | ★★★ 控制面/数据面分离 | **P0** |
| **② Discover 引擎** | 🔴 高 (差异化) | 🟡 中 (prompt 工程) | 1 (engine_build) | 4-6 周 | ★★★ 核心论点 | **P1** |
| **③ 跨厂商路由** | 🟡 中 (降本) | 🟡 中 (多 API) | 2 (LiteLLM) | 4-6 周 | ★★★ 北极星服务 | **P1** |
| **④ 仪表盘/人审** | 🟡 中 (可观测) | 🟢 低 (静态 HTML) | 0 | P0实时: 1周, P0仪表盘: 2周 | ★★ 平台 | **P1** |
| **⑤ 知识引擎** | 🟢 渐进 (学习) | 🟡 中 (嵌入) | 1 (prompt) | 3-5 周 | ★★ Memory/Knowledge | **P2** |

### 短期建议 (下一个 4 周)

```
Week 1-2:  ① Sandbox P1 (git worktree 隔离) + ④ 事件流 API
Week 3-4:  ② Discover prompt 骨架 + ③ 模型目录 (1-2 额外厂商)
```

### 中期建议 (2-3 个月)

```
Month 2:   ① Sandbox P2 (容器化) + ② 置信度提取 + ④ 静态仪表盘
Month 3:   ③ LiteLLM 网关 + ⑤ 语义检索原型
```

---

## 补充: 已识别但暂缓的低优先级方向

| 方向 | 暂缓原因 |
|------|---------|
| **Temporal 持久化工作流** | 单机 checkpoint/resume 已够用; Temporal 引入会打破零依赖, 且收益在多 worker 场景才明显 |
| **OPA/Rego 策略引擎** | YAML + check.py 在当前规模下够用; OPA 引入需要独立的 PDP 服务 |
| **Web UI (完整 Next.js)** | 偏离 CLI/声明式核心; 极简仪表盘 (方向四) 已覆盖 80% 需求 |
| **Agent Registry & Scheduler** | 当前单 executor 模式无需调度; 多 Runner 时才需要 |
| **WASM 便携 Gate** | 边缘场景; 当前 polyglot adapter 架构已足够 |
| **独立 agent-os 仓库** | ADR 0003 设计就绪, 但触发条件未到 (需要 fork 场景) |

---

*生成日期: 2026-07-01 · 基线: forge-core v2 (0 external deps, 13 packages, all green)*
*扫描范围: forge-core/ (Go 41 源文件)、harness/ (Node/Python 25 文件)、.agent/ (40+ 声明文件)、docs/ (15+ 分析文档)*
