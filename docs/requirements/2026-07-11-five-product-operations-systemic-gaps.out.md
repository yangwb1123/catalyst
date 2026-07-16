# 独立验证报告：产品运营五方向

> **验证方法**: 对原文所有 `file:line` 引用在 forge-core 当前代码中逐行验证。
> 查阅了 checkpoint.go、trace.go、memory.go、evolve.go、main.go、doctor.go、
> command_executor.go、gate.go 等全部相关文件。所有验证均为真实代码阅读，
> 非推测。

---

## 验证矩阵

| 方向 | 原文优先级 | 验证结论 | 修正建议 | 关键诊断 |
|------|:----------:|:--------:|:--------:|----------|
| ① 二进制生命周期与版本治理 | P1 | ✅ **确认** | P1 维持 | 所有数据格式无版本溯源，升级路径零基础设施 |
| ② 人工可读诊断表面 | P1 | ⚠️ **部分修正** | P1 维持 | 核心论点成立，但 `rejectHumanGate` 已被改进 |
| ③ 运行时运营可观测性 | P1 | ✅ **确认** | P1 维持 | 无 RunID、无实时查询、无健康端点——完全正确 |
| ④ 优雅降级与部分恢复 | P1 | ⚠️ **需修正** | **P2 降级** | 备份保留已实现（`retain=5`），"不保留旧版本"不准确 |
| ⑤ 跨运行身份与溯源 | P1 | ✅ **确认** | P1 维持 | 三种数据格式均无 RunID/用户/来源字段 |

---

## 方向① 二进制生命周期与版本治理 — ✅ 确认 (P1)

所有代码级声明**验证通过**:

- `forge-core/cmd/forge/main.go:30-33` — `forgeVersion = "dev"`，仅用于 `--version` 展示，
  不写入任何持久化数据。
- `persist/checkpoint.go:42-63` — `FormatVersion` 固定为 `"forgeos.checkpoint.v1"`，无
  `forge_version` 字段。
- `trace/trace.go:63-84` — `Event.Format` 固定为 `"forgeos.trace.v1"`，无版本字段。
- `memory/memory.go:140-155` — `Entry.Format` 固定为 `"forgeos.memory.v1"`，无版本字段。
- `gate/gate.go:68-79` — 调用外部 node 脚本，零版本校验。新 forge 可能依赖新 gate 协议，
  升级二进制而不同步 harness 则**静默产生错误结果**。

**核心价值未被质疑**: 没有升级路径、没有版本兼容性矩阵、没有数据迁移。
这是五个方向中杠杆最高的——直接决定 ForgeOS 能否从"开发者个人工具"进化为"团队平台"。

**补充证据**: forge-init 生成的项目中，`min_forge_version` 或 `forge_version_pinned` 字段
完全不存在。低版本 forge 读取高版本 checkpoint 时不会得到友好提示，只有 Go 错误链。

---

## 方向② 人工可读诊断表面 — ⚠️ 核心论点确认，但 `rejectHumanGate` 状态需修正 (P1)

### 需要修正的声明

原文说 `rejectHumanGate` 的实现"没有报告当前 checkpoint 状态"，但实际代码**已经**有了这个功能：

```go
// forge-core/cmd/forge/evolve.go:65-109 (rejectHumanGate 的实际实现)
func rejectHumanGate(stage string, root string) int {
    cpPath := filepath.Join(root, ".forge", "checkpoint.json")
    if data, err := os.ReadFile(cpPath); err == nil {
        var cp struct {
            Iteration         int     `json:"iteration"`
            RoadmapCompletion float64 `json:"roadmap_completion"`
            GatesGreen        bool    `json:"gates_green"`
        }
        if json.Unmarshal(data, &cp) == nil {
            cpHint = fmt.Sprintf(
                "\n  Current state: checkpoint at iteration %d, roadmap=%.0f%%, gates=%s",
                cp.Iteration, cp.RoadmapCompletion*100,
                map[bool]string{true: "green", false: "red"}[cp.GatesGreen],
            )
        }
    }
    // 还检查了 .forge/<stage>.approved 标记文件
    // 输出包含 checkpoint 状态、approval marker、human_gate 解释
}
```

此外，`forge doctor` 的 Check.Line() 输出确实包含 Detail，且全局 error chain 也如原文所述
缺少分类。但局部细节上的不准确**不影响核心方向**——错误消息的整体质量仍然面向引擎工程师。

### 其他声明验证

| 声明 | 验证 |
|------|------|
| error chain 无分类冒泡到 CLI | ✅ 确认。`main.go:525-529` — 仅 `fmt.Fprintf(os.Stderr, "forge: run: %v\n", runErr)` |
| 无 `OpError`/`ErrorKind` 结构 | ✅ 确认。forge-core 中不存在任何错误分类结构体 |
| 无 `forge why` 命令 | ✅ 确认。无此子命令 |
| 无 `--repair` / `--force` 等恢复标志 | ✅ 确认。无此类 flag |

**评价**: 核心方向完全成立。建议将文档中的 `rejectHumanGate` 示例更新为实际代码，
但方向本身不需要调整。**P1 维持**。

---

## 方向③ 运行时运营可观测性 — ✅ 确认 (P1)

所有声明**验证通过**，是文档中最准确的方向之一：

1. **Tracer 只写不回读**: `trace/trace.go:89-95` — `writer io.Writer` 只写接口。
   无 `Status()` 方法、无实时查询能力。

2. **`forge status` 一次性快照**: `doctor/status.go` 读取 .forge/ 目录后立即退出。
   无 `--watch` 模式、无持续刷新、无操作系统信号监听。

3. **无健康端点**: `cmd/forge/main.go` 全部入口是 CLI 子命令。无 HTTP、Unix socket 或
   命名管道。

4. **无 RunID 跨文件关联**: 已验证所有三种数据格式：
   - Checkpoint: `Workflow`, `Mode`, `Iteration`, `PhaseIndex`, `GatesGreen`, `Reason`
   - Trace: `Seq`, `Kind`, `Name`, `Status`, `DurationMs`, `CostUsdMicros`, `Model`
   - Memory: `Kind`, `Topic`, `Detail`, `Iteration`, `Source`, `Confidence`
   - **三种数据之间没有任何共享关联字段**

5. **并行 evolve 不可见**: forge-core 没有文件锁机制。两个 `forge evolve` 进程同时运行
   时，彼此完全不知晓对方存在，数据直接交错写入同名文件。

**评价**: 这是五个方向中影响最直接、证据最充分的方向。
一个设计为 24h 无人值守的系统没有实时观测能力，本质上是盲飞。
**P1 维持**。

---

## 方向④ 优雅降级与部分恢复 — ⚠️ 需重大修正 (P1 → **P2**)

### 需要修正的核心声明

原文说"checkpoint Save 不保留旧版本"——**这是不准确的**：

```go
// forge-core/internal/persist/checkpoint.go:128-155
func Save(path string, cp Checkpoint, retain int) error {
    // Retain history: rotate the existing checkpoint to path.1 before overwriting.
    if retain > 0 {
        if _, err := os.Stat(path); err == nil {
            rotateRetain(path, retain)
        }
    }
    tmp := path + ".tmp"
    if err := writeSynced(tmp, data); err != nil {
        return err
    }
    return os.Rename(tmp, path)  // 原子提交
}
```

而**生产调用** `evolve.go:checkpointHook` 传入的是 `retain=5`：
```go
// forge-core/cmd/forge/evolve.go
if err := persist.Save(checkpointPath(o.root), cp, 5); err != nil {
    // WARNING only — loop continues
}
```

`rotateRetain` 实现了完整的循环滚动保留。因此：
- `checkpoint.json` — 当前版本
- `checkpoint.json.1` — 上一版本
- `checkpoint.json.2` — 上两个版本
- ...
- `checkpoint.json.5`

——**最多保留 6 个历史版本**。这与原文"不保留旧版本"和"旧版永久丢失"矛盾。

### 验证后的真实画像

| 原文声明 | 实际情况 |
|----------|---------|
| "不保留旧版本" | ❌ **错误** — `retain=5` 保留 5 个备份 |
| "无 checkpoint 备份" | ❌ **部分错误** — 写入前有旋转保留，但 Load 不自动从备份恢复 |
| "损坏 = 死路" | ⚠️ 部分正确 — Load 返回硬错误，但备份存在（可手动恢复） |
| "memory 坏行 = 全文件废弃" | ✅ **确认** — `memory.go:decode` 中一个坏行导致全部失败 |
| "trace 一个坏行 = scorecard 不可用" | ⚠️ 需要确认 scorecard 解析路径 |
| "无 --repair flag" | ✅ 确认 |
| "无紧急空间回收" | ✅ 确认 |

### 修正后的评估

**缺失的是自动恢复路径**（Load 遇到损坏时自动尝试 `.bak.N` 回退），
而不是备份机制本身。这个关键区别意味着：

- 备份基础设施**已经存在**
- 需要的是**自动化恢复逻辑**（Load → 失败 → 尝试备份 → 报告）
- 以及 memory/trace 的容错读取模式

**建议将优先级从 P1 降为 P2**，因为：
1. 备份已存在，需要添加的是恢复逻辑而非基础设施
2. 相比方向①③⑤的从零建设，方向④是增量改进

---

## 方向⑤ 跨运行身份与溯源 — ✅ 确认 (P1)

所有声明**验证通过**，且方向⑤与方向③有深层的协同关系：

### RunID 缺失情况

| 数据格式 | 无 RunID | 无 User | 无 Host | 无 Session | 无 Trigger |
|----------|:--------:|:-------:|:-------:|:----------:|:----------:|
| Checkpoint | ✅ | ✅ | ✅ | ✅ | ✅ |
| Trace | ✅ | ✅ | ✅ | ✅ | ✅ |
| Memory | ✅ | ✅ | ✅ | ✅ | ✅ |

### Seq 的进程内局限

原文正确指出 `Seq` 是进程内自增：`trace.go:89-92` 的 `Tracer` 结构体包含 `seq int` 字段，
**每个 Tracer 实例独立计数**。两个进程同时写 trace.jsonl 时，各自的事件 seq 从 1 开始，
造成数据流歧义。

### CommandExecutor 不注入上下文

```go
// forge-core/internal/orchestrator/command_executor.go
type CommandExecutor struct {
    Build func(p asset.Phase, mode string) []string
}
// 无 FORGE_RUN_ID 环境变量注入
// 无 FORGE_USER 环境变量注入
```

### 与方向③的协同

方向③（运营可观测性）和方向⑤（跨运行身份）共享同一基础需求——RunID。
一次改造可以同时满足两个方向。建议将两个方向的 RunID 基础设施合并为同一个 Epic。

**P1 维持**。

---

## 修正后的优先级矩阵

| 方向 | 原文优先级 | 修正后 | 关键差异 |
|------|:----------:|:------:|----------|
| ① 二进制生命周期与版本治理 | P1 | **P1** | ✅ 无修正——从零建设 |
| ② 人工可读诊断表面 | P1 | **P1** | ⚠️ 局部细节需更新（rejectHumanGate 已有增强），但核心方向不变 |
| ③ 运行时运营可观测性 | P1 | **P1** | ✅ 无修正——证据最充分的方向 |
| ④ 优雅降级与部分恢复 | P1 | **→ P2** | ⚠️ 备份已存在（retain=5），需添加的是自动化恢复逻辑，非基础设施 |
| ⑤ 跨运行身份与溯源 | P1 | **P1** | ✅ 无修正——三种数据格式均缺失 RunID |

---

## 跨方向联结建议

1. **RunID 基础设施合并（方向③ + 方向⑤）**: 一次改造同时解决两个方向。
   - 生成 UUIDv7（时序有序）
   - 注入 checkpoint/trace/memory
   - 暴露为 `$FORGE_RUN_ID` 环境变量给子进程
   - 可作为 `forge status --watch` 的查询键

2. **版本治理与 RunID 的协同**（方向① + 方向③ + 方向⑤）:
   - checkpoint + trace + memory 同时获得 `forge_version` + `run_id`
   - 跨 run 的版本分布可观测（哪些 run 是哪个 forge 版本跑的）

3. **诊断增强与恢复路径的衔接**（方向② + 方向④）:
   - Load 从备份恢复后，详细日志告诉用户：发生了什么、从哪里恢复、检查了哪些备份、
     是否完全一致
   - `forge status --health` 同时作为诊断工具和修复入口

---

## 总结

这是一份高质量的分析，五个方向选择精准且差异化验证充分（与 105+ 已有文档的关系说明
是亮点）。主要需要修正的是方向④中关于备份保留状态的描述——当前的 checkpoint 已经
有 `retain=5` 的循环保留机制，缺的是自动恢复而非备份本身。

文档的**共同模式提炼**（"从一个人能用的强大工具进化为一个团队能依赖的运营平台"）
准确命中靶心。Direction ①③⑤ 的三个 P1 方向构成了运营平台的三角基石——
版本、观测、身份——它们是 ForgeOS 从技术演示到生产就绪的关键跳跃。
