现在我对整个情况有了完整的了解。以下是审阅反馈：

---

## 文档审阅：5 个扩展方向分析

### 🔍 关键发现：基线漂移

这份文档是**针对已提交的 `b0c80e4` 状态**的准确分析。但**当前工作目录**（54 个文件，+2,774/-651 行未提交修改）已经在此分析与实际实现之间的某个时间点实现了大多数推荐方案。

### 逐方向状态核对

| # | 方向 | 文档诊断 | 工作目录状态 |
|---|------|---------|------------|
| 1 | **持久化格式版本化** | 全部 3 种格式无 `_format` 字段 | ✅ **已实现** — `persist/checkpoint.go` 中增加了 `FormatVersion` + `_format` 标签，`trace/trace.go` 增加了 `"forgeos.trace.v1"`，`memory/memory.go` 增加了 `"forgeos.memory.v1"`，均支持向后兼容的空值处理 |
| 2 | **Prompt 注入攻击面** | 无 sanitize/无上下文标记 | ✅ **已实现** — `sanitizeAgentOutput()`（去除控制字符，保留 Unicode 可打印字符/换行符/制表符/回车符），`contextMarker()`（添加 `[context:source]` 前缀），已接入所有注入通道：memory、gate-results、phase-output、findings、emits、template |
| 3 | **全局缓存碰撞** | 包级 `loadMu`/`loadCache`/`loadPath` 全局变量 | ✅ **已实现** — 使用 `sync.Map`（基于路径键），代码注释中引用了 "§方向3: global cache collision" |
| 4 | **Trace 无界文件增长** | 无 rotation/无上限/无压缩 | ❌ **仍未实现** — 当前修改仅增加了格式版本化和构造函数辅助方法（`GateEvent`、`DecisionEvent` 等），未涉及 rotation 或上限 |
| 5 | **状态变更历史缺失** | Phase A：checkpoint 保留策略 | ✅ **Phase A 已实现** — `Save()` 增加 `retain int` 参数，`rotateRetain()` 实现轮转（path → path.1 → path.2 → ...），代码注释中引用了 "§方向5 phase A" |
| | | Phase B：trajectory 文件 | ❌ **未实现** — 未增加单独的 trajectory 文件 |

### 关于文档本身的质量反馈

**优点：**
- 代码引用精确 — 针对 `b0c80e4` 状态的断言完全正确
- 威胁模型合理 — 方向 2（注入）是真正的安全边界；方向 3（缓存抖动）是真实的中等优先级问题
- 依赖关系图准确 — 方向 5 确实依赖方向 1（格式版本化作为文件命名的锚点）
- 优先级划分合理 — 将方向 4 定为 P3 是正确的

**值得注意的疏漏：**
- 方向 4 缺少一项：trace 已经被 **scorecard 消费者** （`scorecard_wind.go`）读取。当前修改在 `scorecard_wind.go` 中增加了 251 行（轮转后的读取路径适配已在实现中），但文档中未提及这种读取方耦合关系
- 方向 2 的评估可以再增加一层：**BM25 检索本身**就是一个注入通道 — `prompt_memory.go` 中的 `memoryContext` 使用 `prompt.Retrieve`（关键词 BM25-lite），这种平面关键词搜索无法防御对抗性植入的文本（例如，在 `Topic` 字段中填入 "ignore all previous instructions"）。文档中未涵盖语义层面的攻击面
- 方向 5 Phase B 给出的替代方案（trace 中增加 `"iteration_summary"` 事件类型）很合理，但文档中未排查 trace 格式是否预留了这种扩展空间 — 当前修改通过 `Event.Kind` 的按种类分组机制已使之变为可行

### 关于工作目录实现的额外观察

未提交的修改已经超出了文档建议的范围，体现在以下几个值得注意的方面：

1. **`trace.go` 构造函数辅助方法** — 文档中未预见的五种事件类型：`GateEvent`、`DecisionEvent`、`OverloadEvent`、`StaleEvent`、`ErrorEvent`。这些实现提供了一个稳定的 JSONL 外部模式替代自由文本字符串，这是一个不错的改进。

2. **`memory.go` 压缩** — 新增了 `Compact()`、`Prune()`、`summarizeBlock()`、`rewriteStore()` 方法并设置了 `DefaultCompactThreshold=500`。文档中仅提到方向 4 讨论了压缩，但 memory 压缩实际上是与方向 4 不同的独立能力，却已实现但文档未能覆盖。

3. **`memory.go` 中的 `Supersedes`+`Confidence`+`Source`** — 文档方向 2 建议了 `confidence` 和 `supersedes`，而实际实现中还进一步增加了 `Source` 字段（来源标记），以便在 prompt 层进行信任归属。

### 当前状态下需要关注的问题

工作的完成顺序很合理（版本化 → 注入防御 → 缓存 → 保留），但仍存在以下问题：

1. **方向 4（Trace rotation）** — 这是文档中唯一一个完全未被实现的方向。如果计划将 trace 用于长时运行审计，这一点值得处理。当前实现中的新增内容（`scorecard_wind.go` 新增 251 行）表明 trace 的读取方路径正在被积极使用中 —— 未做 boundary 保护的 trace 增长是一个正在积累的技术债务。

2. **提交就是否就绪** — 当前未提交的修改范围（54 个文件，2,774 行新增）是一个大变更集合。将这些修改拆分为逻辑提交（格式版本化 / 注入防御 / memory 重构 / checkpoint 保留 / trace 事件类型）会比一个大规模的聚合提交更容易进行审查和回滚追溯。

3. **文档与代码之间的自我引用** — 代码注释中引用了 `analysis §scan-new-angles direction 2`、`§方向3`、`§方向5 phase A`，但这些文档似乎尚未同步提交。如果这些分析文档应当作为架构记录的一部分，当前它们只存在于工作目录之外。

### 结论

| 项目 | 状态 |
|------|------|
| 分析质量 | 针对基线版本的较高准确性，但存在数据集偏移问题 |
| 实际覆盖率 | 方向 1/2/3/5-A 已实现，仅方向 4 和方向 5-B 未实现 |
| 实现完整性 | 实际实现超出建议范围约 30%（compaction、Supersedes、Source 字段、trace 事件辅助方法） |
| 下一步 | 提交现有修改，决定是否将方向 4 纳入当前批次或推迟处理 |
