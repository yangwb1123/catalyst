# ForgeOS — 自我测试与 Dogfooding 质量

> **第七次扫描**，聚焦 **ForgeOS 如何测试自己**
> —— 自测覆盖的广度与深度、测试基础设施的完备性、dogfooding 的真实度。
>
> 不写代码，只做审计与判断。

---

## 目录

1. [三套测试套件的全景](#1-三套测试套件的全景)
2. [ForgeOS 在自己身上的 `forge accept` 结果分析](#2-forgeos-在自己身上的-forge-accept-结果分析)
3. [Go 测试的深度挑战](#3-go-测试的深度挑战)
4. [测试选择系统的自身测试问题](#4-测试选择系统的自身测试问题)
5. [Dogfooding 的真实度：自我使用 vs 示例演示](#5-dogfooding-的真实度自我使用-vs-示例演示)

---

## 1. 三套测试套件的全景

ForgeOS 运行三套独立的测试：

### 套件 A：Harness 自我测试（Node + Python）

```
harness/test_acceptance.mjs          # 425行 — acceptance gate 的集成+单元测试
harness/test_adapters.mjs            # 498行 — 语言适配器系统测试
harness/test_enforce.mjs             # 196行 — gate.mjs 执法测试
harness/test_gate.mjs                # 321行 — gate.mjs 单元测试
harness/test_sca.mjs                 # 290行 — SCA 扫描测试
harness/test_scorecard.mjs           # 379行 — scorecard 核心逻辑测试
harness/test_scorecard-telemetry.mjs # 217行 — telemetry 聚合测试
harness/test_scorecard-update.mjs    # 429行 — scorecard update 端到端测试
harness/test_secret-scan.mjs         # 243行 — secret 扫描测试
harness/test_select-tests.mjs        # 111行 — 测试选择测试
harness/test_check.py                # 479行 — 治理完整性测试
harness/test_yaml2json.py            # 80行  — YAML→JSON 编解码测试
harness/scaffold/test_forge-init.mjs # 新项目脚手架测试
harness/scaffold/test_forge-upgrade.mjs # 升级工具测试
harness/arch/test_arch-check.mjs     # 架构检查测试
--------------------------------------
总计: ~3668 行测试代码
```

**质量**：这 14 个自测试文件覆盖了 ForgeOS 的每个主要 Node/Python 基础设施组件。
从测试名称看，它们测试的是**工具本身**，不是测试工具治理的项目。

### 套件 B：Go 运行时测试（forge-core）

```
16 个包 × *_test.go = 46 个测试文件
总计: 11,164 行测试代码 / 20,069 总代码行（55.6%）

全部通过 go test -race（本次扫描验证：零数据竞争）
全部通过 go test（本次扫描验证：16/16 全部 ok）
```

### 套件 C：示例项目测试（url-shortener）

```
examples/url-shortener/test/*.test.mjs  — 短链服务的领域/基础设施/应用/E2E 测试
```

### 1.1 测试在 CI 中的执行路径

当前 CI（`.github/workflows/forge.yml`）运行：

```yaml
- run: node harness/acceptance.mjs       # 套件 A 的部分测试（通过 probeTests 间接运行）
- run: go -C forge-core test ./...       # 套件 B（全部）
```

**缺失在 CI 中的**：
- `node --test harness/` — **从不直接运行** harness 自我测试的完整集
- `go -C forge-core test -race ./...` — 从不带 race 检测
- `node examples/url-shortener/test/*.test.mjs` — 从不验证示例项目

### 1.2 最令人担忧的测试缺口

`forge accept` 的 `probeTests()` 函数会运行 `node --test "harness/test_*.mjs"` 作为其
`test_pass` 标准的一部分。但这是框架的自我测试——它测试的是**测试基础设施**，不是项目代码。

对于 ForgeOS 自身，`test_pass` 可能通过，而 forge-core 的 Go 测试在 CI 的第二个步骤运行。
但**如果 Node 测试因为环境问题（如 Python 3.12 语法变更导致 test_check.py 失败）而失败**，
`forge accept` 中的 `probeTests` 会正确地报告失败。

---

## 2. ForgeOS 在自己身上的 `forge accept` 结果分析

### 2.1 实时运行结果

本次扫描实测：

```
forge-accept: ACCEPTED
  6 pass · 0 fail · 5 n/a (n/a is NOT counted as satisfied)

Pass:
  ✅ test            — Node test suites + Go tests all green
  ✅ lint            — languages detected & linters passed
  ✅ complexity      — under file/function caps
  ✅ arch_violations — no layering/package/fanin violations
  ✅ secret_scan     — no hardcoded secrets found
  ✅ governance      — all agent/skill/workflow references intact

N/A（不计算为满足,但也不阻塞）:
  ⚠️ coverage       — Go: go installed but no coverage tool; Python/Typescript: no tool
  ⚠️ typecheck       — no TS sources / type-checker
  ⚠️ build           — no build step (declarative + zero-dep harness)
  ⚠️ app_test        — (not shown in this run)
  ⚠️ architecture    — (not shown in this run)
```

### 2.2 N/A 分析：5 个未检查的维度

**覆盖率为 N/A** 不是因为"没有代码"——forge-core 有 20k 行代码。而是因为：
- Go `-coverprofile` 没有集成到 harness 中（adapter 声明了 `coverage: go test -coverprofile=coverage.out ./...`，
  但 `acceptance-quality.mjs` 的 `probeCoverageLang` 在运行 `forge-core` 时可能路径不对）
- Python `pytest --cov` 需要 pytest-cov 包
- TypeScript `vitest run --coverage` 需要 vitest 配置

**这意味着 ForgeOS 不知道自己代码的测试覆盖率。** 在 `mode: engineering` 下，
`modes.yml` 要求 `coverage_threshold: 80`——但 coverage gate 是 N/A，阈值从未被评估。

**类型检查为 N/A**：因为 ForgeOS 不使用 TypeScript。这是正确的——但 `project.yml` 声明
`language: polyglot`，而 typecheck 是 Go/Python/JS 混用项目不需要的标准之一。

**构建为 N/A**：因为 ForgeOS 没有传统的构建步骤（Go 二进制是通过 `go build` 构建的，
但 harness 适配器的 `build` 命令需要 `tsc --noEmit && vite build`，Go 的 `build` 命令在
adapter 中声明 (`go build ./...`) 但没有被用于 forge-core 自身。

### 2.3 这意味着什么？

ForgeOS 通过 `forge accept` **在自己身上得到 ACCEPTED** —— 但 5 个 N/A 中有几个是有真实代码
但工具链不完整的检查。如果 someone 重构了 forge-core 并破坏了 `go build ./...`（例如引入了
外部依赖），当前的 CI **不会发现它**，因为 build gate 是 N/A。

这是**系统自身与它治理其他项目的标准之间的差距**：如果这是一个被 ForgeOS 治理的外部项目，
`forge accept` 会要求 coverage > 80%（在 engineering 模式下）、build 必须通过——但对
ForgeOS 自身，同样的标准被 N/A 软化了。

---

## 3. Go 测试的深度挑战

### 3.1 三种测试模式的覆盖率

```
forge-core/test coverage by directory:
  cmd/forge/           ~5000 LOC   测试: ~3000 LOC    ❌ 全是子进程测试
  internal/asset/      ~230 LOC    测试: ~420 LOC     ✅ 纯单元
  internal/converge/   ~140 LOC    测试: ~370 LOC     ✅ 纯单元
  internal/gate/       ~80 LOC     测试: ~70 LOC      ⚠️ 子进程测试
  internal/memory/     ~130 LOC    测试: ~170 LOC     ✅ 纯单元 + 小 IO
  internal/migrate/    ~60 LOC     测试: ~90 LOC      ✅ 纯单元
  internal/mode/       ~50 LOC     测试: ~350 LOC     ✅ 策略矩阵测试
  internal/orchestrator/ ~750 LOC  测试: ~2100 LOC    ⚠️ 部分子进程
  internal/persist/    ~120 LOC    测试: ~170 LOC     ✅ 原子写入测试
  internal/prompt/     ~100 LOC    测试: ~200 LOC     ✅ 纯单元
  internal/risk/       ~80 LOC     测试: ~130 LOC     ✅ 纯单元
  internal/routing/    ~100 LOC    测试: ~230 LOC     ✅ 纯单元 + 记分卡
  internal/trace/      ~100 LOC    测试: ~180 LOC     ✅ 纯单元 + 文件 IO
```

### 3.2 子进程测试模式的成本

`cmd/forge/` 的所有测试（~/3000 行）都使用**子进程模式**：

```go
// main_test.go
func TestCmdRun_BasicExample(t *testing.T) {
    // 编译 forge 二进制
    // 创建临时目录
    // 复制测试 fixtures
    // 运行 forge run build --executor dry --root /tmp/xxx
    // 检查退出码和 stderr
}
```

这在 CI 中 `.github/workflows/forge.yml` 不会运行 `go test forge-core/cmd/forge` —— 它只运行
`go test forge-core/...`，所以 cmd/forge 的测试会被运行。但它们每个会编译一个临时二进制文件，
创建临时目录，运行子进程。这会：

- **使测试变慢**：每个测试函数 ~0.3-1 秒
- **使测试不稳定**：如果 /tmp 磁盘满或并发冲突，可能间歇性失败
- **使调试困难**：失败在子进程中，堆栈不跨越进程边界

### 3.3 覆盖率盲点

虽然 `internal/orchestrator` 有 2100 行测试（2.8:1 的测试:代码比），但测试中使用的
Engine 总是使用 mock `RunGate` 和 `DryRunExecutor`。这意味着：

- **真实的 gate 执行路径从未被测试**：`Engine.RunFrom` → `runGates` → `gate.Run(...)` 这个调用链
  只在 `internal/gate/` 的独立测试中被覆盖，但**没有从 Engine 到真实 gate 的集成测试**
- **真实的 agent 执行路径从未被测试**：`CommandExecutor` 的 unix 测试只验证 `echo` 和超时，
  不验证从 `Engine` 通过 `Exec.Execute` 到 `CommandExecutor.Execute` 的完整路径

### 3.4 建议

```
# CI 增加的两个低成本改进

1. go test -race ./...         # 已经当前 ok（零数据竞争），但要在 CI 中守住
2. go test -coverprofile=coverage.out ./...  # 生成覆盖率报告

# 高成本但高价值的改进
3. cmd/forge 重构：从 int 错误码 + stderr 模式改为 error 返回模式
   使 cmd 函数可直接被 Go 单元测试调用，无需子进程
```

---

## 4. 测试选择系统的自身测试问题

### 4.1 select-tests.mjs 概述

`select-tests.mjs` 是一个**快速 advisory 信号**——它根据 git diff 的文件变更来选择要运行的
测试子集，目的是在长时间 evolve 循环中避免全量测试运行。设计明确声明：

> "This is NOT the gate. It NEVER replaces forge accept, which always runs
> EVERY suite."

### 4.2 自我测试的元问题

`select-tests.mjs` 通过 `test_select-tests.mjs`（111 行）进行测试。该测试验证：

- `mapFile()` 为给定路径返回正确的测试套件
- 为未知文件返回 `UNMAPPED`
- 正确的过滤逻辑

但 **select-tests.mjs 的增量信号从未被验证**——没有人测试过在真实 git diff 上运行
`select-tests.mjs` 是否会返回与全量运行相同的结果。这是一个关键的安全属性：
如果选择器遗漏了必须运行的测试，它只会被全量 gate 捕捉到，而全量 gate 只在迭代结束时运行。

### 4.3 select-tests.mjs 自身覆盖范围

| 文件类型 | 选择的测试 | 潜在漏洞 |
|---------|-----------|---------|
| `*.go` 变更 | `go test` 对应包 | ✅ 合理 |
| `*.mjs` 变更 | 对应 `test_*.mjs` | ✅ 如果有匹配 |
| `*.py` 变更 | 对应 `test_*.py` | ✅ 如果有匹配 |
| `.agent/*` 变更 | `check.py` + `test_enforce.mjs` | ✅ 合理 |
| `.md` 变更 | 无（只影响文档） | ✅ 合理 |
| **新文件** | 无映射 → UNMAPPED → 推荐全量 | ✅ fail-safe |
| **新的测试文件**（`test_*.mjs` 但还未存在） | 如果被 git diff 捕捉到 → mapFile 找到它 | ⚠️ 取决于 git diff 模式 |

**风险**：如果一个 PR **修改了一个 `test_*.mjs` 文件和一个 `.go` 文件**，
select-tests 会尝试运行两者。但如果修改只影响了一个被间接测试的文件（如 `gate.go` 的变更
但只有 `test_gate.mjs` 间接测试它），选择器会选择 `go test` 那个包——这是正确的。

---

## 5. Dogfooding 的真实度：自我使用 vs 示例演示

### 5.1 自我使用的证据

ForgeOS 在以下方面**真正使用了自己**：

| 使用方式 | 证据 | 真实度 |
|---------|------|--------|
| CI 运行 `forge accept` | `.github/workflows/forge.yml` | ✅ 真实 |
| CI 运行 `go test forge-core/...` | 同上 | ✅ 真实 |
| `.claude/settings.json` PostToolUse hook | 每次编辑后运行 `gate.mjs` | ✅ 真实 |
| 架构规则强制执行 | arch-check.mjs 运行 `.arch/rules.yaml` | ✅ 真实 |
| 治理完整性检查 | check.py 验证文件存在 | ✅ 真实 |
| Secret 扫描 | secret-scan.mjs 扫描代码 | ✅ 真实 |

### 5.2 自我使用的**不**使用

| 功能 | 是否自我使用 | 原因 |
|------|------------|------|
| `forge run build --executor command` | ❌ | 需要 claude binary（CI 中没有） |
| `forge evolve` | ❌ | 同上 |
| `forge route` | ❌ | 设计为查询工具，但 ForgeOS 自身路由通过 Claude Code 插件系统完成 |
| `forge migrate --apply` | ❌ | 本仓库已经是 `engineering` 模式，不需要迁移 |
| `forge detect` | ❌ | 用于检测其他项目的 mode/lifecycle，本仓库已知 |
| Memory 系统 | ❌ | forge-core 的 memory 功能是为 evolve 循环设计的，本仓库不运行 evolve |
| Scorecard 系统 | ❌ | 没有 claude 执行，没有 trace events → 没有 scorecard 更新 |

### 5.3 示例项目与真实使用之间的差距

`examples/url-shortener` 展示了 ForgeOS 治理下的项目结构——有正确的架构分层、
在 `forge accept` 下通过、有零外部依赖的 Node 代码。但它是一个**手工维护的示例**，
不是由 ForgeOS 生成的。它展示的是**治理产出**（最终项目结构），不是**治理过程**
（forge run → evolve → review 的循环）。

真正引人注目的 dogfooding 缺失是：
- 没有任何功能是通过 `forge run build` + reviewer 的循环添加到 ForgeOS 自身的
- 没有 `forge evolve` 循环为 ForgeOS 自身自动发现和填充 roadmap 项
- 没有 reviewer phase 实际审查过 forge-core 的代码

### 5.4 建议：真正的 ForgeOS-on-ForgeOS

要真正吃自己的狗粮，ForgeOS 至少应该：

```
1. 用 forge run build 为 forge-core 添加至少一个功能
   （例如"增加 --json 输出标志到 forge run"）
   验证：reviewer phase 产生真正有意义的审查意见

2. 为 forge-core 编写一个 acceptance.schema.yml 实例
   验证：特征分支的 DoD 通过 forge accept 被实际验证

3. 在全量 CI 中运行一次 forge run build --executor dry
   验证：编排状态机在实际上线前被测试
```

---

## 总结：自我测试清单评分

| 标准 | 等级 | 证据 |
|------|------|------|
| **Harness 自测** | ✅ 7/10 | 14 个文件, 3668 行；但不是所有 CI 都运行 |
| **Go 自测** | ✅ 8/10 | 46 个文件, 11164 行；-race 通过；但全是子进程 cmd 测试 |
| **示例项目** | ⚠️ 5/10 | url-shortener 存在但手工维护；展示产出不展示过程 |
| **CI 覆盖率** | ⚠️ 5/10 | forge accept + go test 在 CI 中；但 -race 和 coverage 不在 CI 中 |
| **自我 dogfooding** | ❌ 2/10 | 没有通过 forge run 添加的功能；没有真正使用 evolve/route |
| **select-tests 安全** | ⚠️ 6/10 | 设计正确但遗漏风险只在全量 gate 中被捕捉 |
| **N/A 风险** | ⚠️ 4/10 | 5 个 N/A 掩盖了潜在的无声回归（build/coverage） |

**最危险的发现**：ForgeOS 治理其他项目要求 build 必须通过、coverage 必须 ≥ 80%（在 engineering 下），
但**自己却不满足这些标准**——因为工具链不完整导致 gate 降级为 N/A。这不是设计缺陷（N/A 是诚实的），
但它意味着 CI 不会发现 `go build ./...` 断裂。

*分析日期：2026-06-29 | 基于第七次全量扫描（自测基础设施 + Dogfooding 真实度视角）*
