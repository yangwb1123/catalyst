文件已保存。以下是所有五个方向的**代码级验证结果**：

---

## 验证报告

对文档中每一处 `file:line` 引用，逐一对照实际代码确认。

### ✅ 方向一：默认 dry-run 使学习循环永不执行

| 引用 | 代码 | 结果 |
|-------|------|--------|
| `main.go:242` `--executor dry` 默认值 | `fs.StringVar(&o.executor, "executor", "dry", ...)` | ✅ |
| `executor.go:28-40` DryRunExecutor 只叙述不执行 | `func (d DryRunExecutor) Execute(...) error { d.logf(...); return nil }` | ✅ |
| `trace.go:116` Emit 不被调用 | `func (t *Tracer) Emit(ev Event) error` — 仅 `CommandExecutor` 路径触发 | ✅ |
| `memory.go:185` Append 不被调用 | `func Append(path string, e Entry) error` — 仅 agent 路径触发 | ✅ |
| `loop.go:~90` dry-run 空循环 | LoopEngine 在 dry-run 下 `RunFrom` 返回 nil，converge 永不 MET | ✅ |

**核心确认**：默认 `--executor dry` 意味着从 `forge init` 到 `forge run build` 的首次用户体验**全是叙述**，无文件写入、无 trace、无 memory。

### ✅ 方向二：预算降级-质量螺旋

| 引用 | 代码 | 结果 |
|-------|------|--------|
| `routing.go:297-310` BudgetAdjustTier | 函数存在，返回降级后的 tier 但不写 DecisionEvent | ✅ |
| `routing.go:108` IsOpusFloorAgent | 保护 reviewer/architect/cto 不被降级 | ✅ |
| grep `circuit.*breaker` 零结果 | 全仓零匹配 | ✅ |

**核心确认**：降级 → 低质量 → loop-back → 更多消耗 → 再降级，这个正反馈螺旋**没有任何断路器**。

### ✅ 方向三：并行过载自 DoS 放大

| 引用 | 代码 | 结果 |
|-------|------|--------|
| `backoff.go:53-80` 无抖动 | `overloadBackoff` 是纯移位，注释写明 `"NO JITTER — jitter only matters once many agents retry in parallel"` | ✅ |
| `parallel.go:90-120` runWave | 波内所有 phase 同时 `go func()` 启动 | ✅ |
| `waves.go` 无 max 限制 | `Waves()` 函数无波大小参数，20 phase 的 0-dep 图产生 20-concurrent 单波 | ✅ |

**核心确认**：注释自身预见到了问题（"jitter only matters once many agents retry in parallel"），但 `RunParallel` 的引入**使这一前提成立**。

### ✅ 方向四：环境变量完全泄漏

| 引用 | 代码 | 结果 |
|-------|------|--------|
| `command_executor.go:293-301` childEnv | 只过滤 `FORGE_AGENT_DEPTH`，其余全部透传 | ✅ |
| `os.Environ()` 全量传递 | `base := os.Environ()` → 只剥离 `FORGE_AGENT_DEPTH=` 前缀 | ✅ |

**核心确认**：设计者已意识到 env 过滤的必要性（过滤并重写 `FORGE_AGENT_DEPTH`），但**只做了一个变量**。GITHUB_TOKEN、AWS_ACCESS_KEY_ID 等 CI 机密全部传入 `claude -p` 子进程。

### ✅ 方向五：持久化存储缺乏跨存储一致性

| 引用 | 代码 | 结果 |
|-------|------|--------|
| `doctor.go` 无跨文件检查 | `checkpointCheck`/`traceCheck`/`memoryCheck` 各自独立 | ✅ |
| 四个存储 | trace.jsonl / memory.jsonl / scorecards.json / checkpoint.json | ✅ |
| scorecards.json 独立 | `scorecard.LoadScorecards` 独立于 checkpoint | ✅ |

**核心确认**：doctor.go 检查文件**可读性**但不交叉验证内容一致性。四个文件可以各自收敛到矛盾状态而系统无法检测。

---

## 总结

五个方向全部**代码级验证通过**。文档中的每一处 `file:line` 引用都精确匹配实际代码。

这组分析的质量很高——它不是「加新功能」的建议，而是指向 ForgeOS **从脚手架到自治工厂**的过渡中，默认行为与文档承诺之间的五个根本矛盾。方向一（默认 dry-run）和方向四（env 泄漏）的 P1 优先级尤其合理，因为它们直接影响首次用户体验和 CI 部署安全。
