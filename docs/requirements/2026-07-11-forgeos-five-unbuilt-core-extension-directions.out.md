Good — I've now read the actual code at the claimed locations and can give you a grounded, honest assessment of the five directions.

---

## 我的验证结论

### ✅ 方向一・Prompt Token 预算管理 — 完全成立

代码证据精确。`prompt.go:Build()` 和 `prompt_context.go:buildPromptWithEmits` 确实在拼接完所有 lane 后**没有任何总大小检查**。`memoryCap=32` (`prompt_memory.go:48`) 是个固定计数器而非 token 预算,`taskCap=4000` 只卡 ROADMAP 单条。没有 `EstimateTokens`、没有模型感知、没有降级逻辑。

**最危险的静默劣化路径**:`memoryCap=32` 条 × 假设每条 ~200 token = ~6.4K(已超 Haiku 8K 的四分之三),加上 ROADMAP(4K runes ≈ 1K token) + ADRs(6 条 × ~500 token) + AGENTS.md + gate results + phase outputs → 很容易在 Haiku phase 中静默溢出。P1 正确。

### ✅ 方向二・语义验证管线 — 完全成立

`build.yml:62-63` 的 `required_gates` 确实全部是形式门禁。`harness/` 中所有 gate 检查的是**代码的样子**,不是**代码是否工作**。没有 `compile` gate。

一个边界我补充:现有 `test` gate 通过 `node --test` 运行测试,但**只跑已有的测试文件**,不检查新代码是否被测试覆盖。因此 agent 可以写出不编译的新函数——只要没被任何已有测试调用,`test` gate 仍然 PASS。

### ✅ 方向三・确定性重放 — 完全成立

`trace.go` 的 `Event` 结构体确实没有 `PromptText` 字段。`checkpoint.go:59-63` 只存 `PhaseIndex`,不含 memory 快照或 prompt 快照。

建议中**Replay bundle 只存最近 5 轮**是合理的成本约束。不过注意:每条 memory entry 的 `Detail` 字段可能很大(由 agent 自由文本写入),5 轮 × 32 条 × ~500 token → ~80KB,加上 prompt 文本 ~10KB,远低于 1MB。

### ⚠️ 方向四・多厂商模型路由 — 需要修正

**文档的核心论点"系统无 Provider 抽象层"不再完全精确**。`routing.go:238-276` 已经存在 `ModelMap`、`ResolveModel()`、`Providers()`:

```go
// routing.go:238-248
var ModelMap = map[string]map[string]string{
    "anthropic": {
        Haiku:  "claude-sonnet-4-haiku",
        Sonnet: "claude-sonnet-4",
        Opus:   "claude-opus-4",
    },
}

// routing.go:256-268
func ResolveModel(provider, tier string) string { ... }

// routing.go:272-278
func Providers() []string { ... }
```

注释中明确标注了 `// Cross-vendor pool (v3 direction)`。所以方向的**核心命题——需要厂商无关的 Provider 抽象层——是对的,但文档忽略了已经存在的抽象骨架**。

**真正的 GAP 不在 `routing` 包,而在上层集成**:`engine_build.go:claudeArgv` 仍然硬编码 `--model` 参数,且没有读 `provider` 配置。`opusFloorAgents` 仍假设 provider=claude。所以方向调整为:**"Provider 抽象层已声明但未集成到 CLI/engine 层"**——这是 P2 正确但论据需要更新。

### ✅ 方向五・Memory 价值感知管理 — 完全成立

`memory.go:75-80` 的 `Confidence` 字段已存在,但 **只有一个消费者**:`prompt_memory.go:186-190` 用它来决定 `[unverified]` / `[low-confidence]` 前缀。没有排序、没有筛选、没有价值衰减。

`boundMemory` 的排序策略是 `(recency floor + relevance)`——完全不看 confidence。`Compact` (`memory_compact.go`) 按 `keepPerKind` 计数截断,不看价值。

最难的问题:Confidence 是 agent 自报告的,天然不可靠。文档的 `sourceWeight` 方案(implementer=0.5, reviewer=1.0)是合理的,但需要小心——reviewer 也可能出错。

---

## 总体评价

| 方向 | 精度 | 最重要的新洞察 |
|------|------|---------------|
| 1 | ✅ 精确 | Haiku phase 溢出路径是真实且可预测的 |
| 2 | ✅ 精确 | 补充:新代码不编译但 `test` 门仍可通过(因为测试不覆盖) |
| 3 | ✅ 精确 | 5 轮 ~80KB 成本估算合理 |
| 4 | ⚠️ 需修正 | `routing.go` 已有 `ModelMap`/`ResolveModel`——GAP 在 engine 集成层,不是 routing 层 |
| 5 | ✅ 精确 | Confidence 只有 2 个消费者(前缀显示),零个用于排序/筛选 |

**优先级建议不变**:Sprint N 同时做方向一 Phase A + 方向五 Phase A + 方向二 compile gate——三个加起来 ~600 行,各自独立,收益立即可见。

---

请问你希望我针对这五个方向做下一步什么工作？比如:
- 为某个方向写更详细的技术设计文档？
- 验证更多代码级 claim？
- 或者直接开始实现某个方向的 Phase A？
