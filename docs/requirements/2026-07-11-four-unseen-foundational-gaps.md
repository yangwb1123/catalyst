# ForgeOS — 四个被忽视的基础设施缺口：子进程、示例维护、桥接契约与配置面

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 逐包扫描 forge-core（18 Go 包, 17 子命令, 210+ 源文件）、harness（41 模块）、  
>    `.agent/` 完整治理骨架、examples（go-taskd + url-shortener）、`.github/workflows/forge.yml`  
> 2. **差异化验证**: 对每个方向的核心关键词组合，在全部 ~130+ 篇 `docs/requirements/`、  
>    ~40 篇 `docs/analysis/`、`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`、31 轮 Sprint 演进记录中  
>    做全文检索，确认该方向的**系统性论述**从未作为独立方向展开  
> 3. **纪律**: 不编写任何代码。每个方向附 `file:line` 代码证据、边界场景、与已有分析的明确边界  
> 4. **共同主题**: 这四个方向都不是「新功能」，而是**基础设施债**——目前不触发，但它们在悄悄侵蚀  
>    项目的长期可维护性、可观测性和生产安全性  
>
> **日期**: 2026-07-11

---

## 全景：已有覆盖规模

ForgeOS 项目已有 **~130 篇 requirements 分析 + ~40 篇 analysis 分析 + 31 轮 Sprint 演进 + 一份功能需求审计**。以下域已被深度覆盖，本文不重复：

| 域 | 关键文档 | 覆盖度 |
|---|---|---|
| 编排引擎（串/并行/loop-back/checkpoint/mode×lifecycle 全部 7 维度） | `strategic-extensions-v22~v33.md`, `novel-five-perspectives-2026-07-10.md` | 完备 |
| 模型路由（Agent→Tier / Score→Tier / BudgetAdjust / HistoryTiebreak / Opus 安全下限） | `expansion-core-five.md`, `expansion-directions-v4.md` | 完备 |
| 四维真点火安全护栏（递归/数量/时间/输出容量/run-level budget） | `strategic-expansion-and-edge-cases.md`, `expansion-next-frontier.md` | 完备 |
| 学习闭环（trace/scorecard/memory/converge 全信号） | `hidden-feedback-and-pipeline-gaps.md`, `expansion-core-five-2026-07-01.md` | 深度 |
| 治理执法（arch-check 8 检查 / check.py 10 检查 / gate.mjs / secret-scan / SCA 框架） | `strategic-extensions-v15.md`, `roadmap-blindspots.md` | 完备 |
| 结构债（Phase 膨胀 / 函数拆分 / 包文件数 / 认知负荷） | `eighth-wave-adr-decay.md`, `go-runtime-health.md` | 深度 |
| 执行语义（重试/退避/进程组/幂等/原子 checkpoint / on_rejected） | `fifth-wave-operational.md`, `strategic-expansion-v21.md` | 深度 |
| 生产就绪（超时/取消/健康检查/自适应收敛/SCA 框架/secret-scan） | `expansion-production-readiness.md`, `forgotten-five-foundations.md` | 深度 |
| 功能需求审计（DONE ~90 条 / GAP 14→收口 / BLOCKED-EXTERNAL 3 / DEFERRED-BY-DESIGN ~15） | `FUNCTIONAL_REQUIREMENTS_AUDIT.md` | 完备 |
| 持久化 Schema 版本化 / yaml2json 解析器可靠性 / 并行 .forge/ 并发 | `2026-07-11-scan-five-codegrounded-systemic-frontiers.md` | 已有覆盖 |
| 二进制分发 / 状态目录灾难恢复 / 统一结构化输出 / 多会话热加载 / 状态数据生命周期 | `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` | 已有覆盖 |
| fork-bomb / setsid 逃逸 / 进程孤儿 / 跨平台可移植性 | `2026-07-11-forgeos-five-unbuilt-product-architectural-extensions.md` | 已有覆盖 |
| fuzz / property-based / 随机测试 / 解析器差分测试 | `forges-five-hidden-product-quality-gaps.md`, `expansion-production-readiness.md` | 已有覆盖 |

**以下四个方向落在以上所有覆盖的间隙中**——不是「缺失的功能组件」，而是**基础设施债**：当前不触发故障，但长期积累会影响可维护性、可观测性、可靠性和安全性。

---

## 方向一 · 跨示例回归检测缺口（Dogfood 示例成为沉默的债）

> **「我们 dogfood 了 url-shortener 来验证治理——但没人知道下次 forge-core 改动后它是否还能通过 `forge accept`。」**

**优先级**: 🟠 P2（非阻塞，但随示例增多指数恶化）  
**类别**: 可维护性 · 质量保障  
**估算**: ~1 sprint  
**已有覆盖**: **零**——全文检索 `example.*regression` / `dogfood.*regression` / `example.*CI` ——  
在全部 ~130 篇需求分析、~40 篇 analysis、FRA 中，无一篇将「示例与核心之间的回归保护缺失」作为独立方向展开。  
FRA 的第 48 行将 url-shortener 列为 DONE，但它只证明了**创建时**的治理通过，未讨论**持续维护**。

### 问题描述

ForgeOS 有两个 dogfood 示例应用：
- `examples/url-shortener/`（Node.js，5 测试文件，被 `forge accept` 实际门控）
- `examples/go-taskd/`（Go，4 测试文件，纯 Go 标准库 Clean Architecture 演示）

它们被创建的目的是证明 ForgeOS 的端到端能力。但 `.github/workflows/forge.yml`（当前 CI 配置）**从不运行它们**：

```yaml
# .github/workflows/forge.yml:1-57（最终步骤）
      - name: forge-core tests (zero-dep Go runtime)
        run: go -C forge-core test ./...
      - name: harness unit tests
        run: node --test harness/
      - name: forge run build --executor dry (end-to-end orchestration smoke test)
        run: |
          go -C forge-core build -o /tmp/forge-test ./cmd/forge
          /tmp/forge-test run build --executor dry --root $PWD
```

CI 中没有任何步骤执行 `forge accept` 在示例应用上，也没有任何构建或测试示例的命令。

### 边界场景

1. **Harness 升级破坏示例**：`gate.mjs` 或 `arch-check.mjs` 的行为变化可能让 url-shortener 的 39 测试突然失败。当前无任何检测机制。
2. **forge-core 的 CLI 接口变化**：如果 `forge run` 或 `forge accept` 的参数解析或默认行为改变，示例的构建/测试可能静默失效。
3. **示例依赖版本漂移**：`go-taskd/go.mod` 使用的是 `go 1.22` + `example/taskd` module path（纯 stdlib，无外部依赖），所以目前无依赖漂移风险。但 url-shortener 使用 Node 内置 `node:test`，当 forge 升级 Node 版本时行为可能变化。
4. **新的示例加入后无任何保护**：如果未来加入 `examples/typescript-app`，当前没有机制确保它们被自动纳入 CI。

### 与已有分析的边界

| 已有方向 | 区别 |
|---------|------|
| `forges-five-hidden-product-quality-gaps.md` 的 fuzz/property-based 测试 | 聚焦解析器质量；本文聚焦**示例作为系统级回归探测器** |
| `FUNCTIONAL_REQUIREMENTS_AUDIT.md` DONE 行的 url-shortener | 只证明创建时治理通过；本文聚焦**持续回归保护** |
| 全部 `expansion-*` / `structural-*` 系列 | 讨论的是 forge-core 自身的治理；本文讨论的是**示例作为外部消费者的回归保护** |

### 落地路径

```
# 最小可行方案（< 10 行 YAML）
# 在 CI 的 forge-core 编译后，依次对每个示例跑：
cd examples/url-shortener && node -C forge-core ../../forge-core/forge run build --executor dry
cd examples/go-taskd && go -C forge-core build ... && ../../forge-core/forge run build --executor dry
node harness/acceptance.mjs --root examples/url-shortener
node harness/acceptance.mjs --root examples/go-taskd
```

扩展方案：加入 `forge verify-examples` 子命令，按 `.forge-examples.yml` 清单依次验证所有注册示例。

### 产品杠杆

⭐⭐⭐⭐（四星）——成本极低（一天搭建），但防止的 regression 类型（harness/CLI 行为变化静默破坏已验证的 dogfood）是目前完全盲区的。

---

## 方向二 · 子进程资源核算与可观测性

> **「我们知道 agent 花了 $0.18 和 2.6 秒——但我们不知道它用了多少 CPU、占了多少内存、产生了多少 IO。」**

**优先级**: 🟠 P2（非阻塞，但长运行中隐式资源泄漏影响可靠性）  
**类别**: 可观测性 · 运维  
**估算**: ~1.5 sprints  
**已有覆盖**: **接近零**——已有分析覆盖了进程孤儿逃逸（`2026-07-11-forgeos-five-unbuilt-product-architectural-extensions.md`），  
但**没有人讨论子系统进程级资源核算**。全文检索 `subprocess.*account` / `process.*resource.*track` /  
`agent.*cpu.*track` ——零系统性论述。`forgeos-five-architect-product-perspective-2026-07-10.md` 第五行  
提到「gate 风暴中子进程资源耗尽」，但聚焦于**并发 gate 数量**而非**单子进程的资源可见性**。

### 问题描述

`CommandExecutor` 负责 spawn agent 和 gate 子进程。当前可以追踪的内容：

```go
// forge-core/internal/orchestrator/command_executor.go:114-138
// 追踪项: 退出码、超时、输出截断、成本（通过 Observe 回调）、退出错误类型
// 不追踪项: CPU 时间、峰值内存、子进程数、打开文件句柄数、磁盘 IO
```

具体来说，当前 `trace.Event` 记录的是：

```
{kind: "agent", name: "implementer", status: "PASS", duration_ms: 2640, cost_usd_micros: 184100, model: "sonnet"}
```

**缺失的维度**：
- `cpu_ms`：agent 消耗了多少用户态 + 内核态 CPU 时间
- `peak_memory_kb`：agent 进程的峰值 RSS
- `num_probes`：gate 子进程衍生数（arch-check 内部 spawn 了 git/grep）
- `io_read_bytes` / `io_write_bytes`：子进程磁盘 IO 量
- `open_fds`：子进程打开的文件句柄数

### 代码级证据

**证据 A: CommandExecutor 的 Observe 回调只有 output + latency**

```go
// forge-core/internal/orchestrator/command_executor.go:81-83
Observe func(phase, output string, latency time.Duration)
// ↑ 只有名字、文本输出、墙钟延迟。
// 没有 CPU/内存/IO 的资源使用信息。
```

**证据 B: trace.Event 没有资源字段**

```go
// forge-core/internal/trace/trace.go:58-79
type Event struct {
    // ... 现有字段: Kind, Name, Status, DurationMs, CostUsdMicros, Model, Detail
    // 没有: CPUMs, PeakMemoryKB, IOStats, OpenFDs
}
```

**证据 C: OS 级别可获取但未被收集**

```go
// forge-core/internal/orchestrator/command_executor_unix.go:49-58
func setupProcessGroup(cmd *exec.Cmd) {
    cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
}
// syscall 包可用，但从未调用 getrusage(2) 或 wait4(2) 的 rusage 结构
//
// 系统调用已存在:
//   - wait4(pid, &status, 0, &rusage) → 返回 CPU 时间 + 缺页 + 上下文切换
//   - getrusage(RUSAGE_CHILDREN, &ru) → 累计子进程资源用量
// 两者都可通过 Go 的 syscall.Wait4 / syscall.Getrusage 调用，
// 但 forge-core 的 cmd.Wait() 从不收集 rusage。
```

### 边界场景

1. **长 evolve 循环的累积泄漏检测**：24h evolve (~120 agent 调用) 中，如果每个子进程泄漏 1MB RSS 碎片，终端可能累积 ~120MB 不可回收内存——`forge status` 当前无法报告这一信息。
2. **子进程 fork 炸弹逼近内存上限时**：递归 agent 有 `MaxDepth` 保护，但如果每个子进程贪心地分配内存，系统 OOM killer 可能杀掉 forge 而非违规子进程——没有 `memory_high_water_mark` 预兆信号。
3. **gate 风暴中的资源隔离**：`forge accept` 衍生 `go test` / `node --test` / `arch-check`（内部 git grep）多个子进程。在 CI 共享 runner 上，这些子进程争用 CPU/内存，但 forge-core 不知道争用是否存在。
4. **容器化环境中的资源限幅告警**：在内存 512MiB 的容器中跑 forge，一个 agent 子进程可能被 OOM kill——当前日志只有 `signal: killed`，没有区分 OOM kill 与外部 SIGTERM。

### 落地路径

```
1. cmd.Wait() → syscall.Wait4() 收集 rusage（CPU time + page faults + ctx switches）
2. /proc/<pid>/status 读取 VmPeak（峰值虚拟内存）— Linux only，optional
3. 新增 trace.Event 字段（omitempty 向后兼容）:
   - cpu_ms: int64
   - peak_memory_kb: int64 (optional)
   - oom_killed: bool (从 signal 判断)
4. forge status --resources 报告 session 累计资源使用
5. 按 model × phase 聚合资源趋势记入扩展 scorecard
```

### 产品杠杆

⭐⭐⭐（三星）——不是紧急问题，但对于一个宣称「24h 无人值守」的系统，没有子进程资源可见性意味着**长时间运行中的退化只能靠猜**。成本追踪已落地，CPU/内存追踪是对称的自然扩展。

---

## 方向三 · Harness 桥接契约测试

> **「gate.mjs 改了输出格式——但 gate.go 的解析器没人通知它。」**

**优先级**: 🟡 P3（当前不触发，但触发时静默且难以调试）  
**类别**: 可靠性 · 测试基础设施  
**估算**: ~1 sprint  
**已有覆盖**: **零**——全文检索 `harness.*drift` / `gate.*bridge.*test` / `shell.*out.*test` /  
`Go.*bridge.*pars` 在所有需求/分析文档中**零命中**。`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 第 187 行  
提到 `mode_gating` 漂移守卫，但那是**治理资产漂移**（YAML vs modes.yml），不是**工具输出格式漂移**（harness stdout → Go 解析器）。

### 问题描述

ForgeOS 的架构有一个双层结构：

```
forge-core (Go)                    harness (Node.js / Python)
  ┌──────────────┐                   ┌─────────────────┐
  │ gate.go      │ ──exec.Command──▶ │ gate.mjs        │  stdout: "PASS: 374 files"
  │ check.go     │ ──exec.Command──▶ │ check.py         │  stdout: "[PASS] check_agent_refs"
  │ acceptance   │ ──exec.Command──▶ │ acceptance.mjs   │  stdout: multi-line report
  │ yamlpath.go  │ ──exec.Command──▶ │ yaml2json.py     │  stdout: JSON
  └──────────────┘                   └─────────────────┘
```

**每个桥接点都有隐式输出格式契约**，但没有任何测试验证这些契约。如果 harness 工具的 stdout 格式变化，Go 层的消费方会静默误解。

### 代码级证据

**证据 A: gate.go 对工具 exit code 的隐式依赖**

```go
// forge-core/internal/gate/gate.go:97-112
func run(root, name, cmd, args string) Result {
    c := exec.Command(cmd, parsed...)
    c.Dir = root
    out, err := c.CombinedOutput()
    // ...
    if err != nil {
        // exit code != 0 → FAIL; 但 exit code 可能因工具 bug 或环境问题而非 test failure
        return Result{Name: name, OK: false, Status: StatusFail, Output: string(out)}
    }
    return Result{Name: name, OK: true, Status: StatusPass, Output: string(out)}
}
```

仅依赖 exit code（0/非0）。如果工具因自身 bug 而非 test failure 返回非零，gate 报告 FAIL——这是正确行为。但如果工具的 stdout 中出现了人类可读的错误、警告、或非标准输出，Go 层没有做任何**语义验证**。

**证据 B: check.py stdout 被直接透传**

```go
// forge-core/internal/gate/gate.go:63
func Check(root string) Result {
    return run(root, "check", "python3", []string{filepath.Join(root, "harness", "check.py"), root})
}
```

`check.py` 的 stdout 被直接 print 到 forge 的 stdout——假设工具的 stdout 是人类可读的。如果 check.py 在某个版本改为 JSON 输出（完全合理的改进），gate.go 的行为不会出错（exit code 仍然正确），但人类看到的输出格式会突变，且任何试图解析输出的 CI 脚本会静默中断。

**证据 C: yamlpath.go 对 yaml2json.py JSON 输出格式的隐式依赖**

```go
// forge-core/internal/yamlpath/yamlpath.go:159-172
func resolveRef(...) (any, error) {
    out, err := exec.Command("python3", shim, absFile).Output()
    // ...
    var root any
    if err := json.Unmarshal(out, &root); err != nil {
        return nil, fmt.Errorf("yamlpath: decode JSON from %s: %w", absFile, err)
    }
    return walkPath(root, strings.Split(r.Path, "."))
}
```

这里的依赖是最脆弱的——如果 yaml2json.py 的输出从 `{"key": "val"}` 变为 `[{"key": "val"}]` 或加入 wrapper，`json.Unmarshal` 会直接失败。但当前唯一的测试覆盖是单元测试，只测了**与自身**的一致性，从未测过与 Python shim 输出的对比。

**证据 D: 历史先例——yaml2json 的 block-scalar 损坏**

Sprint 27 修复的 `yaml2json` block-scalar bug 是一个活生生的教训：Go 的 yaml2json 替换了 Python shim 作为主要解析路径，但它的输出与 PyYAML 有差异（`"> "` 注入到值中），差分测试只 `t.Logf` 不 `t.Errorf`——测试存在但**从未真正断言**。修复后真正的差分断言才被启用。

### 边界场景

1. **gate.mjs 的 stdout banner 变化**：当前 gate.mjs 输出 `PASS: 374 files`，如果改为 `OK: 374 files`，gate.go 的 `delegate()` 函数照常工作（exit code 仍是 0），但任何 grep/log 解析脚本中断。
2. **check.py 新增一行输出**：当前 check.py 输出 `[PASS] ...`，每行对应一个检查。如果新增一行带 `[WARN]`（注意不是 FAIL），exit code 仍是 0，Go 层照常 PASS——但 `[WARN]` 行被静默吞入 stdout，用户可能在 CI 日志中误认为全部通过。
3. **acceptance.mjs 改为 JSON 输出**：当前 acceptance.mjs 输出人类可读报告。如果未来希望 CI 以结构化格式消费它，Go 层只透传 stdout——真正的格式变化需要同时更新 Go 层消费者。
4. **yaml2json.py 替换实现**：当前用 PyYAML。如果未来换用 `ruamel.yaml`，其输出可能在某些边缘情况下有细微差异。当前没有任何回归测试检测这种差异。

### 落地路径

为每个桥接点建立**契约测试**——具体来说：

```
tests/harness_bridge/
  gate_contract_test.go      — spawn gate.mjs, 验证 stdout 格式 + exit code 语义
  check_contract_test.go     — spawn check.py, 验证 [PASS]/[FAIL] 格式
  yaml2json_contract_test.go — spawn yaml2json.py, 将结果与 Go yaml2json 对比
  acceptance_contract_test.go — spawn acceptance.mjs, 验证报告格式 + exit code
```

这些不是端到端集成测试——它们是**格式契约守卫**。它们验证的是「Go 层对工具输出的假设仍然成立」，而不是「工具的逻辑正确」。

### 产品杠杆

⭐⭐（两星）——触发概率低，但触发时**百分之百静默**，且调试路径是跨语言跨进程的。一个 bridge 契约测试套件是对历史上已经发生过（yaml2json block-scalar）的问题的再保险。

---

## 方向四 · 配置面安全分析（多源配置的隐式攻击面）

> **「四个配置源、不同优先级、不同验证深度——一个参数就能静默关掉安全门。」**

**优先级**: 🔴 P1（安全相关，应优先评估）  
**类别**: 安全 · 配置管理  
**估算**: ~1.5 sprints（含安全审计 + 加固）  
**已有覆盖**: **接近零**——全文检索 `config.*attack` / `config.*security` / `配置.*攻击` /  
`parameter.*inject` / `env.*inject` 在所有需求/分析文档中**没有系统性安全分析**。  
`expansion-directions-v14-operational-trust.md` 标题包含"operational trust"但讨论的是  
Temporal/workflow 的可信执行，而非配置面本身的攻击面。

### 问题描述

ForgeOS 的运行时参数从 **4 个独立来源** 流入，每个来源的验证深度和信任假设不同：

| 来源 | 示例 | 验证 | 信任假设 |
|------|------|------|---------|
| CLI 标志 | `--mode explorer --max-agent-calls 9999` | flag.Parse() 类型级验证 | 调用者知悉并同意 |
| 环境变量 | `FORGE_REPO_ROOT` / `FORGE_AGENT_DEPTH` | 读字符串，无结构验证 | 环境未被篡改 |
| project YAML | `.agent/project.yml` 的 `mode:` / `lifecycle:` | 行扫描取字符串 | git-tracked，但可被直接编辑 |
| policy YAML | `.agent/policies/modes.yml` 的阈值 | Python check.py 校验，Go mode.go 硬编码解析 | 同 YAML 解析器质量 |

**攻击面一：参数注入通过环境变量绕过 CLI 约束**

```go
// forge-core/internal/gate/gate.go:37-42
func RepoRoot(root string) string {
    if root != "" {
        return root
    }
    if env := os.Getenv(EnvRoot); env != "" {
        return env
    }
    return "."
}
```

设计意图是便利性——如果忘记传 `--root`，从环境变量读取。但攻击者可设置 `FORGE_REPO_ROOT` 指向另一个项目，使 `forge run` 作用于非预期仓库。没有来源标记（"来自 CLI" vs "来自 env" vs "来自默认值"）。

**攻击面二：mode/lifecycle 通过 project YAML 静默提权**

```go
// forge-core/cmd/forge/main.go:314-320
func resolveLifecycle(o runOpts) string {
    if o.lifecycle != "" {
        return o.lifecycle   // CLI 指定优先
    }
    if v := projectYAMLValue(o.root, "lifecycle"); v != "" {
        return v             // 来自 project.yml
    }
    return "mvp"
}
```

`resolveLifecycle` 从 `project.yml` 读取 `lifecycle`。如果攻击者将 `lifecycle` 改为 `idea`（最宽松的模式对应 `modes.yml` 中 lifecycle floors 最宽松的策略），可以绕过 production 级别的 gate 强制和 reviewer 要求。虽然 `Effective()` 的 fail-safe 默认行为是「未知值取最严格」，但 `idea` / `mvp` / `growth` 都是有效值——它们**确实是**降低安全门槛的手段。

**攻击面三：FORGE_AGENT_DEPTH 可被子进程覆写**

```go
// forge-core/internal/orchestrator/command_executor.go:261-269
// agentDepthEnv = "FORGE_AGENT_DEPTH"
depth, _ := strconv.Atoi(os.Getenv(agentDepthEnv))
if depth >= maxDepth {
    return 0, fmt.Errorf("...recursion limit...")
}
childEnv := append(os.Environ(), fmt.Sprintf("%s=%d", agentDepthEnv, depth+1))
```

这是递归防护。但如果顶层 agent 被注入恶意 prompt，它可以在 spawn 子进程前 `unset FORGE_AGENT_DEPTH`，则子进程收到"0"并正常启动——递归防护被绕过。当前代码注释也已承认这一点（`adversarial env tampering` 不在 v1 范围内）。

**攻击面四：`--approved` 标志无认证**

```go
// forge-core/cmd/forge/main.go:265
fs.BoolVar(&o.approved, "approved", false, "supply the human-approval signal...")
```

`--approved` 是一个 CLI 布尔标志。任何能运行 `forge` 的人都可以传 `--approved` 来绕过 human_gate（设计→build 审查）。没有签名、没有来源追溯、没有审计日志说"谁在什么时候批准了什么"。

### 边界场景

1. **CI runner 上 env 污染**：如果 CI 系统设了 `FORGE_REPO_ROOT=/malicious/project`，后续 `forge run` 静默作用于恶意项目。
2. **.env 文件泄露**：如果 `.env` 包含 `FORGE_AGENT_DEPTH=99` 或 `FORGE_REPO_ROOT=/different/project`，forge 行为静默改变。`secret-scan.mjs` 会扫描 `.env` 文件，但只检测 credential 模式，不检测 forge 特定的 env 配置。
3. **git 子模块中的 project.yml 被篡改**：如果项目依赖 git submodule 共享 `.agent/` 模板（ADR-0003），恶意子模块的 `project.yml` 可设 `lifecycle: idea` 以降低目标项目的治理严格度。
4. **`--max-agent-calls 0` 的风险**：默认值 0 = unbounded（无限）。文档注释说"向后兼容"，但新用户可能误以为 0 是禁用而非无限。
5. **projectYAMLValue 的行扫描绕过**：`main.go:327-340` 的 `projectYAMLValue` 使用行扫描读取 `mode:` / `lifecycle:`。如果 project.yml 中包含 `mode: explorer # 绕过 production gate`，注释后的空格不影响取值——"explorer"仍然是有效值，降低安全级别。

### 落地路径

```
阶段一：溯源审计（1 week）
  审计所有 env/config 读取点，标记：
  - 必须验证来源（CLI / env / YAML / default）
  - 可被恶意覆写且产生安全影响（lifecycle/mode/approved/agent-depth）
  - 安全关键参数应被锁定（production 模式下某些参数不可被 project.yml 松绑）

阶段二：安全加固（1 sprint）
  - `--approved` 标记增加日志记录（谁、何时、哪个 workflow）
  - project.yml 的 mode/lifecycle 增加校验白名单（只允许 modes.yml 声明的值）
  - FORGE_AGENT_DEPTH 增加进程内计数器（env 只作通信，不作权威来源）
  - `--max-agent-calls` 禁止 0 值在生产模式下（强制显式上限）
  - env 来源增加 warning 日志（"FORGE_REPO_ROOT set from environment"）

阶段三：文档与治理（持续）
  - 在 .agent/DECISIONS.md 记录"哪些配置源是受信任的"决策
  - forge doctor 增加配置安全检查（检测危险 env 组合）
  - forge validate 检查 project.yml 中 mode/lifecycle 是否在允许范围内
```

### 产品杠杆

⭐⭐⭐⭐（四星）——安全面分析的 ROI 很高：当前没有已知攻击事件，但**代码中已有多个可被利用的模式**。部分加固（如 `--approved` 日志、project.yml mode 校验）只需几行代码，但防御效果显著。

---

## 总结

| # | 方向 | 优先级 | 类别 | 核心论点 | 已有覆盖 | 估算 |
|---|------|--------|------|---------|---------|------|
| 1 | 跨示例回归检测缺口 | 🟠 P2 | 可维护性 | 示例被用来 dogfood 但不在 CI 中验证；forge-core 变化可能静默破坏已验证的示例 | **零** | ~1 sprint |
| 2 | 子进程资源核算与可观测性 | 🟠 P2 | 可观测性 | 4 维安全护栏（递归/数量/时间/输出）和成本追踪已完备，但 CPU/内存/IO 维度完全盲区 | **接近零** | ~1.5 sprints |
| 3 | Harness 桥接契约测试 | 🟡 P3 | 可靠性 | Go 桥接层对 Node/Python 工具的输出格式有隐式契约，但无 conformance 测试捕获格式漂移 | **零** | ~1 sprint |
| 4 | 配置面安全分析 | 🔴 P1 | 安全 | 4 个配置源（CLI/env/YAML/policy）在安全敏感参数上的静默提权路径和审计盲区 | **接近零** | ~1.5 sprints |

这四个方向共享一个特征：它们不是「新功能」，而是**基础设施债**。每个方向当前都「工作正常」——示例还没坏、子进程还没 OOM、桥接契约还没断、配置还没被恶意利用——但它们都是「还没触发，不是不可能触发」的风险。ForgeOS 已经完成了大量前瞻性功能，现在是时候加固地基了。
