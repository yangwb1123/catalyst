读完了你的全部分析文档。以下是我的**同行评审意见**。

---

## 总体评价

这是一篇**质量极高的工程扫描**。最突出的三点：

1. **证据链扎实** — 每个方向都追到了 `file:line` 级别，且引用的是「代码实际行为」而非「文档声明」。这一点超越了 `docs/analysis/` 中不少偏概念的分析。
2. **边界情况不敷衍** — 比如方向一的 `Geo-routing`、方向三的 `embedding 冷启动 fallback`、方向五的 `Heisenbug + -count=1000`，都是实现中一定会撞墙的真实痛点。
3. **准备度百分比诚实** — `方向二沙箱 = 10%` 没有粉饰，这比泛泛说「需要做」有用得多。

---

## 与现有 27+ 份分析的关系

我快速比对了 `docs/analysis/expansion-next-frontier.md` 和 `expansion-core-five-2026-07-01.md`，结论如下：

| 你的方向 | 已有分析覆盖情况 | 你的增量价值 |
|---------|----------------|-------------|
| **一 · 跨厂商池** | 被 `expansion-next-frontier.md` 列为「已有覆盖，不再展开」 | **中等** — 你给出的 `parseClaudeCostUsd → provider-specific parser` 和 `claudeArgv → --model <resolved>` 两个具体改造点是前人没写的代码级路径 |
| **二 · 沙箱** | 同上，「已有覆盖」 | **低** — 沙箱是 ForgeOS 分析文档中重复率最高的方向之一（至少 6 篇涉及），你的边界分析没超出已有范围 |
| **三 · 语义检索** | 同上，「已有覆盖」 | **高** — 你指出了 `Gather` 的 query = `p.Name + " " + p.Agent` 导致 `implementer implementer` 的重复，以及 `memory.Query` 只做 exact-match 但 `Retrieve` 从未接入 memory——这两个**代码级的事实**之前的分析没抓住 |
| **四 · 跨 Workflow** | 同上，「已有覆盖」 | **高** — 你验证了 `nextStageLabel` 只渲染标签、`on_approve.next_stage=build` 不触发、`prompt_context.go` 无跨 workflow 注入——这是**反证架构图声称的脊柱不存在**，有价值 |
| **五 · 混沌工程** | 同上，「已有覆盖」 | **高** — 预算穿透分析（`SpentUsdMicros` re-seed 时序 + checkpoint 写入前崩溃的 double-charge 场景）是我在 27 份分析中第一次看到有人追到这个精度 |

**总结增量**：方向三/四/五提供了前人所没有的代码级发现，值得作为独立分析留存。

---

## 我看到的三个潜在盲区

### 盲区 1：方向一低估了 provider adapter 的工程量

你写的是：

> `claudeArgv` 的 `--model` 换成通用 `--model <resolved>`，`cost.go` 的 `parseClaudeCostUsd` 拆成 provider-specific parser

但问题不在模型名和 cost 解析——在 **prompt 格式**。当前 `buildPrompt()` 构造的 prompt 是 Claude 原生格式（XML 标签 + `\n\nHuman:` / `\n\nAssistant:` 角色标记），直接喂给 Gemini 会得到灾难性输出。你需要一个 `PromptAdapter` 接口：

```go
type PromptAdapter interface {
    SystemPrompt(string) string
    UserPrompt(string) string
    ParseResponse([]byte) (*AgentResult, error)
    ParseCost([]byte) (decimal.Decimal, error)
}
```

这不是「一行 ModelMap 配置」的改动，是 200-400 行新 Go 代码 + 跨 provider prompt 调优。

### 盲区 2：方向三的 embedding 冷启动比你想的更严重

你提到「新 repo 零 embedding → 退化为 TF-IDF」，但当前的 TF-IDF 实现（`retrieve.go`）没有 **document store**——它只搜 `adrTitles` 和 `p.Name` 两个极小的语料库。嵌入 memory/代码注释/issue 需要：

1. 一个可持久化的 **document store**（当前 retrieve 是无状态的）
2. 每次新 data 写入后的**增量索引**（Go 侧的 embedding 模型推理）
3. 跨 session 的**索引版本管理**（embedding 模型升级后旧向量失效）

这个「50% 准备度」可能需要下调到 20-25%。

### 盲区 3：缺失一个方向——**自举 / Dogfooding**

你排除了「需外部资源」的方向，但 ForgeOS **自身**就是最好的测试对象。`docs/analysis/self-testing-and-dogfooding.md` 已经论证了：如果 ForgeOS 不能用 ForgeOS 来 evolve 自己，那「24h 自治」的 vision 对自身的适用性从未被验证。当前 `forge accept` 跑 211+ 测试，但这些测试是手写的——不是 agent 写的。

这个方向不需要外部资源，只需要在 CI 或夜间任务中加一条：

```
forge run build --workflow .agent/workflows/self-evolve.yml --budget 0.50
```

让它去修一个真实的、已知的 tech debt（比如 `cmd/forge` 主文件膨胀——`growth-bottlenecks-and-scalability.md` 已经论证了这是真实问题）。如果连自己的 tech debt 都修不了，那「修用户的」的置信度打折。

---

## 收敛建议的收敛建议

你建议「做前三件：方向一+三+四」。我建议**重新排序**：

| 排序 | 方向 | 理由 |
|-----|------|------|
| **P0-first** | 方向三（语义检索） | 投入最小（Retrieve 接口就绪 + 加 embedder），但每个 phase 都受益——上下文质量是 agent 输出的**乘数因子** |
| **P0-second** | 方向四（跨 Workflow） | 脊柱自动串联是 vision 承诺的核心——没有它，discover→design→build 的人肉接力是系统最脆弱的环节 |
| **P1** | 方向一（跨厂商） | 价值大但**依赖方向三的 adapter 思维**——你需要的 `PromptAdapter` 和 per-provider prompt 调优，与 embedding 的 `DocumentStore` 抽象在架构上是同一件事（provider 抽象层） |
| **P2** | 方向五（混沌） | 同意你的判断——成本最低、揭示的问题最真实。建议在 `--parallel` 默认开启**之前**跑一次 |
| **P3** | 方向二（沙箱） | 现有 `allowedTools` + `FORGE_AGENT_DEPTH` + `maxOutputBytes` 三项防护对于当前 threat model 足够。沙箱在首个 `production` lifecycle 之前做就行，不必现在 |

关键洞察：**方向三的 `PromptAdapter` 和方向一的 `Provider` 抽象是同一个接口的两面**——应该一起设计，分开实现。先做 adapter interface（方向三/四的交集），再做第二个 provider 实现（方向一）。

---

## 一个具体的下一步建议

你的分析文件 `next-horizons.md` 已经存在。我建议：

1. **保留**方向三/四/五中新增的代码级发现（你在前人基础上的增量部分）
2. **补充**上述三个盲区的修正（PromptAdapter 复杂度、embedding 准备度下调、自举方向）
3. 在文件结尾加一个 **「与已有 27 份分析的关系」** 的 self-map 表（类似 `expansion-next-frontier.md` 开头那张表），让读者一眼看出哪些是增量、哪些是确认

需要我帮你做这三件事吗？
