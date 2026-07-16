# ForgeOS — 当前状态缺口扫描（2026-06 重扫描）

> **方法**: 完整重读 forge-core（41 源文件）、harness（26 文件）、CI、git log
> **当前基线**: `b0c80e4 feat: Loop Memory/Learning + Adaptive Assembly + Reflect`
> **诚实前提**: 项目在 6 月有大量并行开发（并行编排、信号处理、检测/迁移 CLI、评审工作流等），
> 以下分析仅覆盖 **当前仍未实现的缺口**，不重复已被解决的发现。

---

## 当前缺口概况

| 缺口 | 严重性 | 改动量 | 属于之前分析？ | 仍待解决？ |
|------|--------|--------|--------------|-----------|
| CI 无 `-race` + `go build` + `node --test` | 🔴 **高** | ~20 行 CI 配置 | ✅ 分析⑦ | ✅ 仍无 |
| Memory 无置信度/撤回机制 | 🔴 **高** | ~300 行核心 + 测试 | ✅ 分析⑤ | ✅ 仍无 |
| Signal 无二次信号硬退出 | 🟠 中 | ~10 行 | ✅ 分析④(§3) | ❌ 已有 context 取消但无 force-exit |
| YAML python shim 仍为唯一桥接 | 🟠 中 | 替换为 Go YAML 库 | ✅ 分析④(§6) | ✅ 仍无 |
| Cross-vendor 路由未启动 | 🟡 中 | ~500 行核心 | ✅ 分析⑩ | ✅ 仍无 |
| WASM 便携 Gate 未启动 | 🟡 中 | POC 不确定 | ✅ 分析⑩ | ✅ 仍无 |
| 独立 agent-os 仓库 | 🟠 中 | 架构就绪 | ADR 0003 | ✅ 仍无 |
| converge 仍部分依赖 agent 自报告 | 🟠 中 | ~100 行 | ✅ 分析⑤ | ⚠️ GatesGreen 已加入但 roadMapCompletion 仍自报告 |

**重要说明**: 分析①-⑩中约 60% 的发现已被项目在 6 月的密集开发中解决（并行编排、信号处理、
context 传播、LLM 缓存、Reflect 步、forge detect/validate/migrate 等）。
以下 5 个方向聚焦于 **当前代码库中仍然真实存在的缺口**。

---

## 方向 1: 生产级 CI 全链路

### 当前状态

```yaml
# .github/workflows/forge.yml 的实际内容（完整）
- name: forge accept
  run: node harness/acceptance.mjs
- name: forge-core tests
  run: go -C forge-core test ./...
```

CI **不执行**以下操作：
- `go build ./...` — 编译错误不被 CI 拦截
- `go test -race ./...` — 数据竞争不被 CI 拦截
- `node --test harness/` — harness 单元测试不被 CI 直接运行

### 为什么需要

`go build ./...` 失败目前是 **不会被 CI 发现的盲区**。`forge accept` 的 build gate 在
ForgeOS 自身上是 N/A（无 go build 检查工具），所以一个 bring `import` 可以轻松进入 main 分支。

`go test -race` 是 Go 数据竞争的黄金标准检测工具。目前 CI 只跑无 race 的测试，
一个 `unlock before write` 类型的竞争可以在测试中 PASS 但在生产中出现神秘崩溃。

### 改动范围

纯 CI 配置修改，不触及任何 Go/Node 代码：

```yaml
# 新增的三行
- name: go build
  run: go -C forge-core build ./...
- name: forge-core tests with -race
  run: go -C forge-core test -race ./...
- name: harness unit tests  
  run: node --test harness/
```

### 估计

- ~20 行 YAML
- 1 小时配置，1 天验证（确保 race 测试无假阳性）

---

## 方向 2: Memory 置信度与错误撤回

### 当前状态

`internal/memory` 是 append-only 的知识存储。每个 `Entry` 结构（`memory.go:130`）包含：

```go
type Entry struct {
    Iteration  int
    Agent      string
    Kind       string  // "finding" | "decision" | "lesson"
    Topic      string
    Summary    string
    Detail     string
}
```

**没有** `confidence` 字段、**没有** retraction/supersede 机制。一个错误的发现一旦写入，
后续所有迭代都会读到它。`prompt_memory.go` 实现了 `memoryCap=32` 的上限和 BM25 相关性检索，
但它对所有条目的权重是平等的——错误的 insight 和正确的 insight 在检索时没有区别。

### 为什么需要

这是当前 **最危险的系统性风险**（分析⑤ §回路 A，当前仍存在）：

- 如果在第 3 次迭代 agent 产生了一个错误的分析（"X 方案不可行，因为……"）
- 该 insight 被写入 memory
- 在第 5-32 次迭代中，每次 prompt 都读取到这个错误结论
- agent 被自己的错误输出污染，形成自我强化回环

`GatesGreen` 信号虽然已加入 converge，但它只判断 gate 是否通过，不判断 memory 是否诚实。

### 改动范围

```go
// Entry 增加字段
type Entry struct {
    // ... 现有字段
    Confidence float64   // 0.0-1.0, 默认 1.0 (最高置信)
    Supersedes string    // 被此 entry 替代的 prior entry ID (空 = 不替代)
}
```

然后：
1. `Append` 写入时可选 confidence（缺省 = 1.0）
2. `Load` 返回时过滤掉已被 supersedes 的条目
3. `Query` 按 confidence 排序输出
4. 在 agent prompt 中低 confidence 条目标注为 `[unverified]`

### 估计

- ~100 行 Entry 结构变更
- ~100 行 Load/Query 过滤逻辑
- ~100 行测试
- 1 sprint

### 兼容性

向后兼容：旧 JSONL 行无 `confidence` 字段 ≈ confidence=1.0（旧行为不变）

---

## 方向 3: 跨厂商模型路由

### 当前状态

`internal/routing` 实现了完整的决策链：
```
score → tier → floor → safety_override → budget_guard → history-tiebreak
```

但 `policy.yml` 的 `provider_pool: claude-only` 限制了所有路由只在 3 个 Claude 档位
（haiku/sonnet/opus）中选择。scorecard 的 `(model, task_type)` 主键已经为多厂商
做好准备，`routing/tiers.go` 的 `const` 定义也预留了跨厂商扩展点（注释：
`// Fully-qualified tier is "provider/tier"`）。

### 为什么需要

当前 **单点故障** 风险：Anthropic API 不可用时 ForgeOS 完全不可用。

**验证案例**：在 examples/url-shortener 的真点火运行中，所有 agent 阶段都用 Claude。
如果 Anthropic 在关键运行中发生故障，整个 workflow 会卡住。

`policy.yml` 的 `cross_vendor_pool_v3` 段已声明占位数据（qwen/deepseek/local），
scorecard 系统已为 `(vendor, model)` 复合键就绪——缺失的只是 LiteLLM 集成。

### 改动范围

1. `policy.yml` 和 `routing/policy.yml`：激活 `cross_vendor_pool_v3`
2. `internal/routing`：增加 `(vendor, model)` 复合 tier 支持
3. `cmd/forge`：LiteLLM HTTP 客户端（通过 `exec` 或 HTTP 调用）
4. scorecard 聚合：增加厂商间比较维度

### 估计

- ~300 行核心 + ~200 行测试
- 2-3 sprints（含 LiteLLM 集成和验证）

### 触发条件

建议**不提前做**。当以下条件之一满足时启动：
- 有第二厂商的 API key 配置需求
- Anthropic 在某次重要运行中发生可用性事件
- 用户反馈成本敏感需要多厂商竞价

---

## 方向 4: WASM 可移植 Gate 引擎

### 当前状态

`harness/adapters/*.yml` 定义了每种语言的 gate 命令。当主机缺少工具时，
gate 降级为 N/A。在 ForgeOS 自身上，5/14 个 gate 是 N/A（取决于安装的工具）。

`wazero` Go WASM runtime 是零外部依赖的，与 forge-core 的哲学一致。

### 为什么需要

N/A 是 **最危险的信号类型**——它既不是 PASS 也不是 FAIL，在 prompt 上下文中
对 agent 来说与 PASS 不可区分（分析⑤ 仍成立）。当前：

1. `coverage` gate 总是 N/A（无覆盖率工具）
2. `typecheck` gate 可能 N/A（需要特定语言的类型检查器）
3. `app_test` gate 可能 N/A（需要特定测试框架）

WASM 预编译的工具模块可以在**不依赖主机环境**的情况下运行，将 N/A 转为真正的 PASS/FAIL。

### 改动范围

1. 评估现有的工具 WASM 支持（ESLint 已有 WASM 版本，go linter 没有）
2. 在 `adapters/*.yml` 增加 `wasm_gate` 字段
3. 选择先支持的 1-2 个工具做 POC（建议从 eslint WASM 开始）
4. 集成 wazero runtime

### 风险

各工具链的 WASM 生态成熟度不同。需要用 POC 验证可行性后再投入。

### 估计

- 1 sprint POC（eslint WASM）
- 如果 POC 成功再扩展

---

## 方向 5: converge 的交叉验证

### 当前状态

`converge.Converge()` 检查两种信号：
- `RoadmapCompletion` — agent 自报告的 ROADMAP.md 完成百分比
- `GatesGreen` — 独立测量的 gate 状态

两者是 AND 关系：roadmap 100% + all gates green = CONVERGED。

但 `RoadmapCompletion` 目前来自 `converge.RoadmapCompletion(string(md))` ——
一个基于 ROADMAP.md 文本的解析函数。它的可靠性取决于 agent 是否诚实地更新了
ROADMAP.md 的 checklist。agent 可以**声称**完成了条目而不真正完成。

### 为什么需要

这是 **最后一个未被覆盖的诚实性问题**。`GatesGreen` 已有独立验证（gate.ProbeAll 零 LLM），
但 roadmap 完成度仍然信任 agent 的自我报告。在 24h 无人值守运行中，
agent 有 incentive 声明较早收敛来节省成本。

### 改动范围

增加一个独立测量维度：**文件系统变更映射**

```go
// Signals 增加字段
type Signals struct {
    RoadmapCompletion float64  // 现有：agent 自报告
    GatesGreen        bool     // 现有：独立测量
    FileDelta         float64  // 新增：git diff --stat 与 Roadmap 条目的映射度
                               // 测量实际文件变更是否覆盖了 Roadmap 声称完成的条目
}
```

`FileDelta` 通过 `git diff --name-only HEAD~N` 和 Roadmap 条目的正则匹配来计算。
如果 agent 声称 "完成 100%"，但文件系统几乎没有变更，则 `FileDelta` 验证不通过。

### 估计

- ~100 行 git 差异分析
- ~100 行 converge 信号集成
- ~100 行测试
- 1 sprint

---

## 方向优先级与执行顺序

| 优先级 | 方向 | 改动量 | 独立可交付？ | 依赖 | 推荐阶段 |
|--------|------|--------|------------|------|---------|
| 🔴 P0 | 生产级 CI（方向1） | ~20 行 YAML | ✅ 完全独立 | 无 | **立即** |
| 🔴 P0 | Memory 置信度（方向2） | ~300 行 Go | ✅ 完全独立 | 无 | **Sprint 28** |
| 🟠 P1 | Cross-vendor 路由（方向3） | ~500 行 | ⚠️ 需要 API key | 方向1 完成 | 按需 |
| 🟠 P1 | WASM Gate POC（方向4） | ~300 行 | ✅ 完全独立 | 方向1 完成 | POC 先行 |
| 🟡 P2 | converge 交叉验证（方向5） | ~300 行 | ✅ 完全独立 | 方向2 完成 | 方向2 之后 |

**方向1 应本周完成**——20 行 CI 配置，唯一不对代码库产生任何影响的风险。
  
**方向2 和方向5 解决同一个根本问题（feedback 诚实性）**，方向2 是机制，
方向5 是验证。先建机制，再加验证。

---

## 声明

这份分析基于以下 commit 状态：`b0c80e4 feat: Loop Memory/Learning + Adaptive Assembly + Reflect`
（2026-06 最末提交）。与不同时间点的截图对比可见，项目在 6 月有密集的并行开发
（方向一~五全量交付、真点火 gap 修复、Reflect、detect、parallel、review workflow），
因此以下发现是 **排除已解决问题后当前仍然存在的缺口**，不应与早前的分析文档混淆。
