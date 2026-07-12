现在我已读取所有关键引用点。让我给出架构评审回应。

---

## 架构评审回应：五方向扩展分析

**评审人角色**: 资深架构师（forge-core 全局通读完毕）  
**日期**: 2026-07-12  
**基调**: 严苛但建设性——只挑真实问题，不吹毛求疵

---

### 总体评价

这是一份**高密度、诚实、代码级有根有据**的分析。五个方向没有一个是我能轻易反驳的——每一处的 `file:line` 引用都准确，代码行为描述与源码一致。这与当前仓内 163+ 份已有分析文档的**差异定位**也成立：确实没有一份从「产物信任基线」和「并行竞争安全」的交叉角度切入过。

但我通读源码后有 **4 个修正 / 3 个补充 / 2 个优先级调整建议**，必须坦诚指出。

---

### 修正（事实级）

#### 修正 1：方向一——平行模式当前实际无人使用（方向一自身也隐含，但优先级论证未充分展开）

你引用 `parallel.go:153-169` 说并行冲突是真实风险。完全同意代码路径存在。但读 `parallel.go:13-16` 的文件头：

```go
// It is reached ONLY when the operator passes --parallel AND the workflow
// declares depends_on; every existing workflow (no depends_on, or no
// --parallel) keeps using RunFrom byte-for-byte.
```

当前仓内**全部 5 个 workflow YAML** 没有一个声明 `depends_on`。所以：
- `RunParallel` 是代码可用但**运行时零调用**的路径
- 并行文件冲突是**真实代码路径但零生产风险**的状态

这**不否定方向一的价值**——当第一个声明 `depends_on` 的 workflow 出现时，问题立刻变为真实。但优先级论证中「parallel mode 已就绪但 zero 使用，最大阻碍就是文件冲突无处理」把因果颠倒了：**parallel mode zero 使用的真正原因是 workflow 未声明 depends_on**，而非文件冲突处理缺失。

**影响**: P1 成立但论据需微调——方向一是「pre-condition for parallel mode adoption」（先修条件），不是「unblock zero-usage bottleneck」。

#### 修正 2：方向三——`memory.jsonl` 并非真正完全的「所有 run 共享」

你写：

```
// ← 全局 `memory.jsonl`，所有 run 共享
```

实际代码中 `memory.go:172-197` 的 `Append` 函数接收 `path string` 参数，不是硬编码的全局路径。真正「全局共享」的是 `cmd/forge/evolve.go:168-175` 传入的 `memoryPath(o.root)`——它是**项目级**的 `.forge/memory.jsonl`。

这意味着：
- **同一仓库的多个 evolve 进程**确实共享同一 `memory.jsonl` → 你的知识污染论证成立
- **不同仓库**的 evolve 互相不影响 → 不构成跨项目问题

更重要的是：`memory.go:39-42` 的 `loadCaches sync.Map` 是**进程级 per-path 缓存**。`sync.Map` 的注释写明了 `so concurrent forge processes on different projects do not invalidate each other's cache entries`——说明设计者已知跨进程问题。但 `sync.Map` 跨进程不共享，所以缓存层面不构成问题，**问题只在文件层面**。

**影响**: 核心论证不变，但问题范围从「全局共享」缩窄为「单仓库多进程竞争」。

#### 修正 3：方向五——reviewer mode-skipped + loop-back 死循环的例子当前不可达

你写的边界情况：

```
reviewer 被 mode=explorer 跳过，但 gates phase 的 on_fail 声明
target_phase: reviewer → loop-back 跳到一个被跳过的 phase
```

我读了 `mode_gating.go:98-103`：

```go
func (e Engine) skipByMode(p asset.Phase, stage string) bool {
    if requiredWhenKey(p.RequiredWhen) == "reviewer" && !e.ModePolicy.Reviewer {
        return true
    }
```

当前模式门控只跳过 **agent phase**（`RequiredWhen: "reviewer"`），**gate phase** 不被 `skipByMode` 覆盖。而 `loop-back` 只发生在**gate phase 失败时**（`OnFail.Action == "loop_back"`）。gate phase 的 `on_fail` 跳回 `target_phase`（agent phase）。

所以当前代码中：
- gate 失败 → loop_back target_phase: reviewer → 如果 reviewer 被 mode 跳过，`skipByMode` 返回 true → `runPhaseParallel` 返回 nil 不执行 → 但 `nextStartPhase` 的 `phaseIndex` 仍能定位到它 → 下一迭代跳到跳过 phase → **实际上不会死循环，但会空转一轮**。

**你发现的问题核心是对的**（跳过 phase 不应是 loop-back 的合法目标），但具体机制是「空轮浪费一次迭代」而非「死循环永远不退出」的级别。

**影响**: 方向五的核心问题存在，但严重程度降低一级——从「死循环 hang」到「静默浪费迭代」。

---

### 补充（被分析遗漏的点）

#### 补充 A：方向一需要补充「交叉 emits 检测」作为先修条件

你的建议聚焦于文件锁 + git diff 后检测。但有一个**更低成本、更高收益**的先行步骤：**wave 内 phase 的 `emits:` 交叉检测**。

当 `Waves()` 将 phase 分组到同一 wave 时，它做了依赖分析但没有做**资源集分析**。可以在 wave 构建后加一步：

```
对于 wave 中每个 phase，比较其 emits 声明：
  - 如果两个 phase 声明 emits 到同一文件 → 自动串行化（将其中一个移至下一 wave 或子波）
  - 如果两个 phase 一个 emits 到 dir/a.md，另一个 reads（需新增 reads:字段）dir/a.md → 串行化
```

这比文件锁更轻量：**不需要运行时进程间协调，只需要调度时的声明分析**，且不需要每个 phase 声明新的 `writes:` 字段（可以先复用现有的 `emits:` 声明）。

#### 补充 B：方向二需要处理「append 模式 vs overwrite 模式」的验证差异

你建议验证 `emits` 文件存在且非空。但需区分两种写入模式：

1. **Overwrite**（phase 独占地写一个文件）：如 `design.yml` 的 solution-architect 写 `docs/adr/ADR-000*.md`——可验证存在 + 非空
2. **Append / 共享文件**（多 phase 追加同一文件）：如 `discover.yml` 的多个 phase 都写 `docs/discovery/prd.md`——只有**最后一个 phase 完成时**该文件才完整 → 中间验证会 false negative

解决方案：在 `emits:` 中增加语义标记：
```yaml
emits:
  - path: docs/discovery/prd.md
    mode: append  # 或 overwrite，默认 overwrite
```

对于 `append` 模式的 emits，只在 workflow 级别的**后验证**（而非 per-phase）执行检查。

#### 补充 C：方向四有「隐式可用中介」——wave 已自然限制了并行度

`parallel.go:95-105` 的 wave 循环是以 wave 为粒度启动的。每个 wave 包含**无依赖关系的 phase**。当前 dependency 图大多是窄的（discover.yml 的 3 个 scan phase 可以并行，但其他 stage 大多串行），所以 wave 大小通常 ≤ 3。

这意味着：在没有 `--max-wave-concurrency` 的条件下，wave 内的并发度天然被 dependency 图限制，不容易出现 OOM 场景。**真正的 OOM 风险在 `forge evolve` 本身**（long iteration 中 memory 文件增长 + checkpoint + trace），而非并行 agent spawn。

**建议优先级调整依据**：方向四的实际紧迫性比 P2 还低半级——建议 P3 或 P2-later，等出现真实资源瓶颈报告后再前置。

---

### 优先级调整建议

看了源码后，我的优先级排序与你有**两处分歧**：

#### 调整 1：方向③（Memory 隔离）应当放 P0，不是 P1

理由：`memory.go` 的 `Load` 现在会被 `for` 循环的**每个 phase、每次迭代**调用（`evolve.go` 的 loop 每次迭代都会加载 memory 来构建 prompt context）。当同一仓库上两个 evolve 并发运行时：

```go
// cmd/forge/evolve.go:168-175
loop := orchestrator.NewLoopEngine(eng, wf.Stop,
    func() converge.Signals { return gatherSignals(...) },
    effectiveIter, enforceNoProgress,
    func(s string) { logln(s) },
)
```

这里没有文件锁，没有 run_id 隔离。进程 A 的 `costly_gap` finding 被进程 B 的 agent 读到 → 进程 B 花数千美元处理一个不在其 scope 内的 gap。**这是当前代码库中最昂贵的静默错误**，没有之一。

而且修复成本极低——如你所说，加 `run_id` 字段 + Load 过滤。我建议做在 P0（下个 sprint 的第一件事）。

#### 调整 2：方向④（Resource-Aware Scheduling）建议降为 P3

补充 C 已论证——当前 wave 天然窄，OOM 风险低。真正资源瓶颈发生在 `forge evolve` 的长迭代中（memory + checkpoint + trace 累积），而非多 agent 并行。建议在方向①（并行冲突检测）实现并真实启用 `depends_on` workflow 后，方向④自然变 P2。

---

### 收敛建议（修订版）

| 原优先級 | 调整后 | 方向 | 调整依据 |
|---|---|---|---|
| P1 | **P0** | ③ Memory 隔离 | 静默知识污染是最昂贵的错误，修复成本最低 |
| P1 | P1 | ② Emits 验证 | 信任基线，验证器每 phase 多几次 `os.Stat`即可实现 |
| P1 | P1 | ① 并行冲突检测 | parallel mode 的 pre-condition，但当前零调用压力 |
| P2 | **P2** | ⑤ 状态空间验证 | workflow 数量超过 10 时自动升级为 P1 |
| P2 | **P3** | ④ 资源感知调度 | wave 天然窄 + 无 real-world 资源压力信号 |

**执行路线图**：
1. **Sprint N**：方向③（run_id + Load 过滤 + TTL）——~3 天
2. **Sprint N+1**：方向②（post-phase emits 断言 + 漂移检测 + schema 验证）——~1 周
3. **Sprint N+2**：方向①（交叉 emits 检测 + git diff 后检测 + 自动串行化）——~1.5 周
4. **Sprint N+3**：方向⑤（静态可达性检查 + stop 条件互斥验证）——~2 周
5. **方向④**：等待第一个 `depends_on: true` 用户报告后前置

---

### 一句话总结

**代码读得越深，方向③的危害越清晰、方向④的紧迫越低**。最昂贵的静默错误（memory 跨进程知识污染）修复成本最低，应该做 P0。方向①/②/⑤ 按 P1/P2 推进。方向④先搁置，等真实用户撞墙再说。

你的分析质量很高——五个方向都选在「产品市场 fit」和「架构韧性」的交汇点上，这是当前 163+ 份已有分析都没有的视角。修正的三点都是边界情况级别的精度调整，不影响方向本身的存在价值。
