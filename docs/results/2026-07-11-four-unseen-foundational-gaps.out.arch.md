现在我掌握了完整背景。以下是基于该文档的架构分析。

---

# 架构分析：四个基础设施缺口的系统性评估

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 的架构在几个维度上做得非常出色，这些是评估缺口时必须先承认的上下文：

**分层清晰，依赖单向。** `forge-core` (Go) 是编排内核，`harness` (Node/Python) 是带外执法层，`examples` 是 dogfood 验证。这三层的职责边界在 `ARCHITECTURE.md` 和 `AGENTS.md` 中已经白纸黑字固定。这意味着四个缺口中的三个（方向一、二、三）可以**在现有分层内修**，不影响核心架构。

**载重墙（Load-Bearing Wall）认识清醒。** 项目承认「真相之源是带外执法层，host-independent；各工具的 hook 只是加速器适配器」。这个决定直接影响了方向三（桥接契约）的建议方案——契约测试作为带外闸门，而不是在 Go 代码内加运行时校验，与既有架构哲学一致。

**零外部依赖的纪律严格。** `forge-core` 纯 Go 标准库、零 `require`。这意味着方向二（子进程资源核算）里建议的 `syscall.Wait4` / `getrusage` 路径是**标准库能力内的**，不需要引入 `cgroups` 或 `procfs` 解析库，符合项目红线。

**Sprint 演进纪律成熟。** 从 Sprint 5→31 的演进记录可见：每次发现结构债立即拆分、每次发现测试假阴性（yaml2json block-scalar）立即追责修复根本原因、每次架构决策旁路立即纠正（`cmd/forge` 包文件数从 14→18→16 回调）。这意味着四个缺口一旦被接受进入 Sprint，有成熟的机制确保它们不被搁置。

### 1.2 局限性与架构债务

四个缺口暴露了当前架构的三个系统性盲区：

**盲区一：观测层只覆盖「成本+时间」，不覆盖「资源」。** 当前的可观测性架构是针对**经济单位**设计的（`cost_usd_micros`、`duration_ms`），但在**物理资源**维度上存在完全空白。这不是巧合——整个 `trace.Event` 结构体从 Sprint 5 起就定型了，当时的设计焦点是「agent 调用花了多少钱」，不是「子进程占了多少内存」。这是架构层面的**维度缺失**，不是接线疏忽。

**盲区二：跨语言桥接只有「功能测试」，没有「契约测试」。** `forge-core` Go 层和 `harness` Node/Python 层之间的交互完全依赖 `exec.Command` + stdout 解析。当前架构把这种跨语言交互当作「不可见的管道」——只要 exit code 正确、输出能解析就行。但方向三的前车之鉴（yaml2json block-scalar bug 静默漂移半年、安全网测试用 `t.Logf` 而非 `t.Errorf`）证明，这种假设是高风险的。**架构层面缺少一个「跨语言格式契约层」**——既不在 Go 包结构内，也不在 Node 包结构内，而是悬挂在两者之间。

**盲区三：配置面没有「信任分层」模型。** 当前架构把 CLI 标志、环境变量、project YAML、policy YAML 视为**平等来源**，用优先级覆盖（CLI > env > YAML > default）来排序。但这个模型没有区分**意图**和**注入**——用户传 `--mode explorer` 是意图，攻击者设 `FORGE_REPO_ROOT` 是注入。当前架构对这两类「配置输入」一视同仁，缺少一个「受信来源 vs 非受信来源」的标记层。

### 1.3 关键设计决策评估

| 决策 | 合理性 | 债务影响 |
|------|--------|---------|
| Go 执行器 shell 出 Node/Python 工具（不内嵌） | ✅ 正确。零依赖、语言边界清晰、替换成本低 | 方向三提出了对应成本：格式契约测试 |
| trace.Event 用 omitempty 序列化 | ✅ 正确。保证了扩展字段的向后兼容 | 方向二的资源字段直接复用此模式 |
| `--max-agent-calls 0 = unbounded` | ⚠️ 历史债。设计合理（向后兼容），但文档警告不够 | 方向四建议生产模式禁止 0 |
| 4 源配置用 if-else 优先级，无来源标记 | ❌ 架构不足。这是方向四的核心问题 | 影响安全审计和故障诊断 |
| `examples/` 不在 CI 中执行 | ❌ 明确的遗忘。ROADMAP v1 DONE 了 url-shortener，但 CI 从未跟 | 方向一：典型的「已验证一个版本但未持续验证」 |

---

## 2. 扩展方向

基于四个缺口和项目现状，以下是高价值的架构扩展方向（按推荐优先级排序）：

### 方向 A：配置面信任分层模型（P0，来自方向四扩展）

**为什么需要。** 当前有 4 个配置源 + 1 个隐式源（CI 环境变量），但系统无法区分「用户意图」和「环境注入」。对于一个宣称「24h 无人值守自治」的系统，配置注入攻击面是真实威胁——特别是 `lifecycle` / `mode` / `--approved` 这些能绕过安全护栏的参数。安全分析 ROI 极高（代码中已有多个可被利用的模式，修复成本低）。

**核心挑战。**

- **来源溯源机制：** 需要在值本身之外携带元信息（来自 CLI / env / YAML / default）。当前 Go 的类型系统（`string` / `bool`）不携带来源。引入 `TrustedValue[T]` 包装类型会增加代码复杂度。
- **策略 vs 配置的边界：** `modes.yml` 中的阈值（如 `max_depth: 5`）是「策略」还是「配置」？当前都走 YAML 解析 + 相同校验路径。信任分层要求区分——策略来自 git-tracked 的治理资产，配置来自环境/CLI。
- **production 模式的行为收紧：** project.yml 的 `lifecycle: production` 已经能 override 几乎一切（mode-gating / harness 严格度 / reviewer 深度），但 `--approved` 和 `FORGE_AGENT_DEPTH` 仍然是不受 production 保护的旁路。

**预期的架构变更。**

```
// 新增 TrustedValue 包装（internal/config 包）
type Source int
const (
    SourceDefault Source = iota
    SourceYAML
    SourceEnv
    SourceCLI
)
type TrustedValue[T any] struct {
    Value  T
    Source Source
}
```

- `resolveLifecycle` / `resolveMode` 等函数返回 `TrustedValue[string]` 而不是 `string`
- `forge doctor` 新增 `--audit-config` 标志，输出每个参数来源
- `--approved` 改走签名文件而非布尔标志（`.forge/approval.sig` + 时间戳 + 批准者）
- production lifecycle 自动对 `FORGE_*` 环境变量打印警告

**对现有系统的影响。**

- **向后兼容：** `TrustedValue.Value` 可直接取值，所有现有消费者不需要改。
- **侵入范围：** 只影响 `internal/config` 和 `cmd/forge` 的读取点。`internal/orchestrator` 和 `internal/gate` 只消费解析后的值，不接触来源追踪层。
- **估算：** 1 sprint（含安全审计 + production 加固 + approval 签名），与方向四估算一致。

### 方向 B：子进程资源核算框架（P1，来自方向二）

**为什么需要。** 微核（microkernel）式架构的天然代价——子进程对宿主来说是黑盒。当前的成本追踪（`cost_usd_micros`，API 账单）是对 LLM 层的核算，但 CPU/内存/IO 是对**机器资源**的核算。24h evolve 循环中，这两者可能偏离：agent API 账单很低，但子进程内存泄漏已经吃掉 500MB。没有机器资源核算，长时间运行的可靠性只能靠「没有 OOM」来定义。

**核心挑战。**

- **跨平台接口：** `getrusage(2)` 在 Linux 上可用，但 macOS 的字段不同，Windows 完全没有。需要一个抽象层 `ResourceUsage` 搭配 platform-specific 采集。
- **容器环境：** 在 cgroup v2 容器中（Docker/K8s），`/proc/self/stat` 报告的是**容器视角**的资源，不是宿主视角——但子进程的 OOM kill 信号在容器内可被观察。
- **采集开销：** 每次子进程退出都走 `syscall.Wait4` + `rusage` 是零开销的（内核已记账），但 `/proc` 读取是文件 IO。应该**延迟采集**（notable events 触发全量读，否则只读 wait4 的零开销字段）。
- **存储与聚合：** `trace.Event` 当前是 per-event 结构。资源字段如果是 optional（`omitempty`），序列化很干净。但「按模型 × phase × session 的聚合」需要在 scorecard 层新增维度。

**预期的架构变更。**

```
// forge-core/internal/resource/usage.go
type Usage struct {
    CPUMs        int64       // 用户态 + 内核态，来自 getrusage
    PeakRSSKb    int64       // 仅 Linux /proc
    OOMKilled    bool        // 从 signal 判断
    IOReadBytes  int64       // 仅 Linux /proc/self/io（需要额外 syscall）
    IOWriteBytes int64
    NumFDs       int         // 仅 Linux /proc
    Source       string      // "wait4" / "procfs" / "unsupported"
}
```

- `CommandExecutor` 的 `Observe` 回调签名扩展为 `Observe(phase, output string, latency time.Duration, usage *resource.Usage)`
- `trace.Event` 加 `omitempty` 字段
- `forge status --resources` 输出 session 累计
- 新增 package `internal/resource/`（平台文件 `usage_linux.go` / `usage_darwin.go` / `usage_unsupported.go`）

**对现有系统的影响。**

- **向后兼容：** `Observe` 签名的旧实现自动满足新接口（Go 接口的 `*resource.Usage` 参数可选为 nil），所有现有 gateway（cost.go、gateLedger）不需要改一行。
- **侵入范围：** 只影响 `trace.Event` 结构体、`CommandExecutor` 接口、`forge status` 输出。不影响其他引擎。
- **估算：** 1.5 sprints（含跨平台 + 容器边界场景 + scorecard 聚合），与方向二估算一致。

### 方向 C：跨语言桥接契约层（P2，来自方向三）

**为什么需要。** 项目有 6 个 Go ↔ Node/Python 跨语言桥接点（gate.mjs、check.py、acceptance.mjs、yaml2json.py、sca.mjs、mode_gating_check.py），每个都有隐式 stdout 格式契约。历史上 yaml2json block-scalar 静默漂移 6+ 个 workflow 文件的先例证明：这不是「理论风险」。对于「零外部依赖」的 Go 运行时来说，Python shim 是最弱的一环——一个 `pip install` 改变 PyYAML 行为就可能无声破坏 forge 核心。

**核心挑战。**

- **测试定位：** 方向三的核心争议：「契约测试该放在 Go 侧还是 harness 侧还是独立的测试框架？」。放在 Go 侧（`forge-core/internal/gate/gate_test.go`）意味着 Go 包依赖 Python 解释器，破坏 `go test ./...` 的独立性。放在 harness 侧（`harness/bridge_contract_test.mjs`）更符合「带外执法」哲学，但 Node 要 spawn Go 编译产物。
- **格式契约的版本化：** 如果 harness 工具的输出格式演进（比如 check.py 从「人类可读行」改为「JSON 报告」），契约测试应该提供过渡期（两版同时接受），还是原子切换？
- **延迟绑定 vs 立即绑定：** 应该在 CI（`forge accept`）时跑契约测试，还是在 Go 工具编译时跑？后者更紧但更慢。

**预期的架构变更。**

```
// 推荐方案：在 harness/ 下新增 bridge_contracts/ 目录
harness/bridge_contracts/
  gate_contract_test.mjs     — spawn gate.mjs, validate stdout 格式
  check_contract_test.mjs    — spawn check.py, validate [PASS]/[FAIL] 行格式
  yaml2json_contract_test.mjs — 对比 Python shim + Go yaml2json 输出
  acceptance_contract_test.mjs — validate report 格式 + exit code
```

- 用 `node --test harness/bridge_contracts/` 在 CI 中跑，作为 `forge accept` 的前置条件
- 每个测试包含：**格式 schema**（如 `"PASS|FAIL: \d+ files"`）、**exit code 语义**、**负面测试**（注入损坏格式验证拒绝正确）
- 不使用任何外部测试框架（`node:test` 足够），保持零外部依赖

**对现有系统的影响。**

- **向后兼容：** 零影响。新增目录不在现有 gate/check/accept 路径中，只在 CI 中显式接入。
- **侵入范围：** 只增加 `harness/bridge_contracts/` 目录和 CI 配置。
- **估算：** ~1 sprint，与方向三一致。

### 方向 D：跨示例回归检测自动化（P2，来自方向一）

**为什么需要。** 当前有两个 dogfood 示例（url-shortener、go-taskd），每个通过 `forge accept` 验证。但 CI 从不运行它们——这意味着任何 forge-core CLI 行为变化、harness 行为变化、Node 版本变化都可能静默破坏已验证的示例。项目宣称「dogfood 是验证策略的基石」，但没有持续验证就等于没有验证。

**核心挑战。**

- **示例依赖脱离：** go-taskd 是零外部依赖（纯 stdlib），但 url-shortener 依赖 Node 内置 `node:test`。如果未来示例引入外部依赖（如 TypeScript），CI 需要安装对应运行时，否则测试必须诚实 N/A。
- **CI 时间预算：** `examples/` 的构建+测试+gate 增加 CI 时长。应该并行为主、串行为辅。
- **新的示例加入流程：** 应该强制要求在 CONTRIBUTING.md 中声明「新增示例必须一并更新 CI」，或者用守护逻辑自动检测新目录。

**预期的架构变更。**

```
# CI 新增步骤（最小可行，~10 行 YAML）
- name: verify examples regression
  run: |
    for ex in examples/*/; do
      echo "=== Verifying $ex ==="
      cd "$ex"
      forge run build --executor dry
      forge accept
    done
```

扩展方案：`forge verify-examples` 子命令 + `.forge-examples.yml` 清单 + `forge validate --examples` 检查清单与真实目录一致性。

**对现有系统的影响。**

- **向后兼容：** 零。示例目录和 CI 配置独立。
- **侵入范围：** 只改 `.github/workflows/forge.yml` + 新增子命令（可选扩展）。
- **估算：** ~1 sprint，与方向一致。

### 方向 E：安全日志与审计追踪（P1，方向四的衍生）

**为什么需要。** `--approved` 标记当前无日志、无签名、无来源追溯。`human_gate` 的设计意图是「人批准设计后 AI 才能开始 build」，但目前的 `--approved` 实现可以被任何能运行 `forge` 的人绕过。方向四的其他加固手段（mode/lifecycle 校验、production 锁定）都是在修「后门」，但 `--approved` 是「前门没有锁」。

**核心挑战。**

- **无外部依赖的签名机制：** forge-core 零外部依赖，不能用 `libsodium` 或 Go 的 `crypto/ed25519`（那是 stdlib，可用！可以）。`crypto/ed25519` 是标准库，可以零开销集成。
- **审计日志存储：** `.forge/` 目录是自然的选择。但当前 forge-core 的持久化层只存 session trace / checkpoint，没有操作审计日志。需要新增一个 append-only 审计日志文件。
- **CI 下的 --approved 语义：** 在 CI runner 中，没有人按回车——`--approved` 可能由 CI 系统自动设置。需要区分「真人批准」和「CI 流水线批准」。

**预期的架构变更。**

```
type Approval struct {
    Workflow  string    // "design" / "build" / "evolve"
    Approver  string    // human@example.com 或 "ci://github-actions"
    Timestamp time.Time
    Signature []byte    // ed25519 签名(approver 私钥签 WF+TS)
}
```

- `--approved` 改为 `--approve-from`（从文件读取签名批准）或交互式批准（`forge approve design` 写入 `.forge/`）
- `forge audit` 命令输出审计日志链
- CI 场景下，`--approve-from .forge/ci-approval.sig` 由 CI 系统预生成

**对现有系统的影响。**

- **向后兼容：** `--approved` 布尔标志可保留为 deprecated 别名（内部转为匿名批准并打日志）。
- **侵入范围：** `internal/persist` 包（新增审计日志）+ `cmd/forge`（新增 approve 子命令）+ `internal/gate`（human_gate 消费批准日志而非布尔标志）。
- **估算：** 1.5 sprints（含 ed25519 集成 + 审计日志 + CI 集成）。

---

## 3. 接口设计建议

### 3.1 关键模块接口设计原则

**原则一：Observability 接口应该隐式传递，不被显式依赖。**

方向二的 `ResourceUsage` 应该通过 `Observe` 回调传递（而不是通过 `exec.Cmd` 的新字段），因为：

- 现有架构已经在 `Observe` 中传递 `output + latency`，资源是自然扩展
- `*resource.Usage` 作为 optional 值（`nil` 表示平台不支持），所有旧回调自动兼容
- 不破坏 `CommandExecutor` 的现有测试

```
// 推荐扩展方向（不是最终 API）
type ObserveFunc func(phase, output string, latency time.Duration, usage *resource.Usage)
```

**原则二：契约测试应该是「带外的格式守卫」，不是「运行时断言」。**

方向三的桥接契约测试应该位于 `harness/` 下，是 `forge accept` 的前置条件，而不是 `gate.go` 内部的运行时解析校验。理由：

- 运行时校验增加每个 `exec.Command` 的延迟（spawn 一个子进程额外读 stdout）
- 契约测试只需要在 CI 和开发阶段运行，不需要在生产运行时执行
- 带外执法是项目「载重墙」哲学的标准应用

**原则三：配置源追踪应该是「值携带来源」，不是「全局上下文」。**

方向四的信任分层不应该通过全局 `context.Context` 传递来源标记，而应该通过返回 `TrustedValue[T]` 值。理由：

- 显式类型迫使消费者有意识处理来源信息
- 不依赖隐式上下文（goroutine-safe）
- 序列化到日志/输出时自然携带来源

### 3.2 是否需要新的抽象层

**需要：`internal/resource` 包（方向二）**。理由：

- 跨平台接口（Linux / macOS / Windows / unsupported）需要统一的平台文件模式
- `ResourceUsage` 数据类型在 `trace.Event`、`scorecard`、`forge status` 三处消费，需要一个**跨模块共享的结构体类型**
- 当前没有一个包承载「物理资源」概念——它在 `trace`、`orchestrator`、`persist` 三者的间隙中

**不需要：`internal/config` 新包用于 TrustedValue（方向四）**。理由：

- 当前 `internal/gate` 和 `cmd/forge` 都有自己的配置读取函数
- 增加一个共享 `TrustedValue` 类型会导致这两个包都依赖新包，增加耦合
- 更好的方式：在 `cmd/forge` 内部先实现来源追踪，等需要跨包共享时再提取

**不需要：独立的「契约测试框架」（方向三）**。理由：

- 当前 `harness/` 下的测试用 `node:test` 足矣
- 引入第三方测试框架（如 Jest / Mocha）违反零外部依赖纪律
- `node:test` 的 `describe` / `it` 结构足够表达格式契约

### 3.3 向后兼容性策略

| 变更 | 兼容策略 | 退化行为 |
|------|---------|---------|
| `Observe` 加 `*resource.Usage` 参数 | Go 接口加新参数会 break 所有实现 → 改为 **optional interface 断言**：`if ru, ok := observe.(ResourceObserver); ok { ru.ObserveResource(...) }` | 旧实现不实现新接口，资源字段永久为 0 |
| `trace.Event` 加资源字段 | `omitempty` 序列化，旧 JSON 消费者不受影响 | 新字段不出现在旧日志中 |
| `--approved` 改为 `--approve-from` | 保留 `--approved` 作为 deprecated 别名，内部转为匿名批准 + warning 日志 | `--approved` 仍工作但日志记录「无签名批准」 |
| 新增 `harness/bridge_contracts/` | 不在任何现有路径中，只通过 CI 配置接入 | 不打破任何现有测试 |
| CI 新增示例回归步骤 | 新步骤不影响旧步骤，旧行为完全保留 | CI 时间增加（但示例少） |

---

## 4. 技术选型

### 4.1 是否需要引入新技术栈或框架

**明确不需要的：**

- **cgroups 库**（方向二）：项目零外部依赖的纪律不应被破坏。`/proc` 和 `syscall` 是标准库和 OS 接口，足以完成跨平台资源采集。cgroups v2 的路径可以通过环境变量推导，不需要 cgroup 库。
- **测试框架**（方向三）：`node:test` 是 Node 内置，足矣。Jest / Mocha 是外部依赖，不应引入。
- **YAML 库**（方向三方向四）：当前「Go 零依赖 + Python shim」的策略虽然有协议间隙问题，但方向三的契约测试正是为了解决这个问题。引入 Go YAML 库（如 `gopkg.in/yaml.v3`）会增加依赖关系，带来 ADR-0002 级别的审查开销。

**可能需要的（有待架构决策）：**

- **`crypto/ed25519`**（方向四衍生——审计日志签名）：这是 Go 标准库零外部依赖的合法使用。不需要引入外部密码库。但需要 ADR 记录「为什么 forge-core 使用签名机制」，因为这是 forge-core 第一次引入非编排的能力。

### 4.2 自建 vs 采购的决策依据

| 问题域 | 方案 | 选用原因 |
|--------|------|---------|
| 资源采集 | 自建 `internal/resource` | 跨平台 + 零外部依赖 + 轻量级（~300 行），没有现成的零依赖 Go 库 |
| 契约测试 | 自建 `harness/bridge_contracts/` | 用 `node:test` 原生支持，不需框架 |
| 审计日志签名 | 自建 `crypto/ed25519` + `.forge/` 日志 | 标准库能力范围，零外部依赖 |
| 配置安全审计 | 自建 `forge doctor --audit-config` | 利用已有 `forge doctor` 框架（`internal/doctor` 已存在） |

**所有四个方向的自建理由一致：** 项目零外部依赖的纪律 + 功能规模极小（每个方向 200-500 行）+ 不涉及复杂算法/数据存储。没有采购或引入外部框架的必要。

### 4.3 技术债务雷达

以下问题虽然不是本四个缺口的直接范畴，但架构成熟度提升后会暴露：

- **YAML 解析的双轨制：** Go side 行扫描（`projectYAMLValue`）+ Python shim（yaml2json.py）。当前行扫描处理简单 key-value，shim 处理复杂嵌套。但 `resolveLifecycle` 和 `resolveMode` 用行扫描读 mode/lifecycle，而 `ExecuteEffective` 用 Python shim 读 phases——两种解析路径对齐性从未审计。方向四加固时应该顺便做一次对齐性审计。

- **测试资产 vs 真实资产的边界：** `harness/bridge_contracts/` 如果用测试 fixture 验证格式契约，但 fixture 可能和真实 harness 行为漂移。方向二的 `ResourceUsage` 测试如果用 mock 进程验证，mock 的 rusage 值和真实 OS 行为可能不一致。

---

## 5. 实施路线图

### 5.1 优先级排序

| 优先级 | 方向 | 理由 |
|--------|------|------|
| **P0** | 方向四：配置面安全分析（扩展方向 A + E） | 安全相关，已有可被利用的模式（无签名 --approved、env 静默覆写、mode/lifecycle 无校验）。ROI 最高：最少的代码变更换取最大的防御效果 |
| **P1** | 方向二：子进程资源核算（扩展方向 B） | 长运行可靠性的核心盲区。虽然不是今天触发的问题，但**当问题发生时唯一的信号是 OOM 杀进程**——这是最差的故障模式 |
| **P2** | 方向一：跨示例回归检测（扩展方向 D） | 成本最低（CI 10 行 YAML）、防止的 regression 类型目前完全盲区。但触发概率低，P2 合理 |
| **P3** | 方向三：桥接契约测试（扩展方向 C） | 触发概率低，但触发时 100% 静默。yaml2json 先例已经发生了，但已修复。可以推迟到有新的跨语言桥接点加入时再激活 |

### 5.2 阶段划分和里程碑

**阶段一：安全加固（P0，~2 sprints）**

```
M0（week 1）: 配置来源审计
  - 全量扫描所有 env / CLI / YAML 读取点
  - 标注安全关键参数（lifecycle/mode/approved/agent-depth）
  - 输出 forge doctor --audit-config（标记来源）

M1（week 2-3）: production 安全加固
  - --approved 改为签名批准 + 审计日志（crypto/ed25519）
  - project.yml mode/lifecycle 值校验（只允许 modes.yml 声明值）
  - FORGE_AGENT_DEPTH 改为进程内计数器为主
  - production lifecycle 强制 --max-agent-calls 非 0
  - env 来源值打印警告日志
```

**阶段二：资源可观测性（P1，~1.5 sprints）**

```
M2（week 4-5）: 基础资源采集
  - internal/resource 包（platform-specific + fallback）
  - CommandExecutor 集成 rusage 采集
  - trace.Event 扩展（+cpu_ms/+peak_memory_kb/+oom_killed）
  - 单测覆盖 Linux + macOS + unsupported 三平台

M3（week 6）: 聚合与展示
  - forge status --resources 输出 session 累计
  - scorecard 集成资源维度（per-model × per-phase）
  - 资源预警（单 agent 峰值 > 阈值打 warning 日志）
```

**阶段三：回归保护（P2，~1 sprint）**

```
M4（week 7）: 示例 CI 集成
  - CI 新增 examples 回归步骤
  - forge verify-examples 子命令（可选扩展）
  - .forge-examples.yml 清单 + forge validate --examples 一致性校验
```

**阶段四：契约测试（P3，~1 sprint）**

```
M5（week 8）: 桥接契约测试
  - harness/bridge_contracts/ 目录创建
  - gate.mjs / check.py / yaml2json.py / acceptance.mjs 各一个格式契约测试
  - CI 集成（forge accept 前置条件）
```

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 方向四加固触发「false sense of security」（修了 --approved 签名但 CI 自动签名绕过意图） | 中 | 高 | `--approve-from` 中记录来源（`ci://` vs `human://`），`forge audit` 可区分 |
| 方向二资源采集在 macOS 上字段不匹配 Linux | 高 | 低 | platform-specific 文件 + `unsupported.go` fallback。资源字段 optional + `omitempty`，跨平台不会报错只缺数据 |
| 方向三契约测试与真实 harness 行为漂移（测试过了但实际坏了） | 低 | 中 | 契约测试 fixture 必须从真实 harness 提取样本（而不是人写正则）。每个 CI run 自动用当前版本 harness 刷新 fixture |
| 方向一 CI 新增步骤击穿时间预算 | 低 | 低 | 示例少（2 个），构建+测试耗时 < 30s。如果有新示例增加 CI 步骤，可以在 .forge-examples.yml 中用 `ci: skip` 标记 |
| 四个方向同时推进导致架构碎片化（每个方向生成新包、新目录、新约定） | 中 | 中 | **集中控制：** 所有四个方向的包/目录/约定先写一个 ADR（`ADR-0005-infrastructure-debt-repayment.md`），统一约定新增包/目录的命名和位置规则，然后才分方向实现 |

---

## 总结

这份四个缺口的文档是项目目前已产出的 ~295 篇分析中，**最好地覆盖了基础设施债类问题**的分析之一。它不是在请求新功能，而是在请求「加固地基」——这与项目当前阶段（Sprint 31 完成、真点火坐实、功能需求审计完备）完美对齐。

从架构视角，这四个缺口的共同根因是：**ForgeOS 的架构在功能维度的覆盖已经接近完备（编排、路由、安全护栏、治理执法、学习闭环均已落地），但在基础设施维度（可观测性、配置安全、回归保护、格式契约）存在系统性盲区。这些盲区不是因为遗漏，而是因为架构进化路径是「先功能后地基」**——功能侧的 ROI 更可见、更容易驱动 Sprint 决策。

现在功能侧已成熟，正是转向基础设施债的最佳时机。方向四（配置安全）应最先启动，因为它风险最高且修复成本最低；方向二（资源核算）次之，因为它对「24h 无人值守」的价值主张至关重要；方向一和三可按节奏跟上。
