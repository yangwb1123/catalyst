现在我已全面阅毕。让我以架构师 / 产品经理的视角做评审反馈。

---

# 架构评审: `strategic-expansion-directions.md`

## 总体评价

这是一份**高质量的分析产物**——代码锚点精确、北极星对标清晰、"为什么不是 v3 再做"的论证有说服力。但它在 15 份已有分析的基础上做了**优先级重排**,其中部分调整成立,部分值得商榷。

## 一、本分析相较已有产物的增量贡献

现有 15+ 份分析 (~40 方向) 中,本文的**真正增量**:

| 发现 | 此前分析覆盖 | 本文新贡献 |
|------|------------|-----------|
| **Sandbox P1 路径: git worktree** | expansion-deep-analysis §3 建议 Docker, expansion-gaps-v7 §4 建议 gVisor | ✅ **第一个提出纯 Git 方案** —— 0 新依赖, 2 周, 且不违背零依赖红线 |
| **Discover → converge.Signals 数据流溯源** | 无人追踪过这条链路 | ✅ 明确标识 `RequirementConfidence` 字段已定义但信号源为空 |
| **事件流 API (`forge events --watch`)** | expansion-high-value §5 建议 WebSocket 仪表盘 | ✅ 更轻量的 SSE 方案 |
| **优先级重排: Sandbox P0, Discover P1, Knowledge P2** | 此前分析多将 Knowledge Engine 放在 P0/P1 | ✅ **新的权衡判断** |

其余 3 个方向(跨厂商路由/仪表盘/知识引擎)与 **expansion-deep-analysis §2/§5** 和 **expansion-gaps-v7 §1** 高度重叠。

## 二、优先级矩阵的校准意见

### 🔴 方向① Sandbox P0 — **同意,但需要一个补充**

`git worktree add` 方案确实优雅,但有**一个隐含前提**:目标仓库必须使用 Git。对于:

- 非 Git 工作目录 (`forge run --dir /tmp/scratch`)
- Git 但不合作的工作树 (`git worktree add` 需要在同一文件系统,且后续 `worktree remove` 清理)
- 已经 dirty 的工作目录

这些 edge cases 需要 fallback。建议 P1 实现增加**白名单/黑名单路径策略**:

```
SandboxConfig {
    Strategy: "worktree" | "copy" | "none"
    AllowedPaths: []string  // agent 可读路径白名单
    DeniedPaths:  []string  // ~/.ssh, /etc, 等
    NetworkAccess: bool     // 默认 false
}
```

`copy` 模式 (`cp -r` + `mktemp`) 作为 worktree 不可用时的降级——成本稍高但无需 Git 依赖。

### 🟡 方向③ 跨厂商路由 P1 — **建议降为 P2,理由如下**

本文的论证缺口:

1. **用户价值在当前模式不成立**:ForgeOS 当前是 `claude -p` (print-mode) 单次调用。跨厂商收益在**多轮 tool-use 循环**中才显著——而那是 Agent-Runtime 执行层(尚未实现)的事。在 print-mode 下换 GPT-4o 只是换一个同样做单次输出的模型,差异化不大。

2. **估计偏乐观**:LiteLLM 集成"~300 行 Go HTTP"——但每家厂商的:
   - 认证方式不同 (Anthropic: x-api-key, OpenAI: Bearer, Google: OAuth)
   - 错误响应格式不同
   - Rate limit 反馈不同 (429 的 Retry-After 格式不一)
   - 流式 vs 非流式行为不同
   
   实际工作量预计 600-900 行,且需要每个厂商的集成测试。

3. **零依赖约束冲突**:走 HTTP 需要 `net/http` (stdlib, 可接受),但 JSON 反序列化不同厂商的响应结构差异需要大量的类型定义。这不是架构风险,但确实不是"2 周"的事。

**建议**:将 LiteLLM 集成放到 P2,但**模型目录 YAML + 性价比评分逻辑**可以现在启动(不影响零依赖,纯声明式数据)。

### 🟡 方向④ 仪表盘 P1 — **同意事件流 API,暂缓静态仪表盘**

`forge events --watch` 是**高价值低风险**的 ~100 行改动。但"单页 HTML 无构建步骤"低估了:

- 需要从 CLI 二进制中嵌入/分发静态文件
- JSONL 文件在运行中频繁写入,HTML 页面读文件会有竞态
- 在浏览器中展示 trace 数据的 JS 逻辑(树形展开/时间线/成本图)远非"1 个 HTML 文件"

**建议拆分**:

```
P1 (1周): forge events / forge events --watch  →  CLI 实时流
P2 (2-3周): .forge/dashboard.html  →  极简静态仪表盘
P3 (暂缓): 审批端点 →  依赖人审流程的成熟
```

### 🟢 方向⑤ 知识引擎 P2 — **同意,但补充一个时间触发条件**

当项目 ADR 数量 > 20 或 memory 条目 > 200 时,**当前精确匹配的检索方式会开始明显降级**。建议将此作为触发条件而不是固定时间表:

```
WHEN (len(adrs) > 20 OR len(memory_entries) > 200) → P2 知识引擎自动提升为 P1
```

## 三、本文未覆盖的重要方向

### 盲点 1: YAML shim 违反零依赖承诺 (建议 P1)

```bash
# 当前: forge run/evolve 每次 shell 出 python3
python3 harness/yaml2json.py < .agent/workflows/discover.yml
```

本文完全没有提及。但 **expansion-strategic-v5.md §3** 将其标为 **P1**:

- 每次 workflow 加载增加 ~100ms 延迟
- 跨平台 Python 可用性不可靠 (Windows/MinGW/Alpine 可能无 `python3`)
- `pyyaml` 无版本锁定,供应链风险

**建议**: 将 Go YAML 解析器加入 `forge-core/internal/yaml/` 作为 P1 技术债务。Go stdlib 无 YAML 解析,所以这需要第一个(也是唯一一个)例外:一个嵌入式的纯 Go YAML 解析器(如 `gopkg.in/yaml.v3` 的 vendor 或自己实现最小子集)。或者,接受这个脚手架,明确记录为"已知技术债务"。

### 盲点 2: 收敛自报告偏移 (建议 P1 修复)

`converge.go` 的 `roadMapCompletion` 信号来自 agent 的自我评估。这是一个**系统性风险**——当前代码中没有独立验证:

```
roadMapCompletion = agent 说 "我完成了 85%"
                   ↓
converge 判断 → MET (roadMapCompletion ≥ ROADMAP 100%)
```

如果 agent 高估完成度(已多次在 dogfood 中观察到),converge 会提前判定为 MET,跳过未完成的工作。**这是当前架构中最危险的自我报告漏洞**。

修复方案(~100 行):`forge gate --roadmap` 作为独立验证器,从 `.agent/ROADMAP.md` 解析未完成项,与当前代码库对比给出独立完成度估算,与 agent 自报告交叉验证。

### 盲点 3: CI 缺少竞态检测和全构建 (建议 P2)

```yaml
# .github/workflows/forge.yml
- name: forge-core tests
  run: go -C forge-core test ./...   # ← 无 -race, 无 -count=1
```

`go test -race` 在并发编排代码(`orchestrator/loop.go` 的 goroutine + channel 模式)中不是可选质量属性——**它是正确性保证**。竞态在单次运行时可能隐藏、在压力下爆炸。

## 四、优先级矩阵修正建议

| 本文件排序 | 建议排序 | 方向 | 修正理由 |
|-----------|---------|------|---------|
| P0 | **P0** | ① Sandbox P1 (git worktree) | 同意 |
| — | **P1** | *(新增)* YAML shim 替换或明确记录为技术债务 | 零依赖红线的完整性 |
| — | **P1** | *(新增)* 收敛独立验证 | 当前最危险的系统性漏洞 |
| P1 | **P1** | ④ 事件流 API (`forge events`) | 同意,但拆出仪表盘 |
| P1 | **P1** | ② Discover 引擎 | 同意,但估计 6-8 周而非 4-6 周 |
| P1 | **P2** | ③ 跨厂商路由 | print-mode 下价值不充分 |
| P1 | **P2** | ④ 静态仪表盘 + 审批端点 | 嵌入/竞态/JS 复杂度低估 |
| P2 | **P2** | ⑤ 知识引擎 | 同意,加触发条件 `adr>20 ∥ memory>200` |
| — | **P2** | *(新增)* CI -race + 全构建 | 基础设施健康 |
| — | **P3** | ③ LiteLLM 网关 | 等 Agent-Runtime 执行层就绪 |

## 五、一个高层的架构观察

本文的 5 个方向涵盖**安全/发现/路由/观测/知识**——全是 forge-core 之上或之外的能力。但有一个**更深层的架构缺口未被触及**:当前的工作流执行模型是**同步线性**的:

```
Phase 1 → Phase 2 → Phase 3 → ... → Converge Evaluation
```

它在单机单 executor 模式下够用,但**当你想让一个 agent 做调研、另一个同时开始实现一个独立模块时,当前架构没有并发的概念**——`RunParallel` 是 resource-level 的并行(同一个 YAML 定义的多 resource 并行执行),不是 workflow-level 的并行(多个独立不冲突的 phase 同时跑)。

这不是"v3 再做"的事——**Discover 引擎一旦上线,requirement-discovery + market-research 天然可并行**。建议在 Discover prompt 骨架的同时,花少量时间设计 `parallel_phases` 的 YAML schema 扩展和 `PhaseGraph` 的 DAG 解析器,否则 Discover 引擎上线后会被迫串行执行本可并行的阶段。

---

## 总结

| 维度 | 评价 |
|------|------|
| 代码锚点精确度 | ⭐⭐⭐⭐⭐ 卓越 — 每个断言都有 Go struct/行号支撑 |
| 北极星对齐 | ⭐⭐⭐⭐ 好 — 15 服务目录的对标有说服力 |
| 相对已有分析的增量 | ⭐⭐⭐ 中 — 3/5 方向有深度重叠,2/5 有真正新发现 |
| 工作量估计 | ⭐⭐⭐ 中 — 跨厂商路由和仪表盘偏低估 |
| 盲点覆盖 | ⭐⭐ 需要补充 — YAML shim / 收敛自报告 / CI 安全 |
| 优先级判断 | ⭐⭐⭐⭐ 总体合理 — Sandbox P0 的判断尤其正确 |

**最终建议**:将此文件另存为 `docs/expansion-strategic-v16-with-review.md`(包含本评审追加的三个盲点方向),用作 Sprint 27 规划输入。Sandbox P1 + 事件流 API + YAML shim 决策应列在**Week 1-2 即时行动**而非"4 周内"。

---

*评审人: pi (架构 Agent) · 基线: forge-core v2 · 日期: 2026-07-01*
