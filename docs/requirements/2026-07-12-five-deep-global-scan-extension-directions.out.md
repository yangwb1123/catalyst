Now I have a thorough, validated understanding of the codebase. Here is my full review.

---

## 审查反馈：ForgeOS 全局扫描扩展方向文档

### 总体评价

**质量很高**。五个方向都有扎实的代码级证据支撑，引用行号（asset.go:225、memory.go:327-349、parallel.go:33-42、backoff.go:61 等）全部经我交叉核验为**准确**。五个方向与已有 ~60 个方向确实**不重复**——我在 `docs/requirements/` 的 ~300 个文件和 `docs/analysis/` 的 ~30 个文件中验证过，没有任何篇提出完全相同的问题集。

但有**5 处事实错误 + 3 处论点弱化 + 1 个重大遗漏**需要修正。

---

### 一、事实错误（必须修改）

#### 错误 1：方向一「`converge.go` 的 `Converge` 函数对 `on_approved` 仅返回 met/unmet」

**原文**：
> `converge.go` 的 `Converge` 函数对 `on_approved` 仅返回 met/unmet，从不触发下一阶段。

**问题描述准确，但**引用的函数名称和边界不准确。`Converge` 函数的职责就是「检查收敛条件是否满足」——它只是一个纯函数判断，**从来不被设计为**触发下一阶段。真正该被点名的是 `cmd/forge/main.go:297` 的 `runWorkflow` 和 `cmd/forge/evolve.go:162` 的 `execLoop`——它们调用了 `converge.Converge` 但只拿结果做报告，不根据 `on_approved.next_stage` 做阶段路由。

**建议修改**：把责任方从 `converge.go` 改为 `cmd/forge/main.go` / `cmd/forge/evolve.go`。

#### 错误 2：方向一「phase 复用模式已建立：`LoopEngine.Run` 已经演示了如何驱动一个多迭代收敛循环」

**代码证据**：`LoopEngine.Run`（`loop.go:145`）驱动的是**单 workflow 的多迭代收敛**（evolve 的迭代循环），不是**多 workflow 的编排**。它和 `Engine.Run`（`orchestrator.go:176`）在同一层抽象，没有「嵌套复用」关系。

**建议修改**：改为「`LoopEngine.Run` 演示了跨迭代收敛循环模式（iteration-level 的外层循环），workflow 组合可看作 *workflow-level* 的外层循环——相同模式在上一层的复用。」

#### 错误 3：方向二「`memory.Load` 返回 (nil, nil)——从零开始」

**代码证据**：`memory.go:229-248`：
```go
func Load(path string) ([]Entry, error) {
    // ...
    data, err := os.ReadFile(path)
    if err != nil {
        if errors.Is(err, fs.ErrNotExist) {
            storeToCache(path, nil, nil)
            return nil, nil  // ← 确实返回 (nil, nil)
        }
    }
```

这是正确的，但**上下文缺失**：`cmd/forge/evolve.go:68` 的 `rejectHumanGate` 说明 `forge evolve` **禁止在 human_gate workflow 上运行**。而 `forge run` 是单次调用——每次启动确实从零开始。但 `forge evolve --resume`（`evolve.go:55`）会加载 `checkpoint.json`（`persist/checkpoint.go`），它包含迭代状态。Memory 确实丢失——但 checkpoint 保留了跨迭代的 `LoopState`。

**建议修改**：补充 nuance——`checkpoint.json`（`internal/persist/checkpoint.go`）已经在做 **部分** 跨迭代状态恢复（迭代轮次、已完成的 phase 列表、budget 余额），但**不包含 memory Entry**。所以 memory 的跨运行丢失是真实的，但应说明「不是所有状态都丢失→checkpoint 已做了一部分，memory 是缺口」。

#### 错误 4：方向三「`prompt_context.go` 的 `injectPhaseOutputs` 路径」

**代码证据**：`grep` 在 `prompt_context.go` 中未匹配到 `injectPhaseOutputs` 或 `phaseOutput`。这个函数**不存在**。`parallel.go:21` 提到了 `phaseOutputLedger`，但那是 phase 的 `feeds_forward` 信号，不是完整的 output 注入。

**建议修改**：删除或修正此引用——`feeds_forward` 是已有的但粒度不同的模式（布尔标记 vs 文件级契约验证）。

#### 错误 5：方向五「`cmd/forge` 新子命令 `forge diagnose`」

**方向命名本身没有问题**，但 `forge scorecard rebuild --from <trace.jsonl>`（`scorecard_wind.go:272`）**已经做了一个 trace 的聚合读取**——它读 trace.jsonl 产出 scorecards.json。所以「trace 数据从不聚合查询」不完全准确。

**建议修改**：改为「trace 数据**仅被 scorecard 子系统**读一次用于模型路由统计，失败模式分析从未实现」。

---

### 二、论点弱化（需要补充）

#### 弱化 1：方向一——信号传递的断裂程度

**原文**：feeds_forward 只在 *phase 之间*工作，不在 *workflow 之间*。

**正确但不够准确**。`feeds_forward`（`asset.go:61-63` + `asset.go:152`）是一个布尔标记，**不包含具体的输出内容**。它的实际机制是 `phaseOutputLedger`（`parallel.go:21`）把标记为 `feeds_forward: true` 的 phase 的产出路径注入到提示词上下文。这个机制在**同一 workflow 内**的 phase 间工作，但不在 workflow 间。

建议补充一个可操作的改进路径：同一个 `phaseOutputLedger` 可以扩展为支持跨 workflow——只需在 `memory.jsonl` 或新 `transition.json` 中记录「design workflow approved → emits ARCHITECTURE.md」，在 `build` workflow 启动时以 `--load-memory` 注入。

#### 弱化 2：方向五——backoff 参数全局 hardcode

**原文**：backoff 参数（`overloadBackoffBase=2s`、`overloadBackoffCap=60s`、`Engine.MaxRetries`）是**全局 hardcode**（`backoff.go` 第 56-61 行）。

`overloadBackoffBase` 和 `overloadBackoffCap` 确实是全局包级变量（`backoff.go:70-71`），但 `Engine.MaxRetries`（`orchestrator.go:90+`）是**Engine 结构体的字段**——`Engine` 是由 `buildRunEngine`（`engine_build.go:230`）从 `--max-retries` flag 和 mode policy 综合构造的，**不是 hardcode**。

建议修正：`overloadBackoffBase` 和 `overloadBackoffCap` 是 hardcode，`MaxRetries` 不是。

#### 弱化 3：方向三——Emits 只是字符串数组

**原文**：Emits 只是**字符串数组**——没有说明「这个文件应该长什么样」。

**代码证据**：`asset.go:155`：
```go
Emits []string `json:"emits,omitempty"`
```

正确。但应补充：`writes_adr`（`asset.go:51-55` + `asset.go:151` + `asset.go:164-170`）已经是一个**结构化声明模式**（有 `condition` 和 `target` 字段），可以作为 Emits 契约扩展的模板。Emits 目前只是路径列表，可以扩展为 `Emits []EmitSpec` 其中 `EmitSpec` 有 `path` + `schema_ref` + `required_sections` 等字段。

---

### 三、重大遗漏

**没有提到全方向共通的「可观测性缺口」**：当前五个方向都依赖或建议某种形式的跨进程/跨运行状态共享（组合引擎的信号传递、knowledge lifecycle 的跨运行继承、contract 验证、resource governance 的监控反馈、失败智能的 trace 聚合），但没有一个方向讨论了**这些共享状态本身的正确性保证**——它们都是文件系统 JSONL，在并发/部分写入/断电下面临你之前审计的 `TOCTOU` 和 `无声数据丢失` 问题。建议在方向一或二的「关键设计边界」段补充一个跨方向的风险说明。

---

### 四、优先级和依赖建议

| 方向 | 建议优先级 | 调整理由 |
|------|-----------|---------|
| 方向一：Workflow Composition Engine | **P1** → **P0** | 这是从「CLI 工具」到「自治平台」的**架构跳跃点**。当前 `forge evolve` 只跑单 workflow，`forge run` 也只跑单 workflow。方向一直接解锁两端串接。且已有 `on_approved.next_stage` 声明 + `.forge/<stage>.approved` 标记基础设施，增量路径极短。 |
| 方向四：并行资源治理 | **P1** | 同意 P1。`--parallel` 已经 merge，但无 governance 就是炸弹——100 个 claude 进程同时启动的经济风险是真实的。 |
| 方向三：Phase 输出契约验证 | **P2** → **P2.5** | 第一步（文件存在性检查）增量成本极低，但收益在真 agent 场景才显现。可以先做文件存在性检查，语义验证推迟。 |
| 方向二：跨运行知识生命周期 | **P2** | P2 合理，但和方向一的组合引擎有**依赖关系**——没有跨运行继承，组合引擎就不能有效传递 design 的架构决策到 build。建议在方向一的 v2 实现中捆绑最小 memory 继承。 |
| 方向五：失败智能与自动修复 | **P2** | P2 合理，但 `forge diagnose` 命令是**纯增值**，不 blocking 任何其他方向。 |

---

### 五、全局质量评分

| 维度 | 评分 | 说明 |
|------|------|------|
| 代码准确性 | 8/10 | 5 个事实错误，但核心论点全部正确 |
| 已有方向零覆盖 | 10/10 | 已核验，确实不重复 |
| 增量路径清晰度 | 9/10 | 每个方向都有 vs 目标态的边界描述 |
| 设计边界纪律 | 9/10 | 「不引入外部依赖」「不改变串行路径」「advisory 非自动执行」等边界很好 |
| 产品价值论证 | 8/10 | 每个方向的「为什么需要」都很扎实 |

**总体结论**：**高质量高价值文档**，修正上述 5 处事实错误 + 补充 3 处弱化后即可定稿。五个方向中，方向一和方向四有最清晰的增量路径和最高的即时 ROI，建议作为 Sprint 32/33 的核心工作项。
