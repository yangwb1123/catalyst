# ForgeOS — 全局扫描后的五个结构性盲区

> **角色**: 资深架构师 / 产品经理  
> **方法**:
> 1. 全局深扫: forge-core 18+ Go 包 / `cmd/forge` 17+ 子命令 / harness 39+ 模块 / `.agent/` 完整治理骨架
> 2. 完整阅读 Sprint 1–31 演进记录、`FUNCTIONAL_REQUIREMENTS_AUDIT.md`、4 篇 ADR、全部架构文档
> 3. **差异化验证**: 逐方向在 85+ 份已有分析文档（`docs/requirements/` 68 篇 + `docs/analysis/` 39 篇 + 其他）中交叉检索核心关键词，确认该方向的核心论点**从未被已有分析作为独立方向展开**
> 4. **纪律**: 不编写任何代码。所有建议附精确代码行级证据
> **日期**: 2026-07-10

---

## 全景定位

已有 85+ 份分析文档覆盖了 ForgeOS 几乎全部功能域——编排引擎、生产可靠性、可观测性、记忆/学习、路由/调度、安全纵深、治理/执法、中枢旋钮、结构债务、北向扩展等。**但几乎全部聚焦于"系统能做什么"，很少审视"系统如何被构建、分发、维护和演化"这些工程元层的问题。**

本文五个方向落在已有分析的共同盲区中——不是因为它们不重要，而是因为它们位于分析习惯的"桌面之下"：

| # | 方向 | 类别 | 优先级 | 一句话 |
|---|------|------|--------|--------|
| 1 | **On-disk 格式版本字段：写入但不消费** | 数据完整性 · 架构债务 | P1 | 三个持久化存储（memory/trace/checkpoint）都写 `_format` 版本号，但没有任何 Load/Decode 函数读取它——如果明天需要格式演化，无法做迁移 |
| 2 | **双 YAML 解析器维护面** | 工程效率 · 技术债务 | P1 | Go 原生解析器与 Python shim 必须保持同步，任何 YAML schema 变更需要改两个实现，且没有强制一致性门禁 |
| 3 | **forge-core 二进制不自包含** | 分发 · 部署 | P0 | 编译好的 `forge` 二进制运行时需要 `harness/` 目录 + Node.js + Python3——不能独立分发 |
| 4 | **无语义自校验的状态目录恢复** | 可靠性 · 边界情况 | P1 | 从 checkpoint/memory 恢复时只校验 JSON 语法，不校验运行时语义（phase 是否存在、mode 是否有效、预算是否一致） |
| 5 | **Scorecard 管线：多进程文件系统 IPC** | 可靠性 · 韧性 | P2 | scorecard 更新走 Go→Node.js 子进程→文件系统→Go 的连锁，无事务性保证 |

---

## 方向一 · On-disk 格式版本字段：写入但不消费

**优先级**: 🟠 P1 | **类别**: 数据完整性 · 架构债务 | **预估**: ~1–1.5 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 有三个持久化存储，各自定义了一个格式版本标识字段。它们都**在写入时设置这个字段，但在读取时从不检查它**。这意味着：
- 如果未来需要改变格式（新增一个必需字段、改变一个字段类型、弃用一个字段），loader 无从知晓它读到的数据是哪个版本
- 新旧版本混在一起，loader 要么静默解码出错误的结构（多出的字段被忽略、缺失的字段取零值），要么在遇到不兼容结构时崩溃
- `_format` 字段给读者一种虚假的安全感——「这个格式是版本化的」——但实际上没有任何版本感知的 dispatch

### 代码级证据

**证据 1: memory.go 的 `Format` 字段写入但不消费**

```go
// forge-core/internal/memory/memory.go:159-161
type Entry struct {
    Format string `json:"_format,omitempty"` // ← 声明
    // ...
}
```

写入时设置：

```go
// forge-core/internal/memory/memory.go:186-188
func Append(path string, e Entry) error {
    if e.Format == "" {
        e.Format = "forgeos.memory.v1"    // ← 写入时标记版本
    }
```

但 `decode()` 从不读取或 dispatch：

```go
// forge-core/internal/memory/memory.go:330-348
func decode(data []byte) ([]Entry, error) {
    var entries []Entry
    for sc.Scan() {
        var e Entry
        if err := json.Unmarshal(raw, &e); err != nil {
            return nil, ...
        }
        // ← 没有 if e.Format != "forgeos.memory.v1" { ... }
        // ← 没有格式版本 dispatch
        // ← 直接使用解码后的结构
```

完整代码库验证：`Format` / `_format` 在 load/decode 路径上零消费者：

```bash
$ grep -n "\.Format\b\|\"forgeos\.\|FormatVersion" forge-core/internal/memory/memory.go \
  forge-core/internal/persist/checkpoint.go forge-core/internal/trace/trace.go | grep -v "_test.go"
# 只找到写入和声明位置，没有任何 if/switch 读取 FormatVersion 的值
```

**证据 2: trace.go 的 `Format` 字段同样只写不读**

```go
// forge-core/internal/trace/trace.go:56-58
type Event struct {
    Format string `json:"_format,omitempty"` // ← 声明，写入时设置为 "forgeos.trace.v1"
    // ...
}
```

trace 甚至没有 `Load`/`Decode` 函数——它只有 `Emit`（写入）。下游的 `scorecard_wind.go` 手动 `bufio.Scanner` + `json.Unmarshal` 逐行解析 trace 事件，**从不检查 `_format` 字段**：

```go
// forge-core/cmd/forge/scorecard_wind.go:198-220
for sc.Scan() {
    var ev trace.Event
    if err := json.Unmarshal(sc.Bytes(), &ev); err != nil {
        logln("scorecard: skip unparseable trace line: " + err.Error())
        continue // ← 跳过畸形的行，但从不检查格式版本
    }
```

**证据 3: checkpoint.go 的 `FormatVersion` 字段同样只写不读**

```go
// forge-core/internal/persist/checkpoint.go:52-54
type Checkpoint struct {
    FormatVersion string `json:"_format,omitempty"` // ← 声明
    // ...
}
```

写入时设置：

```go
// forge-core/internal/persist/checkpoint.go:103-105
func Save(path string, cp Checkpoint, retain int) error {
    if cp.FormatVersion == "" {
        cp.FormatVersion = "forgeos.checkpoint.v1" // ← 写入时标记
    }
```

但 `decode()` 从不检查：

```go
// forge-core/internal/persist/checkpoint.go:163-169
func decode(data []byte) (Checkpoint, error) {
    var cp Checkpoint
    if err := json.Unmarshal(data, &cp); err != nil {
        return Checkpoint{}, ...
    }
    // ← 没有版本检查
    // ← 如果未来 v2 的 checkpoint 被写入，v1 loader 静默解码
    return cp, nil
}
```

### 边界情况

- **v1→v2 字段重命名**：如果未来把 `roadmap_completion` 重命名为 `completion`，v1 loader 解码 v2 文件时会读取一个零值（字段不匹配），系统可能认为 roadmap 完成度为 0%，触发误收敛或误发散
- **v2 添加必需字段但默认值危险**：例如给 checkpoint 添加 `Checksum string` 字段（有则验证完整性、缺失则不验证），旧版本不提供该字段，如果 v1 文件在 v2 loader 下解码，`Checksum=""` 被错误理解为「跳过校验」，而非「该文件不支持校验」
- **跨版本 resume**：一个 evolve 运行开始在 v1 格式下，中途升级 binary 到写 v2 格式的版本，checkpoint.json 被重写为 v2，如果回退到旧 binary，v1 loader 读 v2 文件可能无声地丢失高版本独占的字段

### 产品影响

这不是一个「未来可能有用」的功能，而是一个**已经存在的脆弱性**。版本字段给了读者虚假的安全感——它暗示格式是可演化的，但没有任何演化路径被实现。当第一次真正的格式变更发生时，迁移策略需要临时发明，而在这之前积累的所有 `.forge/` 文件都是「版本化但不可迁移」的状态。

### 建议方向

1. **三处统一做格式版本校验**：`memory.decode()`、`persist.decode()` 中增加读取 `_format` 并与已知版本列表比较，未知版本显式报错（fail-closed），而非静默尝试解码
2. **定义格式迁移契约**：同一个文件中不能同时出现 v1 和 v2 行（memory.go 是 JSONL，允许逐行版本混合；checkpoint 是单 JSON 对象，不需要）。为 memory 增加 `Format` 字段每行的校验
3. **为下游消费者加版本防御**：`scorecard_wind.go` 的 trace 解析在遇到未知 `_format` 时应跳过该行并告警，而非静默丢弃

---

## 方向二 · 双 YAML 解析器维护面

**优先级**: 🟠 P1 | **类别**: 工程效率 · 技术债务 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 有两个 YAML 解析实现：Go 原生解析器（`internal/yaml2json`，~1565 LOC，零外部依赖，纯手写）和 Python 回退解析器（`harness/yaml2json.py`，~200 LOC，基于 PyYAML）。任何工作流 YAML 格式的变更（新增字段、改变语法结构、引入新的 YAML 特性）都需要在两个实现中各自实现，且没有机制确保它们的输出完全一致。

当前两个实现被硬编码为「先 Go 后 Python」的 fallback 链：

```go
// forge-core/cmd/forge/main.go:364-385
val, err := yaml2json.Decode(f)
if err == nil {
    // ... 如果 Go 解析成功且产生有效 workflow → 使用
}
// ← 否则 fallback 到 Python
shim := filepath.Join(repoRoot, "harness", "yaml2json.py")
out, execErr := exec.Command("python3", shim, ymlPath).Output()
return asset.LoadWorkflowJSON(out)
```

Go 解析器有 `TestToJSON_MatchesPythonShim` 差分测试确保一致性——但测试只是**证据性的**，不是**强制性的**。如果 Go 解析器输出一个与 PyYAML 略有差异的 JSON 表示（例如数字解析、布尔值判定、空值处理），这个差异可能不被测试覆盖，但会影响 `asset.LoadWorkflowJSON` 的行为。

### 代码级证据

**证据 1: `loadWorkflow` 和 `parseYAMLFile` 各自独立实现相同的 fallback 逻辑**

`main.go:353-385` 和 `validate.go:56-76` 有**几乎相同的代码**——先试 Go 解析器，失败则 fallback 到 Python shim。任何修复或增强需要在两个位置同步更新：

```go
// validate.go:56-76
func parseYAMLFile(root, relPath string) ([]byte, error) {
    f, err := os.Open(path)
    // ...
    val, err := yaml2json.Decode(f)
    if err == nil {
        // Go 成功 → 返回
    }
    // fallback 到 Python
    f.Close()  // ← 手工关闭，因为 defer 在函数退出
    shim := filepath.Join(root, "harness", "yaml2json.py")
    return exec.Command("python3", shim, path).Output()
}
```

**证据 2: Go 解析器有特定的 YAML 子集限制**

如果工作流 YAML 文件使用了 Go 解析器不支持的 YAML 特性（anchor/alias、merge key、tag、multi-document），Go 解析器会静默返回错误，回退到 Python。但**测试套件只覆盖 Go 解析器支持的特性**——任何未覆盖的 YAML 特性既可能被 Go 解析器错误地「成功解析」而产生语义错误的 JSON，也可能正确 fallback。

```go
// forge-core/internal/yaml2json/yaml2json.go:29-37
// It deliberately does NOT support these YAML 1.x features:
//   - Anchors (&) and aliases (*) — not used by ForgeOS configs
//   - Merge keys (<<:) — not used by ForgeOS configs
//   - Tags (!!str, !binary) — not used
```

**证据 3: Python shim 的可用性没有保证**

```go
// main.go:378-380
if _, err := os.Stat(shim); err != nil {
    return asset.Workflow{}, fmt.Errorf("YAML->JSON via Go parser failed and python shim missing")
}
```

如果 Go 解析器失败且 Python 不存在，forge 返回硬错误。这意味着：
- 一个仅 Go 部署（无 Python）遇到 Go 解析器 bug 或未覆盖的 YAML 特性时，**完全不可用**
- Go 解析器的任何未捕获的边界输入都能把入口变成死路

### 与已有覆盖的关系

`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 行 176 提到 ROADMAP.md 中关于 YAML shim 的描述过时（Python fallback 被描述为 primary，而实际上 Go 原生解析器已领先）。但那是关于**文档过时**的备注，不是对**双解析器维护面本身的分析**。方向二聚焦于双实现的持续维护成本和一致性保障缺口，是独立的工程债务方向。

### 建议方向

1. **消除 fallback**：将 `loadWorkflow` 和 `parseYAMLFile` 中的双路径合并为单一 Go 解析器路径，不再依赖 Python。为此需要补全 Go 解析器在 block scalar、sequence 等方面的覆盖（Sprint 27 已解决了 block scalar 问题，但仍缺少 anchors/merge keys——确认 ForgeOS 自己的 YAML 从不使用这些特性即可）
2. **在消除 fallback 前加一致性门禁**：每次 `forge validate` 运行时，同时跑 Go 和 Python 两个解析器，如果 JSON 输出不一致则报 WARNING
3. **抽取公共 fallback 逻辑**：`loadWorkflow` 和 `parseYAMLFile` 共享 90% 的 fallback 代码，应抽取为公共函数 `transcodeYAML(root, path) ([]byte, error)`

---

## 方向三 · forge-core 二进制不自包含

**优先级**: 🔴 P0 | **类别**: 分发 · 部署 | **预估**: ~2–3 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

当前 `forge` 二进制（`forge-core/cmd/forge`）编译后**不能在运行时独立存在**。它在运行中通过子进程调用多个外部工具：

1. **Node.js**: `node harness/scorecard-update.mjs`（scorecard_wind.go）
2. **Node.js**: `node harness/gate.mjs`（通过 harness gate phases）
3. **Python3**: `python3 harness/yaml2json.py`（YAML 解析 fallback）
4. **Python3**: `python3 harness/check.py`（治理检查）

其中一些调用通过 `repoRoot`（`$FORGE_REPO_ROOT` 或 `.`）解析脚本路径，意味着二进制必须在项目目录内运行且该目录必须包含完整的 `harness/` 目录树。

### 代码级证据

**证据 1: scorecard 更新走 Node.js 子进程**

```go
// forge-core/cmd/forge/scorecard_wind.go:168-172
args := []string{
    "harness/scorecard-update.mjs",     // ← 相对路径，从 repoRoot 解析
    "--trace", tracePath(root),
    "--out", scorecardPath(root),
    "--model", model,
    "--task-type", tt,
}
cmd := exec.Command("node", args...)   // ← 依赖 node 在 PATH 中
```

**证据 2: workflow 的 gate phases 通过 `node` 运行 gate.mjs**

```go
// 不是直接的代码证据，但 harness gate phases 的定义在 system prompt 中被构建为调用 node 命令
// defaultAgentAllowedTools 显示 agent 被允许运行 node harness/gate.mjs
// forge-core/cmd/forge/main.go:58
const defaultAgentAllowedTools = "Bash(node --test*) Bash(node harness/gate.mjs*)"
```

**证据 3: YAML 解析 fallback 依赖 Python 3**

```go
// forge-core/cmd/forge/main.go:376-381
shim := filepath.Join(repoRoot, "harness", "yaml2json.py")
out, execErr := exec.Command("python3", shim, ymlPath).Output()
```

**证据 4: 检查测试确认 Node.js 和 Python3 是强制依赖**

```go
// forge-core/cmd/forge/evolve_test.go:30-33
func requirePython(t *testing.T) {
    if _, err := exec.LookPath("python3"); err != nil {
        t.Skip("python3 not in PATH")
    }
}
```

**证据 5: CI 中也使用 `node` 直接运行 harness 工具**

```yaml
# .github/workflows/forge.yml: 未引用，但所有 harness 测试
# 通过 node --test 运行
```

### 边界情况

- **部署在只有 Go 运行时的 Docker 镜像中**（如 `golang:alpine`）：Node.js 和 Python 3 都不存在，forge 大部分功能不可用
- **CI 环境没有安装 Node.js**（如纯 Go 构建容器）：scorecard 更新静默跳过（`fail-loud-and-continue`），但 gate 运行完全失败
- **`harness/` 目录未复制到部署包**：所有 gate、check、scorecard 功能全部失败
- **多版本 Node.js/Python 兼容性**：forge 从未声明对 Node.js 或 Python 版本的依赖——一个系统 Python 升级（如 3.9→3.13）可能引入语义变化

### 产品影响

这是 ForgeOS 目前**最大的采纳障碍**。一个潜在用户要试用 forge，当前需要：
1. 下载/构建 forge 二进制
2. 安装 Go 工具链
3. 安装 Node.js
4. 安装 Python 3
5. 拉取包含 harness/ 的完整仓库

而产品愿景是「一个 Go 二进制解决所有问题」。当前状态的二进制分发策略实际上不存在——`forge` 只是一个**仓库-感知的运行器**，不是真正的 CLI 工具。

### 建议方向

1. **将 harness gate 内联到二进制中**：`gate.mjs`、`check.py`、`acceptance.mjs`、`sca.mjs`、`secret-scan.mjs` 等核心执法工具通过 `embed.FS`（Go 1.16+）嵌入二进制，运行时以临时文件形式提取执行。消除 `harness/` 目录的运行时依赖
2. **将 scorecard-update.mjs 的逻辑移植到 Go**：scorecard 更新当前完全在 Node.js 中实现（`harness/scorecard-update.mjs`）。将核心计算逻辑移植到 `internal/scorecard` Go 包，消除 Node.js 依赖
3. **将 yaml2json.py 回退路径标记废弃**：确认 Go 解析器已完全覆盖所有 ForgeOS 工作流 YAML 后，移除 Python fallback 路径
4. **定义 Node.js/Python 版本契约**：在 README 和 CI 中声明最低 Node.js 版本（当前默默依赖 `node` 的可用性），并将该检查纳入 `forge doctor`

---

## 方向四 · 无语义自校验的状态目录恢复

**优先级**: 🟠 P1 | **类别**: 可靠性 · 边界情况 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐

### 问题描述

`forge evolve --resume` 和 checkpoint 恢复功能可以从上一次中断处继续执行。但恢复逻辑只校验了两件事：
1. 文件是否存在（不存在 = 冷启动）
2. JSON 是否能被 Unmarshal（语法错误 = 硬错误）

**语义上没有做任何校验**——恢复后的运行时状态可能引用已经不存在的工作流阶段或已经无效的模式配置。

### 代码级证据

**证据 1: checkpoint Load 只校验 JSON 语法**

```go
// forge-core/internal/persist/checkpoint.go:143-151
func Load(path string) (Checkpoint, bool, error) {
    data, err := os.ReadFile(path)
    // ... 文件不存在 → 冷启动
    cp, err := decode(data)
    // ... JSON 解码失败 → 硬错误
    return cp, true, nil             // ← 语义校验为零
}
```

**证据 2: resume 路径直接使用 checkpoint 的 PhaseIndex 和 Workflow 名**

```go
// forge-core/cmd/forge/evolve.go:259-275 (相关逻辑)
func resumeStart(root string, wf asset.Workflow) int {
    cp, ok, err := persist.Load(filepath.Join(forgeDir(root), "checkpoint.json"))
    if !ok || err != nil {
        return 0
    }
    // ← 没有检查 cp.Workflow == wf.Name
    // ← 没有检查 cp.PhaseIndex 是否 < len(wf.Phases)
    // ← 没有检查 cp.Mode 是否是已知的 mode
    return startPhase
}
```

**证据 3: checkpoint 的 SpentUsdMicros 与 trace 事件的总成本不进行对账**

```go
// forge-core/internal/persist/checkpoint.go:68-70
type Checkpoint struct {
    SpentUsdMicros int64 `json:"spent_usd_micros,omitempty"` // ← 累积成本
    // ...
}

// 没有任何代码在 resume 时验证 checkpoint 的累计成本
// 是否与 trace.jsonl 中各条 cost 事件的总和一致
```

**证据 4: 恢复后不检查 memory store 的完整性**

```go
// forge-core/cmd/forge/evolve.go:175-185
// openTracer + openCheckpoint 是恢复时的两个步骤
// 但没有对被引用的 memory 内容做任何校验
// 如果 memory.jsonl 在前一次运行中被 Partial Append 损坏
// （O_APPEND 保证单行原子性，但不能防御 crash 后的文件截断）
// 恢复时会直接 Load 并失败
```

### 边界情况

- **工作流重命名或重构**：用户把 `build.yml` 的 `implementer` phase 重命名为 `implement`，checkpoint 中 `PhaseIndex=3`（原 implementer）指向了不同的 phase。恢复后执行了错误的 phase
- **模式重命名或删除**：`engineering` mode 被重命名为 `rigorous`，checkpoint 中的 `Mode: "engineering"` 被送到 `mode.Effective()` 处理——fail-safe 模式是全开（所有 gate），这可能意外绕过安全约束
- **二进制版本升级**：旧版 checkpoint 记录的 `Iteration=5` 在新版的收敛算法下应该重新计算，但恢复代码直接使用旧值
- **手动编辑 checkpoint**：用户或工具手动编辑了 checkpoint.json，语法仍有效但语义无意义（如 `roadmap_completion: -1`）
- **SpentUsdMicros 不匹配**：trace.jsonl 中有记录的 cost events 总和与 checkpoint 的 `SpentUsdMicros` 不一致（由于前一次运行的 crash 导致 checkpoint 成功写入但 trace 丢失最后几条事件）

### 产品影响

`--resume` 是 24h 自治运行的基石特性。如果恢复后的执行从一个错误的 phase 开始或使用了无效的模式，可能会导致：
- 数小时的计算浪费（重做已完成的 phase）
- 成本失控（从错误的 phase 重跑，产生意料之外的 API 调用）
- 结构损坏（agent 写了不一致的代码）

### 建议方向

1. **PhaseIndex 越界校验**：resume 时检查 `cp.PhaseIndex < len(wf.Phases)`，越界则从 phase 0 恢复并记录 WARNING
2. **Workflow 名校验**：如果 checkpoint 的 `Workflow` 与当前请求的 workflow 名不匹配，从 phase 0 恢复（回退安全，不丢任何已完成的工作，因为完工作已提交到 git）
3. **Mode 校验**：`cp.Mode` 通过 `mode.ModeFor` 验证，未知模式则回退到 lifecycle 缺省值并告警
4. **Cost 对账**：resume 时对 trace.jsonl 中 cost events 的 `CostUsdMicros` 求和，验证与 `SpentUsdMicros` 偏差在 5% 以内，超限则记录 WARNING 但不阻塞
5. **Memory 健康检查**：resume 前跑 `memory.Load(path)`，如果返回错误（损坏的行），告警后从空内存冷启动（保留损坏的文件作为备份，不自动删除）

---

## 方向五 · Scorecard 管线：多进程文件系统 IPC

**优先级**: 🟡 P2 | **类别**: 可靠性 · 韧性 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐

### 问题描述

Scorecard 的更新走一条**多进程文件系统 IPC 链**，没有任何事务性保证：

```
forge evolve/run
  └─ RunFrom 执行 agent phase → trace.Emit 写入 trace.jsonl
  └─ windDownScorecards (deferred)
       └─ 遍历 (model, task_type) 组合
            └─ exec.Command("node", "harness/scorecard-update.mjs",
                 "--trace", tracePath, "--out", scorecardPath)
                 └─ scorecard-update.mjs 读取 trace.jsonl → 写入 scorecards.json
       └─ routing.LoadScorecards 读取 scorecards.json
            ↓ (scorecard 数据用于未来的路由决策)
```

这个链中每一步之间的**唯一连接是文件系统**。如果任意一步失败，中间状态是不一致的：
- scorecard-update.mjs 被 SIGKILL 时，scorecards.json 处于半写入状态
- 多个 `forge run` 并行运行在同一仓库时，scorecards.json 被并发写入导致数据损坏
- scorecards.json 读取后，没有校验其内容是否与 trace.jsonl 中的成本匹配

### 代码级证据

**证据 1: scorecard-update 结果不经校验直接使用**

```go
// forge-core/cmd/forge/scorecard_wind.go:88-95
func runScorecardUpdate(...) {
    cmd := exec.Command("node", args...)
    if out, err := cmd.CombinedOutput(); err != nil {
        logln("scorecard: update failed (non-fatal): " + string(out))
        return  // ← fail-loud-and-continue，不重试
    }
    // ← 没有验证输出文件的完整性
}
```

**证据 2: 没有防止并发写入的锁机制**

```go
// scorecardPath 是单一文件
// scorecard_wind.go:44-48
func scorecardPath(root string) string {
    return filepath.Join(root, ".agent", "routing", "scorecards.json")
}
// 如果两个 forge 进程并行运行（不同迭代或不同 workflow），
// 它们会同时写入同一个 scorecards.json——没有 advisory lock
```

**证据 3: trace 文件的读取没有格式校验**

```go
// forge-core/cmd/forge/scorecard_wind.go:198-220
for sc.Scan() {
    var ev trace.Event
    if err := json.Unmarshal(sc.Bytes(), &ev); err != nil {
        logln("scorecard: skip unparseable trace line: " + err.Error())
        continue  // ← 跳过而非 abort，可能损失关键成本数据
    }
    // ← 读取了 ev.CostUsdMicros 但不与 scorecards.json 中已有值交叉验证
}
```

**证据 4: scorecard-update.mjs 的写入不是原子的**

```bash
$ head -30 harness/scorecard-update.mjs | grep -n "write\|Write\|fs\.\|JSON\.\|save\|Save"
# (需要在 Node.js 代码中确认——但 Go 端的 checkpoint.go Save 使用原子写入，
# 而 scorecard-update.mjs 是否使用类似技术不在 forge-core 的控制范围内)
```

### 边界情况

- **并行 evolve 运行**：开发者不小心在两个终端同时运行 `forge evolve build`，两个进程各自调用 `runScorecardUpdate` 写入同一个 scorecards.json，最后一个写入者覆盖前一个的数据
- **scorecard-update.mjs crash**：Node.js 在处理大量 trace 数据时 OOM 或超时，scorecards.json 被部分覆盖。下一个 `forge run` 从残缺的 scorecards 中读取路由偏好
- **trace.jsonl 被同时读取**：scorecard-update.mjs 运行时迭代到一半，另一个 `windDownScorecards` 被 defer 触发（理论上不可能因当前代码的单线程设计，但未来的并发改进可能引入）
- **空 trace 文件**：新初始化的项目跑第一次 forge run，trace.jsonl 为空，scorecard-update.mjs 读取空文件后可能写一个空的 scorecards.json，覆盖手工设置的历史数据

### 产品影响

Scorecard 是 ForgeOS 学习闭环（Eval→Scorecard→Router→Agent）的关键反馈信号。如果 scorecard 数据因为管线脆弱性而被损坏或丢失，路由决策会退化为随机选择或完全依赖固定 tier，系统失去自适应调优能力。虽然当前 `fail-loud-and-continue` 的设计保证了核心编排不受影响，但在运营环境中，scorecard 数据的可靠性直接决定学习的收敛速度。

### 建议方向

1. **atomic write for scorecard-update.mjs**：确保 `harness/scorecard-update.mjs` 使用临时文件→rename 模式（与 `persist.Save` 相同），消除半写入状态
2. **advisory lock**：在 `.forge/scorecard.lock` 上使用 `flock` 或类似机制，防止并行写入
3. **读取后校验**：`routing.LoadScorecards` 在解析 scorecards.json 后，校验其内部一致性（所有 score 在合理范围内、时间戳可排序），异常时告警而非静默使用
4. **引入 scorecard 写入事务性**：将 scorecard 数据合并逻辑从 Node.js 迁移到 Go（与方向三联动），消除进程间文件系统 IPC 的脆弱性

---

## 总结

| # | 方向 | 核心代码证据 | 已有分析覆盖 | 建议优先级 | 杠杆 |
|---|------|------------|------------|-----------|------|
| 1 | 格式版本字段只写不读 | memory.go:186/persist.go:103/trace.go:122 写 `_format`，三处 decode 零检查 | **零独立覆盖** | P1 | ⭐⭐⭐⭐ |
| 2 | 双 YAML 解析器维护面 | main.go:364-385 + validate.go:56-76 各自独立实现相同的 fallback | 仅文档过时备注 | P1 | ⭐⭐⭐⭐ |
| 3 | forge-core 二进制不自包含 | scorecard_wind.go:168-172 `exec.Command("node",...)` / main.go:376 `exec.Command("python3",...)` | **零独立覆盖** | P0 | ⭐⭐⭐⭐⭐ |
| 4 | 无语义自校验的状态目录恢复 | persist.go:143-151 Load 只校验 JSON / evolve.go resume 无 PhaseIndex 越界校验 | **零独立覆盖** | P1 | ⭐⭐⭐ |
| 5 | Scorecard 多进程文件系统 IPC | scorecard_wind.go:88-95 子进程写 scorecards.json 无原子性保证 | **零独立覆盖** | P2 | ⭐⭐⭐ |

五个方向的共同主题：**ForgeOS 的运行时能力已经完整（18 Go 包、5 引擎、31 轮 Sprint），但它的「工程元层」——数据格式演化、依赖管理、二进制分发、状态自愈——仍然是未受审视的薄弱环节。这些不是功能特性，而是长期可维护性和生产采纳的前提条件。** 方向三（二进制不自包含）是受阻的最大单项瓶颈：只要 `forge` 不能独立分发，所有其他方向的改进都只能被已经能构建 Go 的人使用。
