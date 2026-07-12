# 架构分析报告：ForgeOS 五方向扫描的技术评估与扩展建议

> **分析对象**：`docs/requirements/2026-07-12-five-post-scan-architectural-extension-directions.md`
> **分析视角**：系统架构 · 接口设计 · 技术债务 · 实施路径
> **分析日期**：2026-07-12

---

## 一、架构评估

### 1.1 当前架构的优势

ForgeOS 经过 31 个 Sprint 的演进，在以下方面展示了扎实的架构功底：

**分层职责清晰**。从代码级证据可见，orchestrator、trace、mode、asset 四层各司其职，没有出现典型的「包 x 依赖包 y 的 internal 实现细节」式的循环依赖。`internal/mode/mode.go` 的单向依赖树（mode → asset → 无下流依赖）符合 Go 标准布局。`arch-check.mjs` 包含的 8 项检查中有 fan-in 检查证明了团队对包依赖纪律的主动维护。

**治理骨架超前**。`.agent/` 的 5 workflow + 12 agent 卡 + 9 skill 卡是一个**元治理系统**——用 AI 开发的系统自我说明也给 AI 开发者阅读。`gate.mjs` + `arch-check.mjs` 的双层门控虽有不完善之处，但意识已到位。对比同类项目，很多在 Sprint 30+ 时只有测试，没有代码红线门控。

**ADR 驱动演进**。已有 4 篇 ADR 记录关键转向（如 migrate、mode×lifecycle 等），这对理解设计决策的「为什么」至关重要。特别值得肯定的是 `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的存在——它把功能需求与代码实现之间的映射关系做了显式审计。

**错误分类有弹性基架**。`exec_error.go` 的 5 种 `Kind*` 常量 + `classifyRunErr` 函数提供了可扩展的分类框架。虽然当前分类存在灰色地带（文档方向一的核心论点），但基架本身是可扩展的——加两个新 kind 不需要重构分类框架本身。这是一个适合未来横向扩展的良好抽象。

### 1.2 核心架构债务

**债务一：配置面（Configuration Plane）缺乏形式化校验**

这是最严重的技术债。当前 `project.yml` 的加载路径是：

```
project.yml → asset.go (宽容加载) → mode.go (零校验) → Effective() (零值容忍)
```

这条路径没有任何一个环节执行**输入格式验证**。`asset.go` 的注释明确说「deliberately fault tolerant」，但 fault tolerance 不等同于在所有上下文中都容忍——对于 `forge validate` 命令，它应该 fail-fast；对于 `forge run`，它可以在 warn 后继续。**当前架构把「容忍」内置到了解析层，剥夺了上层（CLI 命令）选择策略的权利。**

这种设计意味着：配置语义错误永远无法在加载时被检测到，只能靠后续执行路径上的副作用间接暴露。这是典型的**静默退化（silent degradation）**入口，与文档中方向一的「半死不活」错误分类问题同源。

**债务二：可观测性数据面（Observability Data Plane）缺乏演化契约**

`trace.jsonl` 的 `_format: "forgeos.trace.v1"` 字段形同虚设——生产者总是写 `v1`，消费者从不检查。这实质上等价于 **无版本约束的 schema-on-read 架构**。当前这种设计在小规模（单项目、短运行）下工作良好，但一旦面临以下三个条件中的任意两个，就会断裂：
- 多个 ForgeOS 版本在同一项目上交替运行
- trace 归档时间跨度超过一个版本周期
- 下游消费工具（scorecard、回溯、仿真）与 trace 生产者解耦发布

文档的方向三提出 schema register，是对此债务的直接回应。但需注意：这不是 trace 包的局部问题，而是**整个系统的可观测性架构缺乏版本契约**——checkpoint、scorecard、trace 三个持久化数据面应该共享同一个版本声明的协议。

**债务三：治理执法（Governance Enforcement）将「检测」等同于「预防」**

当前红线执法的架构模式可归纳为：

```
agent edit → gate.mjs (post-hoc) → 发现违规 → agent 手动修复
```

这个循环的**反馈延迟太大**。对于人类开发者，「写了 500 行然后被 CI 拒绝」是一个合理的反馈循环（commit → CI → fix）。但对于 AI agent，一次编辑可能消耗 $0.50 的 API 费用，而 600 行的文件可能需要两次调用才能重构——等于一次不必要的 agent loop 浪费 $1.00+。

更关键的是架构层面的问题：**门控逻辑（gate.mjs）与执行引擎（forge run/evolve）没有集成**。当 `arch-check` 发现包文件数超限时，它报告的只是一个「检测结果」。没有任何机制能阻止 agent 继续在超限的包中添加文件——因为运行时的 `forge run` 不检查 `forge accept` 的结果。

这与文档中方向四的核心论点一致：治理执法需要从**事后检测 → 事前预警 → 运行阻断**三级进化。

**债务四：子进程安全模型基于信任而非边界**

`os.Environ()` 全量继承 + argv 无白名单 + 文件写无路径约束 = **子进程拥有与父进程几乎等价的系统访问权限**。对于个人项目，信任 claude CLI 是合理的假设。但「信任」与「安全」在架构层面有本质区别：

| 维度 | 信任模型 | 安全模型 |
|------|---------|---------|
| 假设 | 子进程不可恶意 | 子进程可能被攻破 |
| 控制 | 谁可以运行 | 运行后能做什么 |
| 故障模式 | 子进程如父进程一样脆弱 | 子进程被隔离在边界内 |

当前 ForgeOS 完全落在信任模型一侧。文档的方向五建议逐步引入最小权限，但架构层的变更需要更系统——不只是在 `command_executor.go` 加环境变量过滤，而是需要定义 `Sandbox` 接口。

### 1.3 关键设计决策评估

| 决策 | 原始动机 | 当前评估 | 建议 |
|------|---------|---------|------|
| 零值容忍（fail-open） | 不因配置小错误崩溃 | ❌ 静默退化缺口 | 改为分层策略：validate fail-fast, run warn-on |
| trace 写死 v1 | 简单可预测 | ❌ 格式进化能力为零 | 引入 schema registry + 版本协商 |
| gate.mjs 作为唯一 hook | 轻量快速 | ⚠️ 漏掉 arch-check | gate-fast.mjs 作为增量检查 |
| `os.Environ()` 全继承 | 子进程无环境问题 | ❌ 凭证泄露风险 | 白名单环境变量 + 可选 preserve-env |
| agent card `emits:` 不用于写权限 | 简化模型 | ❌ 浪费了已有声明 | 从 emits 推导写权限范围 |

---

## 二、高价值架构扩展方向

### 方向 A（新）：**配置生命周期的形式化验证管线 —— Configuration Pipeline**

**为什么需要**。当前 `project.yml` 的验证路径是一维的（宽容解析 → 零校验 → 运行时靠零值）。但配置面的完整性需求远不止于 mode/lifecycle 字段校验——还需要跨字段约束（如 `lifecycle:production` 要求 `mode:engineering`）、引用完整性（如 `project.agents` 引用的 agent ID 必须在 `.agent/agents/` 中存在）、以及版本兼容性约束（如当前 forge-core 版本是否支持 `project.yml` 中声明的 `format_version`）。

**核心挑战**。ForgeOS 的配置面涉及三个层：
1. **项目层**（`project.yml`）：mode、lifecycle、name、agents
2. **工作流层**（`.agent/workflows/`）：phase、gate、agent assignment
3. **Agent 层**（`.agent/agents/`）：role、capabilities、emits

当前这三个层的校验是**分离且不一致的**——project.yml 靠零值容忍，workflow 靠 `asset.go` 的宽容加载，agent card 几乎无校验。需要一个统一的验证框架跨这三层做**端到端配置完整性检查**。

**预期架构变更**：
- 新增 `internal/config/validation/` 包（或 `internal/config/` 作为配置面的根包）
- 定义 `Validator` 接口：`Validate(config interface{}) []ValidationIssue`，返回可遍历的错误/警告列表
- 三个层的配置分别实现该接口（或通过反射 + schema 自动推导）
- `forge validate` 调用 `Validator` 链，输出结构化 JSON 报告
- `forge run` 在 preflight 中执行 `Validator`，严重错误阻止运行，警告写入 trace

**对现有系统的影响**：低。`asset.go`、`mode.go` 的加载逻辑不变。只在加载后**新增**验证步骤。验证器可以渐进式引入——先加 mode/lifecycle allowlist（P0），再加跨字段约束（P1），再加 agent card 引用完整性（P2）。

---

### 方向 B（新）：**运行时策略引擎 —— Policy Decision Point（PDP）抽象**

**为什么需要**。当前 mode×lifecycle 的策略组合逻辑硬编码在 `mode.go` 的 `Effective()` 函数中——一个 switch-case 产出 `Policy` 结构体。gate 的开关逻辑硬编码在 `internal/gate/` 中。这种硬编码方式在策略数量少时直接高效，但有以下问题：

- **策略变更需要重新编译 forge-core**（Go 二进制）。对于组织部署场景，安全团队可能需要在不重新编译运行时的情况下调整策略。
- **策略无审计追踪**。当前没有「为什么这个 gate 被触发/跳过」的决策日志——决策结果体现在行为中（gate 运行了或跳过了），但推理过程不持久化。
- **策略不可组合**。`Effective()` 返回的是一个扁平的 `Policy` 结构体，策略之间的优先级和覆盖关系隐式编码在函数体中，无法在运行时动态调整。

**核心挑战**。策略引擎引入会颠覆当前简洁的 if-else 式决策。需要权衡：
- **声明式 vs 代码式**：OPA/Rego 之类的声明式策略语言比 Go 代码更可审计，但增加了技术栈复杂度和运行时开销（每次决策都要评估策略）。
- **静态 vs 动态**：静态策略（编译时确定）性能好但灵活差；动态策略（运行时加载）灵活但需要策略分发和缓存机制。

**选项权衡**：

| 选项 | 优点 | 缺点 |
|------|------|------|
| **A：保持硬编码 + 输出决策日志** | 零额外依赖，决策可审计 | 策略变更仍需重编 |
| **B：引入轻量级策略 DSL**（自定义 YAML 策略文件） | 声明式，可版本管理 | 需要设计和维护 DSL 解析器 |
| **C：嵌入 OPA/Rego** | 业界标准，工具链成熟 | 引入 8MB+ 二进制依赖，过度工程化 |

**推荐**：**选项 A 目前足够，但需要「决策日志化」**。当前 `Effective()` 不输出决策路径。最小侵入的架构改进是让 `Effective()` 返回一个增强的 `Policy` 结构体——不仅包含最终策略值，还包含决策链（每个覆盖步骤的来源和理由）。这个决策链写入 trace，让每个 gate 的触发/跳过都有可追溯的原因。等到策略数量再翻倍或有组织部署需求时再考虑选项 B。

**对现有系统的影响**：低（选项 A）到中（选项 B）。决策日志化只改 `mode.go` 的返回值和 trace event 的字段。

---

### 方向 C（新）：**持久化数据面的统一版本契约 —— Data Schema Compact**

**为什么需要**。当前 ForgeOS 有三个持久化数据面，各自有不同的版本策略：

| 数据面 | 当前版本策略 | 问题 |
|-------|------------|------|
| `trace.jsonl` | `_format: "forgeos.trace.v1"`（写死） | 消费者不检查，格式无法演进 |
| `checkpoint.json` | 无版本字段 | 二进制格式变更后旧 checkpoint 无法加载 |
| `scorecard` | 从 trace 重建 | 隐式版本依赖 trace 格式 |

三者互相依赖——checkpoint 引用 trace 段索引，scorecard 从 trace 重建。一个数据面格式变更会影响所有下流消费者。当前没有版本兼容性契约。

**核心挑战**。版本契约的引入需要在**灵活性**和**简洁性**之间找到平衡。过度设计（如 protobuf schema registry）会破坏 Go struct 解析的简洁性；完全没有版本约束则使格式演进需要全量消费者升级。

**建议**：
1. 给所有持久化数据面加 `_version` 字段，格式为 `YYYYMMDD-<increment>`（如 `20260712-01`），代表该数据结构的兼容性版本号
2. 定义兼容性规则：minor 版本变更（字段新增/可选字段添加）向前兼容；major 版本变更（字段删除/必须字段语义变更）需要显式迁移
3. 在 `internal/trace`、checkpoint、scorecard 三处各实现一个简单的版本校验函数：`CheckCompatibility(currentVersion, fileVersion string) error`
4. 迁移路径：`forge migrate trace --from-version 20260101-00 --to-version 20260712-01`

**对现有系统的影响**：中低。加 `_version` 字段是向前兼容的（旧文件没有该字段时默认为最早版本）。版本校验函数按需调用——对性能敏感的 hot path（trace emit）可以跳过校验，只在重建/消费时校验。

---

### 方向 D（新）：**Agent 运行时沙箱架构 —— Sandboxed Runtime Surface**

**为什么需要**。文档方向五的「子进程最小权限」聚焦于环境变量和 argv 白名单。但安全边界的架构深度不止于此——ForgeOS 的子进程执行模型有几个深层的安全问题：

- **并发安全**：`CommandExecutor` 目前是串行的（一次一个 agent 调用）。但如果未来引入并行 agent 执行（如 fleet 场景下的多 agent），子进程之间的隔离完全缺失——两个 agent 可能同时写同一个文件。
- **持久副作用**：agent 调用的副作用（文件写、进程创建）在 orchestration loop 结束后留下持久状态。当前没有机制在 `forge run` 结束后清理沙箱残留。
- **网络访问**：claude CLI 需要网络访问（API 调用）。但如果 agent 在 `readonly` phase 中被 prompt 注入，它本不应写文件但可以发出网络请求（如果网络允许）。

**核心挑战**。引入完整沙箱（gVisor、Firecracker、或 Docker 容器）的运维成本远高于当前方案。需要在安全增益和复杂性之间找到「黄金切割点」。

**建议的架构分层**：

```
P0: 环境变量最小化 (buildEnv 白名单)      ← 文档方向五·A，当前 Sprint 可做
P1: argv allowlist (命令白名单)           ← 文档方向五·B，下个 Sprint
P2: 文件系统写路径约束 (per-phase emits)  ← 文档方向五·C，需 schema 先行
P3: 可选的容器执行后端 (CommandExecutor 接口化)
```

关键的架构决策是将 `CommandExecutor` 抽象为**接口**，而不是修改现有实现：

```go
// 当前结构体直接实现
type CommandExecutor struct { ... }

// 建议的接口抽象
type Runner interface {
    Run(ctx context.Context, argv []string, opts RunOptions) (*Result, error)
}

// 保留现有实现为 DefaultRunner（直接 os/exec）
// 叠加层为 SandboxRunner（环境变量过滤 + argv 校验）
// 未来可加 ContainerRunner（Docker 执行）
```

这样方向五的三个子方向可以作为装饰器链（Decorator Chain）叠加，而不改变现有的 `engine_build.go` 调用者。

**对现有系统的影响**：中（接口抽取影响少数调用者，但逻辑不变）。P0/P1 是纯新增代码，不改现有行为。P2 需要 agent card 解析的配合。P3 是可选扩展，不阻塞前三个子方向的落地。

---

### 方向 E（新）：**可观测性面的查询与回放接口 —— Observability Query Layer**

**为什么需要**。文档方向三提出了 trace schema registry 和分段归档。但在这之上，缺少一个统一的查询接口，使得 trace、scorecard、checkpoint 三个数据面可以被统一检索。当前，获取「上周所有 agent timeout 事件」的信息流是：

1. 找到 trace 文件 → 2. 手动 `jq` 过滤 kind:agent+error → 3. 从 trace 事件中提取 model/cost 信息 → 4. 交叉引用 checkpoint 中的 mode_snapshot（当前不存在）→ 5. 整合到 scorecard

这是一个完全手动的数据探查流程，且步骤 4 当前不可行。更结构化的可观测性架构应该提供**一个查询入口**，屏蔽底层存储格式的差异。

**核心挑战**。不要过度工程化为「ForgeOS 版的 Prometheus」。查询层应该保持轻量——不是实时监控系统，而是事后分析工具。核心能力应该是：按时间范围、事件 kind、mode/lifecycle 组合过滤 trace 事件，并输出结构化数据供 scorecard 重建和仿真引擎消费。

**建议的最小可行架构**：

```
CLI: forge trace query --kind error --since 24h --format json
                     --filter 'mode:engineering,lifecycle:production'

内部：TraceStore 接口
      ├── FileTraceStore(当前实现，读 .forge/trace-*.jsonl)
      ├── SegmentedTraceStore(方向三·B 的分段归档)
      └── (未来) SQLiteTraceStore(编译时可选，性能优化用)
```

`TraceStore` 接口不应尝试实现完整的 SQL 查询能力——它应该暴露两个核心方法：
- `Query(q TraceQuery) ([]Event, error)` — 按属性过滤
- `Stream(q TraceQuery) <-chan Event` — 流式读取（用于大结果集）

**对现有系统的影响**：低。`TraceStore` 接口可以增量引入——先包装当前的文件读取逻辑不改接口，再逐步注入分段和 schema 校验能力。CLI 子命令是纯新增入口，不侵入现有 `forge scorecard rebuild` 流程。

---

## 三、接口设计建议

### 3.1 关键接口设计原则

**原则一：校验与执行分离**

当前架构将「宽容加载」与「运行时行为」耦合。建议的分离原则：

```
project.yml → Decode() → Struct     ← 永远成功（容错）
            → Validate() → Issues   ← 可选调用（严格）
            → Validate(strict: true) → Error  ← 显式严格模式
```

`Decode()` 永远不因配置格式返回错误（保持当前 behavior）。`Validate()` 是新增的可选调用，返回一个 `[]Issue`（每个 Issue 有 severity: ERROR | WARN | INFO）。调用者（`forge validate` vs `forge run`）决定哪些 severity 阻断执行。

**原则二：可扩展的错误分类，而非匹配式分类**

当前的 `classifyRunErr` 使用 switch-case 返回值。新增 `KindPartialWrite` 和 `KindResourceExhausted`（文档方向一）应该通过注册模式而非修改 switch-case：

```go
// 当前
func classifyRunErr(err error) ExecKind {
    switch {
    case isConfig(err):
        return KindConfig
    // ...
    }
}

// 建议：新增 Classifier 接口
type Classifier interface {
    Classify(err error) (ExecKind, bool)
}
```

这样新的错误分类可以注册到 orchestrator 而不修改核心分类逻辑。但需要评估这种抽象在当前规模下的必要性——如果错误分类不超过 10 种，switch-case 更清晰。建议在达到第 8 种分类时重构为注册模式。

**原则三：数据面版本在写入时声明，在读取时校验**

```
// 写入
ev.Format = "forgeos.trace.v1"        // ← fix: 从全局常量改为运行时变量
ev.FormatVersion = currentFormatVer   // ← 新增: 版本号动态注入

// 读取
func (s *TraceStore) validateFormat(ev *Event) error {
    if ev.FormatVersion == "" {
        ev.FormatVersion = "20260101-00"  // 最旧兼容版本
    }
    return checkCompatibility(currentFormatVer, ev.FormatVersion)
}
```

关键设计决策：版本应该在**写入时**确定（不是编译时写死字符串），在**读取时**校验（不是假设所有文件都是当前版本）。这使得同一项目目录下可以有不同版本的 trace 文件共存。

### 3.2 是否需要新的抽象层

**是，需要一个「配置面」（Configuration Surface）的抽象层**。

当前配置的读取、解析、校验、使用分散在四个包中（`asset`、`mode`、`orchestrator`、`gate`）。没有一个包是「配置消费者」的唯一入口。建议新增 `internal/config/` 包，职责：

- 聚合加载 `project.yml` + `.agent/workflows/*.yml` + `.agent/agents/*.md`
- 提供 `ProjectConfig` 结构体（非零值容忍的严格结构）
- 暴露 `Validate()` 和 `LoadWithValidation()` 方法

**保持现有的 `asset.go` 和 `mode.go` 不动**——它们继续为内部消费者提供低层加载能力。`internal/config/` 是一个**外观层（Facade）**，提供统一的、带校验的配置入口。现有代码无需修改，新代码（`forge validate`、preflight check）使用外观层。

### 3.3 向后兼容性策略

五个扩展方向对向后兼容性的影响：

| 方向 | 兼容性风险 | 缓解策略 |
|------|-----------|---------|
| 一（错误分类加新 kind） | 低。新 kind 是新增常量，旧事件不受影响 | 新增 kind 不被 `classifyRunErr` 的 `default` 分支捕获，需要显式处理 |
| 二（project.yml schema） | 中。`format_version` 字段对于旧文件为 absent | 缺失 `format_version` 时默认为当前版本（宽松）；`min_forge_version` 缺失时默认兼容所有版本 |
| 三（trace schema registry） | 中。消费者需要学会忽略未知 kind | schema registry 提供 `ValidKind` 而非 `AllowedKindOnly`——未知 kind 触发 warn 而非 error |
| 四（gate-fast.mjs 前置检查） | 低。新增闸门不改变现有行为 | gate-fast.mjs 检查失败只阻止 forge run，不破坏 `.forge/` 中的已有数据 |
| 五（子进程最小权限） | 中。环境变量过滤可能破坏依赖环境变量的子进程 | `--preserve-env` flag 完全恢复旧行为；白名单逐步从宽松（保留常见环境变量）缩紧 |

关键的兼容性原则：**每一步架构变更都必须提供一个 fallback 路径，让旧配置/旧 trace/旧脚本在显式 opt-in 下继续工作**。对于 fail-open → fail-closed 的设计哲学迁移（`project.yml` schema），这个 fallback 应该是 `FORGE_ALLOW_UNVALIDATED_CONFIG=1` 环境变量——在紧急恢复场景下使用，平时默认开启校验。

---

## 四、技术选型

### 4.1 是否需要引入新栈

逐个方向评估：

| 方向 | 所需能力 | 推荐方案 | 理由 |
|------|---------|---------|------|
| ① 错误分类扩展 | 分类器注册模式 | **纯 Go interface**，零依赖 | 不需要外部依赖，接口调通即可 |
| ② project.yml schema | YAML schema 校验 | **直接手写 Go 校验**（无需 JSON Schema 库） | 字段不到 20 个，校验规则简单（allowlist + 正则），引入第三方 schema 库（如 `go-jsonschema`）的维护成本超过手动校验的成本 |
| ③ trace schema registry | 版本兼容性校验 | **纯 Go struct 标签**，零依赖 | 版本号 + 必填字段列表可以用 Go 的 `reflect` 实现，不需要 protobuf/avro |
| ④ 红线自动门控 | 增量 preflight 检查 | **纯 Node.js**（延续现有 gate.mjs 技术栈） | CC hook 域是 Node.js，引入其他语言需要启动时额外开销 |
| ⑤ 子进程最小权限 | 环境白名单 + argv 校验 | **纯 Go**，零依赖 | 检查逻辑是简单的字符串匹配/正则，不需要第三方安全库 |

**结论：当前阶段不需要引入任何新的运行时依赖或框架。** 所有五个方向可以用现有的 Go（forge-core）和 Node.js（harness）技术栈实现。这是罕见的「纯零依赖架构扩展」场景——证明原有技术选型在可扩展性上留足了空间。

### 4.2 自建 vs 采购决策

没有采购选项适用——所有扩展方向都是 ForgeOS 自身的功能增强。但有一个值得注意的**外部集成点**：方向五的 argv allowlist 和文件写路径约束，在组织部署场景下可能需要与外部策略管理系统的集成。但这是 P3 级别的需求（文档的优先级评估正确），当前不构成采购 vs 自建的决策点。

### 4.3 评估标准的建议

如果未来需要引入第三方依赖（如 trace 持久化接入外部存储后端、配置面集成 OPA 策略引擎），建议使用以下标准：

| 标准 | 权重 | 说明 |
|------|------|------|
| 零外部传递依赖 | 高 | forge-core 的目标是纯 Go 标准库，传递依赖超过 3 层则拒绝 |
| 注入式设计 | 高 | 第三方库必须通过接口注入，不能直接引入到核心包 |
| 编译时间影响 | 中 | 增加 forge-core 编译时间超过 10% 则需缓存或 feature gate |
| 二进制体积影响 | 低 | 除非体积增加超过 5MB，否则不关注 |
| 许可证兼容性 | 高 | 必须 MIT/BSD/Apache 2.0，GPL/AGPL 系列拒绝 |

---

## 五、实施路线图

### 5.1 优先级排序

基于「成本 → 收益」矩阵和文档的建议，推荐以下优先级：

**P0（当前 Sprint / 下一个 Sprint）**

| # | 项目 | 对应方向 | 预估工作量 | 理由 |
|---|------|---------|-----------|------|
| 1 | `project.yml` 字段校验（mode/lifecycle allowlist） | 二·方向 2 | 0.3 Sprint | 最低成本 + 最高安全收益，堵住「拼写错误绕过生产覆盖」 |
| 2 | `classifyRunErr` 加 `KindResourceExhausted` | 一·方向 B | 0.5 Sprint | 新增一种错误类型 + 退避重试逻辑，解决静默退化 |
| 3 | `gate.mjs` 增加「距阈值不足 50 行」的预警告警 | 四·方向 A | 0.2 Sprint | 纯文本告警，不涉及架构变更 |

这三个 P0 项目覆盖了文档优先级收敛建议的「方向二 + 四 + 一」三角基底，且每个工作量不超过 0.5 Sprint。

**P1（下两个 Sprint）**

| # | 项目 | 对应方向 | 预估工作量 | 理由 |
|---|------|---------|-----------|------|
| 4 | `project.schema.yml` 完整 schema + `forge validate` | 二·方向 1 | 1 Sprint | 需要用 Go 手写校验规则（~30 行）+ CLI 子命令接线 |
| 5 | `CommandExecutor.buildEnv` 环境变量白名单 | 五·方向 A | 0.5 Sprint | 定义白名单列表 + 修改 buildEnv |
| 6 | trace event 加 `mode_snapshot` 可选字段 | 三·方向 3 | 0.5 Sprint | 从 `RunContext` 注入，`omitempty` 向后兼容 |
| 7 | `arch-check` 的包文件数/函数长度检查接入 CC hook | 四·方向 B | 0.7 Sprint | 新增 `gate-fast.mjs`，聚合体积+架构检查 |

**P2（后续 Sprint）**

| # | 项目 | 对应方向 | 预估工作量 | 理由 |
|---|------|---------|-----------|------|
| 8 | trace 分段归档 | 三·方向 2 | 1.5 Sprint | 涉及 checkpoint 接线，需验证不破坏 `scorecard rebuild` |
| 9 | `classifyRunErr` 加 `KindPartialWrite` + 清理合约 | 一·方向 A | 1 Sprint | 需要 git 集成设计（回滚范围、checkpoint 交互） |
| 10 | argv allowlist + `--preserve-env` flag | 五·方向 B | 1 Sprint | 需要 CLI flag + 兼容旧行为的 fallback |
| 11 | `CommandExecutor` 接口化 | 五·P3 | 1.5 Sprint | 涉及现有调用者重构，需要测试覆盖 |

### 5.2 阶段划分

**Phase 1：治理可信基座（Sprint 32-33）**

P0 项目全部完成。预期交付：
- `project.yml` 校验导致 `lifecycle: "produktion"` 在 `forge run` 时被拒绝
- `ENOSPC`/`EMFILE` 错误不再被分类为 `KindFailed`，而是触发退避重试
- agent 在编辑循环中收到「文件距 500 行上限仅剩 XX 行」的警告
- ADR-005 记录 fail-open → fail-closed 的转向

**Phase 2：安全边界加固（Sprint 34-35）**

P1 项目全部完成。预期交付：
- `forge validate` 命令可用，输出配置完整性报告
- 子进程环境变量从「全量继承」降为「白名单过滤」
- trace 事件携带运行时策略快照，仿真引擎的前提条件满足
- agent 编辑循环在违反包文件数/函数长度红线时立即收到反馈

**Phase 3：可观测性进化（Sprint 36-38）**

P2 项目开始推进。预期交付：
- 30+ 天长运行的 trace 归档不炸内存
- 部分写失败的错误分类 + 自动清理（`git checkout --affected-files`）
- argv allowlist 执行，非白名单命令被阻止并记录 trace
- `CommandExecutor` 接口化，为容器执行后端铺路

### 5.3 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| **R1：环境变量白名单破坏子进程运行** | 中 | 高 | `--preserve-env` 作为紧急回退；白名单逐步缩紧（先 warn 后 block）；CI 中运行适配测试矩阵 |
| **R2：`project.yml` 校验阻断现有项目运行** | 高 | 中 | `FORGE_ALLOW_UNVALIDATED_CONFIG=1` 环境变量绕过校验；校验规则仅对新增字段严格，对旧字段语义保留兼容 |
| **R3：trace 分段归档破坏下游消费工具** | 中 | 高 | 分段文件命名规则对旧 `trace.jsonl` 透明；`scorecard rebuild` 优先扫描 `trace.jsonl`，找不到再遍历 `trace-NNN.jsonl` |
| **R4：gate-fast.mjs 的额外 hook 增加 CC 响应延迟** | 低 | 中 | 严格控制 fast-path 检查不超过 50ms；超时后 fallback 到「不做检查」而非阻断编辑 |
| **R5：`KindResourceExhausted` 的退避策略触发用户感知延迟** | 中 | 低 | 退避策略参数可配置（环境变量）；显著退避间隔（>30s）在 stderr 输出进度信息 |

### 5.4 关键里程碑

```
M0（Sprint 32 末）: 第一个 P0 落地 —— project.yml mode/lifecycle 校验
    验收指标: `lifecycle: "produktion"` → `forge run` 拒绝 + 清晰错误消息

M1（Sprint 33 末）: P0 三角基座完成
    验收指标: 三个 P0 闸门触发时 agent 零知识终止而非静默退化

M2（Sprint 35 末）: 安全边界加固完成
    验收指标: 子进程环境变量白名单生效；`forge validate` 输出完整 JSON 报告

M3（Sprint 38 末）: Phase 3 完成
    验收指标: trace 分段归档在 30 天长运行中稳定工作；argv allowlist 阻止未授权命令
```

---

## 六、关于设计哲学迁移的补充说明

文档引言中提到的「用户补充的 fail-open → fail-closed 迁移」是一个值得 ADR 记录的设计决策。这个迁移不是技术性的——不需要复杂的重构——而是**契约性的**：它改变了用户对「配置错误时 ForgeOS 如何反应」的期望。

当前 `mode.go` 的设计是 **fail-open by default**（未知输入 → 零值 → 全开 → 继续运行）。安全性靠用户输入正确值来确保。方向二建议的 `project.yml` schema 校验将默认策略翻转为 **fail-closed**（未知输入 → 校验失败 → 不运行 → 报告错误）。

这个迁移的微妙之处在于：它不是在所有路径上都 fail-closed。推荐的设计是：

| 调用路径 | 策略 | 理由 |
|---------|------|------|
| `forge validate` | **fail-closed** | 验证的目的就是发现所有错误 |
| `forge run` | **fail-closed for ERROR, fail-open for WARN** | 严重配置错误应该阻止运行；警告级别的错误可以继续 |
| `forge evolve` | **fail-closed (same as run)** | 24h 无人值守应该在起点就阻断错误配置 |
| `forge test` | **warn only** | 测试不应被配置验证阻断 |

这种**上下文感知的校验策略**——同一个 Validate 函数根据调用上下文输出不同 severity 的阻断级别——是 fail-open 和 fail-closed 之间的务实折中。它不需要在 ADR 中声明「我们不再容忍错误配置」，而是声明「我们根据运行上下文动态选择容错级别」。这样的表述更容易被现有用户接受，也保留了 fail-open 在特定场景（如 `forge test`）下的合理性。

---

## 总结：架构评级与建议

从架构成熟度看，ForgeOS 在 Sprint 31 这个阶段处于**架构青春期**——核心抽象正确，扩展点预留充分，但在治理执法、配置验证、安全边界三个领域有可量产的架构债务。五个扫描方向精准地定位了这些债务：

| 维度 | 评级 | 关键行动 |
|------|------|---------|
| 分层与依赖 | ✅ 健康 | 保持现有包边界，不因扩展而打破 |
| 配置面完整性 | ⚠️ 需加固 | Phase 1 引入校验层 + Phase 2 引入统一配置面 |
| 可观测性演进能力 | ❌ 需重建 | Phase 3 引入 schema registry + 版本契约 |
| 治理执法自动化 | ⚠️ 需升级 | Phase 1 的增量预警告警 + Phase 2 的 fast-path 接入 |
| 安全边界 | ⚠️ 需缩紧 | Phase 2 的环境 + argv 白名单；Phase 3 的接口化 |

建议 Phase 1（Sprint 32-33）聚焦治理可信基座，以「`project.yml` 校验 + `KindResourceExhausted` + gate 预警告警」三角为起点，在每 Sprint 末通过 `forge accept` 全量门控验证。不引入任何新的技术栈。最迟在 Sprint 33 末完成 ADR-005，正式记录 fail-open → context-aware fail-closed 的设计哲学转向。
