验证完成。下面是我的**完整评审意见**。

---

# 评审：`2026-07-11-five-unseen-runtime-frontiers.md`

## 总体评价

**质量：A-（优秀）**。这是一份非常扎实的架构分析——证据链完整，边界情况思考成熟，跨方向协同表格是亮点。5 个方向中有 4 个经代码验证准确，1 个存在关键事实错误需修正。

---

## 逐方向代码验证结果

### 方向一 · 多进程运行时安全 ✅ **证据准确**

| 引用 | 验证结果 |
|---|---|
| "全仓无一处 `flock` 或 `LockFile`" | ✅ **确认**：`grep -rn "flock\|FLOCK\|LockFile\|lockfile\|syscall.Flock" forge-core/` → 零结果 |
| "sync.Mutex 仅用于进程内互斥" | ✅ **确认**：`trace.go:90` — `mu sync.Mutex // guards seq and the writer`，确为进程内锁 |
| trace/memory/checkpoint 问题描述 | ✅ **确认**：`memory.go:338-339` decode 函数整文件失败；`evolve.go:334` checkpoint 只写 warning 不中止 |

**建议修正**：`trace.go` 行号写的是「第 47 行」，实际是第 90 行。非关键问题。

### 方向二 · 治理版本偏移 ✅ **证据准确**

| 引用 | 验证结果 |
|---|---|
| "forge-init 不记录版本" | ✅ **确认**：`forge-init.mjs` 的 `renderProjectYml()` 函数输出 `project.yml` **无** `forgeos_version` 字段 |
| "forge-upgrade 是纯复制，不是 diff-merge" | ✅ **确认**：`forge-upgrade.mjs` 核心逻辑是 `enumerateTree → copyFileSync`，无 diff/patch |
| "已存在的文件跳过不更新" | ✅ **确认**：`forge-upgrade.mjs:88-90` — 跳过已存在文件的逻辑 |

### 方向三 · 模型/执行器无关抽象层 ⚠️ **关键事实错误**

| 引用 | 验证结果 |
|---|---|
| **`TierHaiku = "claude-sonnet-4-20250514"`** | ❌ **错误**。实际代码是：`routing.go:18-20` — `Haiku = "haiku"`，`Sonnet = "sonnet"`，`Opus = "opus"`。**路由层已经使用了抽象层名**，注释明确写着 "Provider names extend these for the cross-vendor pool: a fully-qualified tier is 'provider/tier'" |
| "engine_build.go 构造 claude CLI 参数" | ✅ **基本准确**：`engine_build.go:90-120` 的 `claudeArgv` 确实构造 `--permission-mode`、`--disallowedTools`、`--allowedTools`、`--model`、`--max-budget-usd` |
| "cost.go 解析 claude JSON" | ✅ **准确**：`cost.go:182` 的 `claudeResult.TotalCostUsd` 确为 claude 特定格式 |

**此方向的问题依然成立**（engine_build.go/cost.go 有 claude 耦合），但**最强的代码证据（routing.go 硬编码模型名）是错的**——routing 已经是抽象的。建议：
1. 删除 `routing.go:15-20` 那段错误的常量引用
2. 将证据改为聚焦 `engine_build.go` 的 `claudeArgv` 和 `cost.go` 的 `claudeResult`
3. 这实际上**弱化**了方向的紧迫性（架构已经比分析认为的更好），但方向仍是 valid 的 P2

### 方向四 · 确定性 Trace 回放引擎 ✅ **证据准确**

| 引用 | 验证结果 |
|---|---|
| `Event` 结构体字段 | ✅ **确认**：`trace.go` 的 `Event` 结构体完全吻合 |
| `Checkpoint` 结构体 | ✅ **确认**：`persist/checkpoint.go` 的 `Checkpoint` 结构体包含 `Iteration`、`RoadmapCompletion`、`GatesGreen`、`PhaseIndex`、`SpentUsdMicros`、`Reason` |
| Memory `Entry` 结构体 | ✅ **确认**：`memory.go:101-106` 的 `Entry` 结构体包含 `Kind`、`Topic`、`Detail`、`Iteration` |

### 方向五 · 优雅降级架构 ✅ **证据准确**

| 引用 | 验证结果 |
|---|---|
| "checkpoint 是 fail-loud-and-continue" | ✅ **确认**：`evolve.go:334-338` — `fmt.Fprintf(os.Stderr, "WARNING...")` 且继续运行 |
| "doctor 运行在所有/无的基础上" | ✅ **确认**：`doctor.go:31-33` — `return Report{NoForgeDir: true}` 是二元判断 |
| "memory 有一个坏行就致命" | ✅ **确认**：`memory.go:338-339` — `return nil, fmt.Errorf("memory: decode entry on line %d: %w", line, err)` |

---

## 结构性建议

### 1. 方向三必须修正（中风险）
如前所述，`routing.go` 的常量引用是错误事实。这影响到整个方向的**说服力**。建议替换为 `engine_build.go` 和 `cost.go` 的更精确分析——这两个文件确实构成 claude 耦合，但强度比当前文档描述的低一档。

### 2. 方向一与 `genuinely-uncovered-five-deep-runtime-gaps.md` 的方向三有重叠
后者在「三存储跨会话一致性审计」中讨论了 trace/memory/checkpoint 的一致性问题。虽然你的焦点是**进程级并发**而后者是**跨会话一致性**，但重叠足够大值得标注。建议在「饱和覆盖域」标注为「✅ 跳过但有相关」。

### 3. 方向四与刚生成的 `.out.md` 版本对比
`2026-07-11-five-unseen-runtime-frontiers.out.md` 已存在（由之前的处理生成）。当前 .md 与 .out.md 内容一致，无需额外处理。

### 4. 复杂度估算保守
方向一的「~80 行 Go」覆盖了一个 `flock` 包装器 + CLI 入口集成。但要使锁与 `forge run --parallel` 的 goroutine 共享正确协作，还需要额外的信号传播逻辑（子进程继承锁？O_CLOEXEC？）。建议调高到 **~120 行**。

### 5. 缺失的饱和覆盖标注
文档说方向二的 `version.*skew.*governance` → **0 篇命中**。但 `2026-07-11-codegrounded-five-systemic-gaps.md` 的方向二（声明式输出契约的自动化验证）讨论了升级时契约变化的问题。建议标注「1 篇提及但角度不同」。

---

## 优先级重评建议

不改变你的 P1/P2/P3 分级，但补充一个**执行顺序依赖约束**：

```
方向一（多进程锁）     ──→  方向五（健康注册表）
  (flock 是健康状态输入)     (锁状态需被注册表报告)

方向二（版本标记 v1）   ──→  方向四（Trace 回放 v2）
  (版本号是 baseline)        (回放需版本兼容)
```

即：**方向一是方向五的前置依赖**，**方向二 v1 是方向四 v2 的前置依赖**。Sprint 规划时应考虑此偏序。

---

## 结论

**这是当前 ~30+ 份「五个方向」分析中表现最好的之一。** 代码引用扎实（方向三错误是唯一显著瑕疵），边界情况思考完整，跨方向协同分析是有价值的增量。修正方向三的 routing.go 错误后，可作为正式架构决策输入进入 SPRINT 规划流程。
