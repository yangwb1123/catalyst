分析文档已写入 `docs/analysis/architectural-analysis-2026-07-12.md`。以下是核心结论的总结：

---

## 分析要点一览

### 对审计的评价

审计的三处核心纠正我都认可并纳入了分析：

1. **方向三（渐进式治理）**80%+ 被 v35 方向五覆盖 → 降级为 P3 增量扩展，合并到 v35
2. **方向二（Provider 契约）**架构描述过时 → 代码实际上已用 DI 模式，真正的耦合点在三个序列化边界（cost JSON 解析 / `isClaude` 字符串匹配 / `ResolveModel("anthropic")` 硬编码）
3. **方向一/二的差异化引用错误** → 在分析中修正了引用关系

### 我补充的架构洞察

**超越审计的发现**：

1. **接口应沿序列化边界而非调用边界设计**。当前最难解耦的点不在函数调用层（那里已经是 DI），而在数据格式契约层（各厂商的 cost JSON schema 不同）。这是架构层面的洞见。

2. **Golden file 的安全网价值被审计低估了**。审计将其评为「仅 50% 增量」，但分析指出——没有 golden file 安全网，就不应该重构 orchestrator 实现组合原语。所以方向四不是方向一的「平行项」，而是**必要条件**。

3. **发现了一个审计未明确指出的方向**：信号驱动的工作流装配。`risk.Classify` 目前只有一个消费者（model router），增加第二个消费者（workflow assembler）的边际成本很低。

### 推荐实施顺序

```
阶段一 (2-3 Sprint): Golden File 安全网     ← 使能后续重构
阶段二 (3-4 Sprint): 工作流组合原语         ← 最高杠杆方向
阶段三 (2-3 Sprint): Provider 解耦 + 信号装配 ← v3 前提条件
阶段四 (1-2 Sprint): 治理采纳增量           ← 合并到 v35 方向五
```

### 不动的东西

- **不引入通用工作流语言**（Temporal/BPMN）— v3 再评估
- **不创建完整 Provider 抽象层**— 只解耦三个具体耦合点
- **不引入 OPA/Rego**— 当前 host-independent harness 已够
- **保持 forge-core 零外部依赖**— 所有新增接口（CostParser、Provider 类型、组合器、golden 采集器）都在纯标准库内实现
