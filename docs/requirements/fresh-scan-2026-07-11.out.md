现在我在充分验证代码依据的基础上，给出架构审阅反馈。

---

## 架构审阅:对这份分析的评估

### 总体评价

这是迄今对该仓库最**诚实、代码依据最扎实**的一份扩展方向分析。五个方向都立足于可验证的代码证据——我验了 `parallel.go` 锁序契约、`prompt_memory.go` 的 `memoryCap=32`/`boundMemory` 实现、`checkpoint.go` 的 rename-then-sync 原子写、`loop.go` NoProgress tripwire——所有论断在源码中都有精确锚点。与 `docs/requirements/` 下 ~200 份历史文档相比，这份分析确实做到了"从代码推导、与历史正交"。

以下是我逐方向的技术审阅+修正建议。

---

### 方向一:并行写冲突检测 ✅ 核心论据成立,但需补充一个设计细节

**现状描述准确。** `parallel.go:1-52` 确实只声明了 Go 内存状态的锁序(8 级)，`CommandExecutor` 通过 `os/exec` 子进程直接写文件系统，没有任何文件级冲突检测。`--parallel` flag 在 `main.go:254` 已存在。

**需要修正/补充的一个设计盲点:**

> **同一 phase 自身可能写入多个文件(通过 Bash),wave 结束做全 diff 是 O(n²) 复杂度。** 文档说"对 wave 内所有 phase 的 emit 目录做 diff"——但 phase 不一定只写 `emits:` 清单里的文件。agent 可能写临时文件、修改非 emits 文件。真正的 scope 应该是**整个工作树在 wave 前后的完整 diff**(`git diff --name-only` 或 `diff -r`),不是扫描 emits 清单。这意味着:
> - 每个 wave 开始前需要一个快照(`git stash` 或 `rsync` 基线)
> - wave 结束后做 diff → 多 phase 同文件被修改 → 合并分析
>
> 现在的描述("对 phase 的 emit 目录做 diff")范围太小,可能漏掉非 emits 文件的冲突。

**建议改写:**
- 检测范围 = wave 开始前/后整个工作树的 `git diff`,不限于 `emits:`
- 这天然兼容 git 工作流,且 `git merge-file` 可在工作树内就地合并,无需临时目录

**另一个边角情况:** 一个 phase 删文件、另一个 phase 改同一文件 → 分析说"按删优先,被删方的修改注入下一迭代"。但如果 delete 是正确行为(旧 API 被废弃),注入修改会导致它重新出现。应该在**裁决 phase 中由 LLM 判断**而非机械规则。

---

### 方向二:上下文毒性检测 ✅ 核心论据成立,实现约束有改进空间

**现状描述准确。** 确认了:
- `memoryCap=32` 在 `prompt_memory.go:48`
- `boundMemory` 只做 recency floor + BM25 相关性,不做矛盾检测
- `memory.Compact`(`memory_compact.go`)按 Kind 分组去重,但不检矛盾
- `scorecard.go:134` 注释说 `recency_half_life_days` 在路由中"intentionally NOT applied"

**需要修正的一个事实错误:**

> 文档说 "`recency_half_life_days=30` 声明在 `policy.yml`" — 但我在 `.agent/routing/policy.yml` 中**没有找到**这个字段。它在代码中只有 `scorecard.go:134` 的一行注释提及,且注释说"intentionally NOT applied"。这意味着 `recency_half_life_days` 目前**不存在于任何配置文件中**,只是一个被考虑过但未落地的概念。

**建议:** 将 "λ 取自 `policy.yml` 已有但未在 memory 语境消费的 `recency_half_life_days`" 改为**第一步先硬编码默认值**(如 30),或在新字段 `policy.yml → memory.recency_half_life_days` 中引入。不要声称它"已在 policy.yml 存在"。

**实现约束的改进建议:**

文档说毒性检测只做"可判句法/结构矛盾",通过关键词匹配(`but`/`instead`/`revert`)。这过于脆弱。考虑:

1. **简单改进(不引入 embedding):** 同 `Kind` + 同 `Topic` 下,新 entry 的 `Content` 包含 `NegationScore > 0.5` 的句法特征(否定词密度 + 对比连词) → 标记矛盾候选。这比纯关键词可靠,且仍可纯 Go 实现(不需要 embedding 模型)。

2. **更强的改进:** 复用 `internal/prompt` 已有的 BM25 `Retrieve`。将旧 entry 作为 query 检新 entry——如果高相关性但文本中无明显继承关系(新 entry 没有引用旧 entry 的迭代号),标记为"可能遗忘/覆盖"。这不需要新依赖。

**边界情况补充:**

- **知识沉默(silent drift):** 最危险的不是显式矛盾(新旧都说同一话题但结论不同),而是**旧知识被新知识静默取代但无任何冲突信号**(例如:旧说"gate 跑 3 分钟",新说"gate 跑 3 秒",两者不矛盾——系统实际变了但 agent 以为一直如此)。这比矛盾更难检测。建议增加 `delta_monitor`——同一 `Kind`+`Topic` 条目的关键数值/决策单向突变时打标。

---

### 方向三:选择性相位执行 ✅ 论据扎实,但复杂度评估偏低了

**代码确认:** 
- `main.go:246-254` 有 `runOpts.parallel` 但无 `--phase-from`/`--phase-to`
- `forge run` 始终从 phase 0 开始,无范围选择
- `forge gate test` 子命令存在(`main.go:72`)但 evolve/run 不复用

**复杂度评估需要调整:**

文档标"低(CLI flag + 过滤 loop)",但实际上:

1. **相位索引解析不是字符串匹配。** Workflow phase 通过 `asset.Phase.Name` 标识,但执行路径中核心单元是 phase index。`--from architect` 需要将名称解析为 index → 涉及 phase 重名风险(two phases with same name in diff stages)。需要:解析阶段名→索引映射,且验证唯一性。

2. **`--from` 中间跳过的 phase 的 `feeds_forward` 状态怎么处理?** 如果跳过 `planner` phase(其 `feeds_forward: true`),下游收到空的 phase output。这需要**注入缺省警告**而非静默跳过,否则下游 phase 的 prompt 上下文缺少关键输入。

3. **Checkpoint 不一致更难处理:** 如果 operator 用 `--from 3 --to 5` 跳过 phase 0-2,然后写入 checkpoint——下次 resume 看到的 `PhaseIndex=3` 但前序 state 缺失。文档说"flight-recorder 模式不写入 checkpoint"覆盖了这个场景,但**非 flight-recorder(调试重跑)**也需要禁止 checkpoint 写入,或写入时标记 `was_partial:true`。

**建议:** 将复杂度从"低"提升为"中"。特别是 checkpoint 一致性保护需要与 `persist/checkpoint.go` 的 Save 路径协作——新增 `type RunKind string { NormalRun, DebugRun, FlightRecorder }` 传入 engine,engine 在 DebugRun 模式下跳过 checkpoint 写入。

---

### 方向四:阶段间产出契约 ✅ 诊断准确,但 missed 了一个已有基础设施

**代码确认:**
- `asset.Phase` 有 `FeedsForward bool`(`asset.go:152`),有 `Emits []string`,但**无 schema 绑定**
- prompt_context.go 消费 `feeds_forward` 产物时通过 `phaseOutputLedger`,不做格式校验

**关键补充——已有基础设施可复用:**

文档提出"每个 `artifact_kind` 注册一个 `func(io.Reader) error`"——这不错,但忽略了现有的 **`converge.Signals` 模式**:

```
type Signals struct {
    RoadmapCompletion float64
    GatesGreen        bool
    // ...
}
```

`converge` 包已经有**带外质量信号**的模式。artifact schema 验证应该作为 `converge.Signal` 注入,而非独立的验证层。具体来说:

```go
// 新的 converge signal
type ArtifactHealth struct {
    PhaseName string
    Path      string
    Valid     bool
    Warnings  []string
}
```

这样 artifact 验证结果天然进入 converge 的诚实性报告,复用既有的事件/观察框架,不需要新管道。

**格式版本兼容性更好方案:**

文档说 `artifact_kind` 带版本号(`prd.md/v1`, `prd.md/v2`)。这会把版本号编码在文件名里——但文件名是 agent 卡声明的,agent 改版本号时需要同时改 agent 卡和 schema 注册。更好的设计:

- `emits:` 声明文件名**不包含版本号**(仍是 `prd.md`)
- agent 卡新增 `artifact_schema: prd.md@v1` 字段,引用 schema 注册表中的版本
- 消费者声明 `expects_schema: prd.md@v1..v2`(兼容范围)
- schema 注册表维护版本映射,编译时检查兼容性

这样版本更新时不改 agent 卡的 `emits:` 路径,保持向后兼容。

---

### 方向五:控制平面故障注入 ✅ 诊断准确,但一个重要缺口遗漏了

**代码确认:**
- `checkpoint.go` 有 `writeSynced`(写+fsync+close)、`os.Rename(tmp, path)` 原子提交
- 但**没有任何测试注入 `ENOSPC` 或 `EIO`** 来验证恢复行为
- `backoff.go` 无 clock 注入接口(`time.Since` 硬编码)
- `loop.go:107-114` NoProgress 用 `time.Now`(墙钟),非单调时钟

**文档遗漏的重要缺口——但非常重要:**

> **checkpoint 的 `retain > 0` 历史旋转路径从未被故障注入覆盖。** `rotateRetain` (checkpoint.go:119-132) 有大量 `os.Rename` 调用,且注释说"best-effort: a single rename failure logs nothing but does not abort the caller's Save"。但如果 `path.1` 存在且是目录(而非文件),`os.Rename` 会有跨文件系统的边界行为差异——ext4 允许 rename 覆盖空目录,XFS 可能返回 EXDEV。当前 `rotateRetain` 开头检查 `os.Stat(path)` 是文件,但不检查 `path.1` 是否为目录。这是一个**真实的隐蔽竞态**,在容器化构建中(NFS 或 overlayfs 挂载)可能触发。

**建议在方向五的边界情况中补充:**

- `rotateRetain` 中的每个 `os.Rename` 都需要注入故障覆盖——特别是目标 `.N` 文件可能被外部操作替换为目录的场景
- 测试应覆盖 `retain=0`(旧行为)和 `retain>0`(历史保留)两条路径

**另一个交叉方向的机会:**

文档说"故障注入是方向五,在主要功能交付后夯实"。但我认为**方向五应该提前到方向二和三之间**。原因是:

- 24h 自治运行的 checkpoint 可信度是**所有方向的运行前提**。如果 checkpoint 在磁盘满时损坏,方向三的选择性执行(依赖 checkpoint resume)和方向一(并行 checkpoint)都受影响。
- 故障注入大多是 **`_test.go` 纯测试代码**,不增加生产二进制大小或复杂度。可以在方向二之后交付,不阻塞其他方向的开发。

**建议优先级:** ③ → ② → **⑤** → ① → ④

---

### 汇总:修正后的评估

| 方向 | 分析质量 | 事实修正需要 | 设计建议 | 优先级调整 |
|---|---|---|---|---|
| ① 并行写冲突检测 | ✅ 高质量 | 检测范围应为全工作树 diff,非 emits 目录 | 删改冲突的裁决应交给 LLM,非机械规则 | 不变 |
| ② 上下文毒性检测 | ✅ 高质量 | ❌ `recency_half_life_days` 不存在于 policy.yml | 建议用 BM25 检索替代关键词匹配做矛盾候选检测;补充 silent drift 监测 | 不变 |
| ③ 选择性相位执行 | ✅ 论据扎实 | 复杂度应从"低"提升为"中" | 需引入 `RunKind` + checkpoint 写入保护 | 不变 |
| ④ 阶段间产出契约 | ✅ 诊断准确 | 应复用 `converge.Signal` 模式而非新管道 | 版本映射建议用 schema 注册表+声明范围,不在文件名编码版本 | 不变 |
| ⑤ 控制平面故障注入 | ✅ 诊断准确 | ⚠️ 遗漏 `rotateRetain` 的目录覆盖竞态 | 建议优先级提前到第二/三位置(不放在最后) | **提前** |

**总结:** 这是该代码库当前最好的扩展方向分析之一——论点都有代码锚点、约束条件考虑周全、边界情况覆盖深入。以上修正不影响核心方向的有效性,第五方向的优先级调整是我唯一实质性的不同意见。建议按 **③→②→⑤→①→④** 顺序执行,并在方向五交付后重新评估方向一的并行 checkpoint 写入安全(两者有依赖关系)。
