好的，我已经完成了**全盘交叉验证**——逐行比对了文档中每个 `file:line` 引用对应的当前代码（HEAD `b0c80e4`）。

---

## 独立验证报告

### 准确度总评

| 方向 | 文档准确度 | 关键发现 |
|------|:----------:|---------|
| ① 跨仓库工作区 | **✅ 正确** | `gate.RepoRoot()` 确实单路径；`memory.go` 的 `sync.Map` 跨项目冲突注释确实存在 |
| ② Agent 质量断言 | **❌ 严重错误** | 3/3 核心字段的"未实现"断言全部与代码不符 |
| ③ 可观测性导出 | **✅ 正确** | 全文搜索 otel/prometheus/webhook 零命中；仅 `.forge/trace.jsonl` |
| ④ 分布式沙箱池 | **✅ 正确** | `SandboxConfig` 结构体定义存在但 `Execute()` 从不消费它 |
| ⑤ 跨工作流 DAG | **✅ 正确** | `NextStage` 仅用于日志显示 (`main.go:433`)，无自动化触发 |

---

### ❌ 方向② 的 3 个事实性错误

文档引用 `asset.go` 的注释说这些字段"nothing reads/enforces it yet"，但 **注释本身已经过时了**——消费代码早已落地：

| 字段 | 文档说法 | 实际代码 |
|------|---------|---------|
| `RequiresTools` | "nothing in forge-core reads it yet — no degrade-to-advisory check" | **已实现**：`prompt_context.go:424-452` — 工具不可用时注入 `degrading to advisory` 上下文标记 + `contextMarker`。有单元测试 `prompt_context_test.go:396-455` |
| `Readonly` | "nothing in forge-core enforces it yet — no write-blocking, no violation check" | **已实现**：`engine_build.go:161-178` (`readonlyToolScope`) — 生成 `--disallowedTools "Edit Write"` + 路径范围 `--allowedTools` 重开，通过 claude CLI 权限系统真正阻断写入。有单元测试 `engine_build_test.go:97-164` |
| `SecondaryTemplate` | "nothing in forge-core reads or injects it yet" | **已实现**：`prompt_artifacts.go:92-100` (`secondaryTemplateContext`) — 读取第二个 review 维度模板并注入 prompt 上下文。通过 `appendArtifactContext` (line 157) 接入 |

**根因**：文档作者读取了 `asset.go:147-156` 的 `ADDED HERE ONLY` 注释，但没搜消费端代码就下结论。这些注释没及时更新，产生了误导。

**真正的缺口还剩下什么**（文档没覆盖到的）：
- 没有"输出章节完整性断言"——验证 agent 输出是否包含了模板要求的全部章节
- 没有"自我报告与代码 diff 交叉验证"——implementer 声称做了 X 但 diff 没有
- 没有"跨阶段一致性断言"——Reviewer 的发现与 Implementer 的代码变更之间的逻辑闭环

这些才是文档方向②的**真正剩余价值**——不是那三个字段（它们已经实现了），而是更高层的语义断言层。

---

### ✅ 确认正确的 4 个方向

**① 跨仓库工作区** — 代码证据扎实：
- `gate.RepoRoot()` 返回单路径 (`gate.go:50`)
- `memory.go:61-65` 注释承认 `sync.Map` "concurrent forge processes on different projects"
- `.forge/` 运行时目录无工作区级概念
- 产品价值高：任何多仓库产品都需要

**③ 可观测性导出** — 缺口真实：
- trace 只写入 `.forge/trace.jsonl`
- 全文搜索 `otel/prometheus/webhook/event_bus/EventSink` 零结果
- `LoopEngine` 的 `OnIteration`/`OnPhase` 回调仅用于 checkpoint，从不向外推送

**④ 分布式沙箱池** — 结构体存在但零消费：
- `SandboxConfig{Type, Image, MemoryMB, TimeoutSec}` 定义完整但 `Execute()` 方法从不检查 `Sandbox` 字段
- 注释自己说是 "v1 placeholder skeleton"
- 所有 gate 全本地子进程运行

**⑤ 跨工作流 DAG** — `NextStage` 是只读标签：
- `main.go:433-436` 只渲染日志 `"next_stage=build"`，不自动触发
- 无 `pipeline` 子命令、无管线 DSL、无跨工作流状态持久化
- `on_unmet` 是阶段内重定向，不是跨工作流的

---

### 最大信息增量

文档的**真正洞察**不是那三个已实现的字段，而是：

> **结构闸门是必要条件，不是充分条件。** ForgeOS 缺少 Agent 输出的**语义质量断言层**。

这句话是方向②的核心命题，即使三个字段的实现细节错了，这个命题本身依然成立。闸门全绿 ≠ Agent 输出质量好。

---

你的意图是什么？我可以：
1. **修复方向②的分析** — 重写那一节，修正错误引用，保留真正的缺口（输出完整性断言、跨阶段一致性）
2. **更新 `asset.go` 的陈旧注释** — 把 `ADDED HERE ONLY` 改成 `IMPLEMENTED`，避免下一个分析者再被误导
3. **更新 `.out.md`** — 把我的独立验证写进去覆盖已有版本
4. **直接选取一个方向进入实现阶段**（比如③可观测性导出的 `EventSink` 接口抽取，或者⑤的 `forge pipeline` 子命令骨架）
5. **其他**
