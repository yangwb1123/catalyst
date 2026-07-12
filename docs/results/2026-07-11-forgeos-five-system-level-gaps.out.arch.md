现在我对项目架构有了全面理解。以下从架构师角度进行系统分析。

---

# 架构分析报告：ForgeOS 运行时前沿验证

## 一、架构评估

### 当前架构的优势

ForgeOS 的架构有一个被验证报告间接揭示但尚未被明确表述的**结构性优势**：**双平面隔离作为架构稳定性杠杆**。

```
         控制面 (forge-core Go 运行时)
    ┌────────────────────────────────────┐
    │ Orchestrator · Router · Context    │
    │ Memory · Eval · Converge           │
    │ (13 packages, 0 external deps)     │
    └──────────────────┬─────────────────┘
                       │ shell exec
    ┌──────────────────▼─────────────────┐
    │ 数据面 (Harness: Node/Python)      │
    │ gate.mjs · check.py · acceptance   │
    │ · secret-scan · arch-check         │
    └────────────────────────────────────┘
```

这个分离带来的好处验证报告中未充分讨论：

1. **治理独立性**：Harness 在 Node/Python 中，forge-core 在 Go 中，使治理层可以在不触及运行时核心的前提下独立演化。方向一（模板漂移）问题之所以可被接受暂时存在，正是因为控制面与治理面的分离松耦合——模板更新只需要更换 harness 资产，不触及 Go 二进制。

2. **零依赖纪律**：Go 运行时 13 个包、`go.mod` 无 `require`。这在当前 Go 生态中极为罕见。作为架构决策（D6），它在 v2 起步阶段是对的：避免依赖膨胀导致「后续每加一个依赖都要重新审视」的决策成本。但到 v3 目标架构（需要 Temporal、NATS、Qdrant 等），这个纪律需要显式升级——从「零外部依赖」到「审慎外部依赖」。

3. **载重墙模型**：`AGENTS.md` 定义了带外执法的真相之源，CC hook 只是加速器。这是正确的架构原语——不依赖任何具体宿主的能力边界来保证治理执行。但这个模型的弱点是方向一揭露的：**治理资产本身需要被治理**。

### 关键设计决策合理性评估

| 决策 | 合理度 | 备注 |
|------|--------|------|
| D1: Go 核心 polyglot | ✅ 正确 | 验证报告方向三的问题（硬编码语言模板）不是 Go 的问题，而是 `forge-init` 接线问题 |
| D2: 先复用 CC 原生 | ✅ 正确 | 与验证报告方向四一致——过早引入多仓库编排只会膨胀 v2 |
| D3: 带外 gate 为真相 | ✅ 正确 | 方向五的测试缺口不否定这个模式，只是执行深度问题 |
| D4: v1 限 Claude | ✅ 正确 | 方向二冷启动问题与此无关；模型池是 v3 话题 |
| D5: 先做 Context+Harness | ✅ 正确 | 已验证止血完成 |
| D6: v2 forge-core 启动 | ⚠️ 合理但有遗留 | 验证报告方向一、二的缺口正是这个决策的未完成部分 |

### 架构债务识别

**P1 级债务**：

1. **无版本契约链**（方向一核心）：`forge-core` 与项目治理资产之间没有显式版本绑定。这不是一个实现细节，而是一个**架构裂缝**——当 `forge-core` 二进制升级而 `.agent/` 资产没同步升级时，系统行为是未定义的。更严重的是，由于缺失 `.forge-version`，项目负责人无法判断「断连」已经发生了。

2. **任务 Ledge 空值路径未闭合**（方向二核心）：`prompt.go` 中 `currentTask` 返回空字符串 → `cache.go` 跳过整个 prompt 注入。这本质上是**空值传播未被架构防御**——不是 bug，而是一个缺少 null object pattern 或 fallback 策略的设计缺口。

**P2 级债务**：

3. **语言抽象层未接入 CLI**（方向三）：`harness/adapters/` 下有三份 YAML 定义，但 `forge-init` 无视它们。这是**声明了接口但未实现消费者**——资产存在性不错，但它在架构上制造了一个「声称支持但实际不工作」的认知负载。

4. **单项目作用域硬编码**（方向四）：`.forge/` 的路径逻辑（`main.go:454-458`）是硬编码的 repo-scoped。不是它错了，而是它在没有预留扩展点的情况下做了这个假设——未来改多仓库时这是一个 breaking change。

**P3 级债务**：

5. **YAML Python shim 临时脚手架**（ROADMAP 已诚实标注）：非 Go 依赖的临时方案，但积累的认知负载是真实的——每个开发者都需要知道有一个 `python3 harness/yaml2json.py` 在跑。

---

## 二、扩展方向

### 方向 A：版本锚定与漂移检测系统（P1）

**为什么需要**：
治理资产（`.agent/` 模板、策略、workflow）与 `forge-core` 运行时存在隐式版本耦合。当前没有任何机制能检测到「我用的 forge-core v2.3 但我的治理资产是从 v2.0 生成的」。这是一个信任断层——随着项目数增长，治理漂移会从「罕见异常」变成「常态」。

**核心挑战**：
1. 版本编码位置：Go 二进制内置版本 vs 文件系统 `.forge-version` vs git tag
2. 漂移检测的粒度：是整包哈希比较，还是逐文件 checksum？
3. 降级策略：检测到漂移后是 blocking（拒绝运行）还是 warning（日志告警）？

**预期架构变更**：
```
forge-core/
  cmd/forge/main.go
    └── 启动时: read .forge-version → compare with embedded version → warn/block
    └── new subcommand: forge doctor --check-governance-drift

harness/scaffold/forge-init.mjs
    └── 创建项目时: write .forge-version (embedded SHA from source)

.forge/                          # 新增文件
  version.json                    # { "forgeCore": "2.3.0", "governanceDigest": "a1b2c3..." }
```

**对现有系统的影响**：低。不影响任何现有功能，只是新增防卫层。

**实现选项与权衡**：

| 方案 | 优势 | 代价 |
|------|------|------|
| A1. 简单版：`.forge-version` + `forge doctor --check` | 极低侵入，快速落地，可逆 | 不防运行时 drift，仅「按需检查」 |
| A2. 强版：`forge run/evolve` 启动时自动检查 + mismatch 阻断 | 防患于未然 | 打破已有工作流；需 version 格式标准化 |
| A3. 极强版：每份治理文件嵌入 hash，运行时逐文件验证 | 精确检测到哪个文件漂移 | 性能开销，hash 维护成本 |

**推荐**：先 A1 落地（v2 周期内），A2 作为 v3 的开关选项提供给 enterprise 用户。

---

### 方向 B：任务 Ledge 弹性策略（P1）

**为什么需要**：
方向二揭示的冷启动问题不只是「首次体验」问题——当一个项目的 ROADMAP 完成度达到 100% 后再次 `forge evolve`，同样会触发空任务路径。这是一个**边界状态处理缺失**，在自治系统中导致静默错误（agent 收到 prompt 但没有任务，产生随机行为）。

**核心挑战**：
1. 空任务的语义是什么：「所有任务已完成，退出」vs「需要生成新任务」
2. 过渡到 Discover 阶段的触发条件定义
3. 与 `converge.go` 的交互：当 `RoadmapCompletion == 100%` 而仍被触发，系统应该做什么？

**预期架构变更**：
```
forge-core/internal/prompt/
    └── currentTask(): 当 ROADMAP 为空或 completion==100% 时，
        不再返回 ""，而是返回一个 sentinel task:
        { type: "discover", description: "All roadmap tasks complete. Trigger gap analysis." }

forge-core/internal/orchestrator/
    └── 识别 sentinel task → 触发 Discover workflow 而非 Build workflow

internal/converge/
    └── 当检测到 completion==100% 且无待办 → 将收敛判定从 MET 转为
        "EVOLVE_NEEDED" 或 "IDLE"，而非返回 0
```

**对现有系统的影响**：中。需要对 `converge` 和 `prompt` 模块的接口协议做小幅扩展，但不影响已有行为——sentinel task 的引入是向后兼容的。

---

### 方向 C：多语言治理适配器接入点（P2）

**为什么需要**：
方向三的问题本质不是「支持哪些语言」，而是**用户面对的是一个声称 multi-language 但实际只提供 Node.js 模板的系统**。这造成信任损失，且随着系统用户从 forge-core 自身扩展到外部项目，成为 onboarding 摩擦点。

**核心挑战**：
1. `forge-init --lang` flag 需要维护语言特定的 scaffold 模板，而不只是调用 adapter YAML
2. 种子 app 的质量需要与当前 Node.js 版本持平（有测试、有 39 个测试的 url-shortener 级别）
3. 语言特定治理（lint/typecheck/build）的适配器 YAML 需要被 `forge check` 实际消费，而不只是声明

**预期架构变更**：
```
forge-core/cmd/forge/main.go
    └── forge init --lang <go|python|typescript|node> (默认 node)

harness/scaffold/
    forge-init.mjs
        └── 根据 --lang 参数加载不同模板:
            templates/{go,python,typescript,node}/
                ├── src/
                ├── test/
                └── .agent/  (language-specific policies)

harness/adapters/
    go.yml            ← 从声明变为被 forge check 实际调用
    python.yml
    typescript.yml
```

**实现选项与权衡**：

| 方案 | 优势 | 代价 |
|------|------|------|
| C1. 先加 `--lang` flag + Node/Python/Go 三个种子模板 | 快速覆盖主流语言 | 种子模板代码质量不一致 |
| C2. 完全通用化：template registry + ADR 定模板契约 | 可扩展，社区可贡献 | 过度工程——v2 阶段不需要插件系统 |
| C3. 仅修复 Node.js 模板的缺失项 + 加文档说明如何手动适配 | 最小改动 | 用户仍需要自行解决适配 |

**推荐**：C1 路径——Python 和 Go 模板已有 YAML 定义，加上种子 app 的工作量可控。TypeScript 可延后至有真实用户需求。

---

### 方向 D：多仓库预留契约（P3）

**为什么需要**：
方向四提出的 `peer.json` 最小预留是成本极低但收益长远的架构投资。ADR 0003（git submodule 机制）已设计就绪，但被搁置到「被治理项目 ≥ 2～3」。问题是：**即使没有多仓库，单项目也需要为未来多仓库预留接入点**——否则 v3 迁移时所有路径都会 break。

**核心挑战**：
1. 预留的内容：`.forge/peer.json` 的 schema 定义
2. 运行时行为：当 `forge-core` 当前不读 `peer.json` 时，解析这个文件的代码是 dead code——如何管理？
3. 与 ADR 0003 submodule 方案的衔接

**预期架构变更**：
```
.forge/
    peer.json          ← 新增可选文件，当前 forge-core 不读但不过滤

forge-core/cmd/forge/main.go
    └── forge doctor --check-peers  (当前仅验证语法，不执行逻辑)

internal/trace/        ← 已有的 trace 包，可扩展 peer tracking
```

**推荐方案**：在 `.forge/` 目录增加一个可选的 JSON schema 文件，并确保 `forge doctor` 验证其语法合法性。不需要运行时解析。这等价于一个**预留的接口声明**，零运行时成本。

---

### 方向 E：CLI 接线层故障注入框架（P2）

**为什么需要**：
方向五修正后，故障注入的核心缺口不是 orchestrator 内部逻辑（已有 `seqExecutor`），而是 **CLI 接线层**（`cmd/forge/`）以及**真实进程输出解析路径**。这两层是事故多发地带——负载检测、过载分类、retry 决策的「最后一公里」是 Go 标准库 `os/exec` + stdout 解析，目前没有任何测试覆盖。

**核心挑战**：
1. `cmd/forge/` 的接线测试需要 fake CommandExecutor 来注入 stdout/stderr/exit code
2. 输出解析路径（`classifyClaudeOverload`、`observeFor`）需要 YAML 驱动的 fixture
3. 幂等性测试需要模拟「部分写完成」的文件系统状态

**预期架构变更**：
```
forge-core/cmd/forge/
    └── 引入 CommandExecutor 接口（当前已隐式存在，但未分离）
    └── 接线层测试使用 FakeCommandExecutor + YAML fixture

forge-core/testdata/
    executor_fixtures/
        overload-scenario-1.yaml    ← 过载输出模拟
        partial-write-scenario.yaml ← 部分写完成模拟
        success.yaml                ← 正常输出模拟
```

**核心原则**：不要为测试而重构生产代码，而是**通过接口分离来使现有逻辑可测**。`os/exec` 调用已经通过 `CommandExecutor` 接口抽象——目前的缺口是 `cmd/forge/` 层的接线代码尚未使用这个接口（orchestrator 内部已经用了）。

---

## 三、接口设计建议

### 关键模块接口原则

基于验证报告揭示的问题，提出以下接口设计原则：

#### 原则 1：所有入口路径必须有明确的 empty/null 行为定义

方向二的问题（空 ROADMAP → 空 task → 跳过 prompt）的根因是：没有在接口契约层面定义 `currentTask("")` 的语义。这是 Go 接口设计中「零值可用」的一个反面案例——不是所有零值都是有效的。

**具体建议**：

```go
// 当前（隐式契约，不安全）
func currentTask(repoRoot string) string

// 建议（显式契约，安全）
type Task struct {
    Description string
    Type        TaskType  // Build | Discover | Idle
    Priority    int
}
type TaskType int
const (
    TaskBuild    TaskType = iota // 正常 Build 任务
    TaskDiscover                 // ROADMAP 完成，需要发现新任务
    TaskIdle                     // 无任务可做，系统空闲
)

func currentTask(repoRoot string) (*Task, error)
```

#### 原则 2：版本契约应作为运行时的一等接口

方向一的根因是版本信息散落在二进制（`sourceSha()`）和文件系统（COPIED_FILES 逻辑）之间，没有一个统一的版本契约接口。

**具体建议**：

```go
type VersionContract struct {
    CoreVersion      string `json:"coreVersion"`
    GovernanceDigest string `json:"governanceDigest"` // .agent/ 整树 hash
    CreatedAt        time.Time
}

// 写入: forge init → .forge/version.json
// 读取: forge run/evolve/doctor → 启动时验证
// 比较: 内置 coreVersion vs 文件中的 coreVersion
```

#### 原则 3：扩展点应预留声明而非实现的接口

方向四的教训：多仓库在 v2 阶段不应实现，但不应没有预留声明。

**具体建议**：在 `.forge/` 下增加可选接口声明文件，当前 forge-core 不实现、不加载、不依赖。只是保留命名空间和 schema。

---

### 是否需要新的抽象层

**需要**：一个**治理契约层**（Governance Contract Layer），位于 forge-core 运行时与 `.agent/` 资产之间。

当前架构：
```
forge-core 运行时  ─── 隐式依赖 ───→  .agent/ 资产
```

建议架构：
```
forge-core 运行时  ─── 验证 ───→  治理契约层  ─── 声明 ───→  .agent/ 资产
                                    ↑
                               .forge/version.json
                               .forge/policies.lock
```

这个抽象层的职责：
- 启动时验证治理资产与运行时的兼容性
- 提供版本升级的迁移路径（当 `.agent/` schema 变化时）
- 多仓库场景下，验证跨项目的治理一致性

**注意**：这不应是 v2 的紧急工作，但在 v2 晚期（或 v3 初期）引入这个抽象层，可以避免 v3 时治理资产成为「不可迁移的泥潭」。

---

## 四、技术选型

### 需要引入的新技术

| 技术 | 对应方向 | 推荐时机 | 理由 |
|------|----------|----------|------|
| Go YAML 解析库 | 整体（替代 Python shim） | v2 late | 移除临时脚手架，减少认知负载 |
| 文件级 checksum 工具（Go 标准库 `crypto/sha256`） | 方向一 | v2 now | 零依赖，Go 标准库自带，即刻可用 |
| OPA/Rego（已有声明，未集成） | 方向一 + 方向四 | v3 | v2 阶段 governance 策略还不够复杂到需要专用规则引擎 |

### 不必引入的技术

| 技术 | 为什么不需要 |
|------|-------------|
| YAML fixture 驱动的测试框架（类似 go-cmp） | 方向五的问题可完全通过 Go 标准库 `testing` + `os/exec` 的 `Cmd` 接口 mock 解决。不需要第三方框架 |
| Terraform/Pulumi 管理 .agent/ 资产 | 过度工程——治理资产的「状态管理」在 v3 之前不需要 IaC 级别的能力 |
| 分布式版本追踪（类似 git-lfs） | 多仓库编排在 v3 之前不需要文件级版本追踪 |

### 自建 vs 采购决策

验证报告的所有方向都指向**自建**：

| 方向 | 为什么不适合采购 |
|------|-----------------|
| 版本锚定 | forge-core 的版本策略高度定制，没有现成工具可以「治理 AI 治理系统的版本」 |
| 冷启动弹性 | 这是 prompt 语义问题，必须自建 |
| 多语言模板 | 已有 YAML 定义，只是需要接线 |
| 多仓库预留 | 预定死接口声明，没有采购价值 |
| CLI 测试框架 | Go 标准库 + YAML fixture 完全够用 |

判断标准：「如果一个方案涉及 forge-core 自身的核心语义（运行时编排、治理模型、context 装配），就自建；如果只是通用基础设施（YAML 解析、HTTP 代理、认证网关），就采购。」

---

## 五、实施路线图

### 优先级总览

```
P0 (必须 v2 内完成)
└── 方向 A: 版本锚定 + forge doctor drift 检测
└── 方向 B: 空任务 sentinel + 过渡到 Discover

P1 (v2 可选但强烈推荐)
└── 方向 E: CLI 接线层故障注入框架
└── 方向 C: forge-init --lang (至少加 Python)

P2 (v3 前完成)
└── 方向 D: 多仓库预留 + peer.json schema
└── Go YAML 解析库引入 (替代 Python shim)
```

### 详细里程碑

#### 阶段 1：防御层（2–3 周，P0）

```
Milestone: "No silent failures"
├── .forge-version 写入 (forge-init.mjs)
├── forge doctor --check-governance-drift (Go, 纯 stdlib)
├── currentTask() sentinel 空任务处理 (prompt.go + converge.go)
└── 所有现有的 acceptance 测试全绿
```

风险：低。纯新增代码，不修改现有行为。

#### 阶段 2：可测性（2–3 周，P1）

```
Milestone: "Known failure modes are tested"
├── CommandExecutor 接口提取 (cmd/forge/)
├── YAML fixture 驱动接线测试 (testdata/executor_fixtures/)
├── 过载分类、retry 全链路的集成测试
└── 幂等性测试（部分写完成快照 vs 恢复）
```

风险：中。接口提取可能涉及重构，但 CommandExecutor 已经是一个隐式接口，显式化后风险可控。

#### 阶段 3：语言覆盖（2–3 周，P1）

```
Milestone: "Multi-language scaffolding works"
├── forge-init --lang <go|python|node>
├── Python 种子 app (等价于 url-shortener 级别)
├── Go 种子 app (等价于 url-shortener 级别)
├── adapters/{go,python}.yml 被 forge check 实际消费
└── 语言适配器文档 + 贡献指南
```

风险：中高。种子 app 的质量直接暴露给用户——错误模板比没有模板的伤害更大。需要对种子 app 做完整的 `forge accept` 验证。

#### 阶段 4：未来预留（1 周，P2）

```
Milestone: "v3-ready foundations"
├── .forge/peer.json schema 定义 + forge doctor 验证
├── ADR 0003 更新（版本契约 + 预留接口）
├── Go YAML 解析库引入（零外部依赖原则升级为审慎外部依赖原则）
└── 替换 Python shim
```

风险：低。但 Go YAML 库的引入需要 ADR 记录——这是 forge-core 的第一个外部依赖，需要明确选型标准。

---

### 风险矩阵

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|----------|
| 方向 A 的版本比较触发误报（用户本地的治理资产与 forge-core 版本实际兼容但签名不匹配） | 中 | 中（用户信任损失） | 版本比较用 semver range 而非 exact match；误报时提供 `--override-version-check` flag |
| 方向 B 的 sentinel task 触发不该触发的 Discover 阶段（用户在 mid-Build 时 ROADMAP 为空导致意外跳转） | 低 | 高（工作流被打断） | sentinel 只在 `RoadmapCompletion == 100%` 时触发；添加 `forge evolve --dry-run` 预览模式 |
| 方向 C 的多语言种子 app 质量不一致导致用户对 forge-core 的信任降低 | 中 | 中 | 每个种子 app 都要过完整的 `forge accept`；设最低质量标准（≥ 20 测试，无硬编码 secret，arch-check 全绿） |
| Go YAML 库引入破坏了「零外部依赖」纪律，团队心态从「谨慎」转为「随意加依赖」 | 低 | 高（架构腐化） | ADR 记录选型标准和引入理由；设依赖审查门（每周 review `go.mod`，每次新增需 architect 批准） |

---

## 六、未被谈论的盲点

验证报告的结尾提到一个「结构性盲点」。我推测这个盲点是：

**Harness 的自举问题**（Bootstrapping the Governor）

ForgeOS 的架构假设是：Harness（gate.mjs、check.py、acceptance.mjs）是未被治理的治理者。它们相对于 forge-core Go 运行时是「带外」的——这意味着：

1. **没有机制保证 Harness 自身的质量**：谁给 gate.mjs 跑 gate？谁给 check.py 跑 check？
2. **Harness 的语言栈（Node/Python）与 forge-core（Go）分离**，意味着版本兼容性矩阵是 2×N——N 个 Go 版本 × Node 版本 × Python 版本。
3. **目前 `forge accept` 的聚合闸门（gate + arch-check + check.py + secret-scan + test + app-test）是 orchestration，不是 verification**——它调用各工具但自己不验证这些工具本身是否正常工作。

这个自举盲点的架构后果是：当治理本身出错时（例如 `gate.mjs` 的阈值被不小心改低、`check.py` 漏掉一个检查），**没有任何自动防护**——因为防护层本身就是出问题的组件。

这不是一个 P0 问题（每次 CI 跑 `forge accept` 时，如果 Harness 损坏，CI 会红），但它是 v3 需要解决的一个架构级挑战：**如何让治理者的自举成为可验证的 CI 环节？**

一个可行的方向是：在 CI 中增加一个 `forge doctor --self-test` 阶段，它运行 Harness 的全部工具在一个已知通过/已知失败的测试 repo 上，验证每个工具的输出是否与预期一致。这相当于 Harness 的「冒烟测试」——不需要完整的元治理体系，但能捕获大多数无声故障。
