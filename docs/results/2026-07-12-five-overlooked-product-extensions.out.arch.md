现在我有了对 ForgeOS 架构的深刻理解。以下是我的架构分析：

---

# 架构分析：ForgeOS 产品扩展方向

> **角色**：资深架构师  
> **分析基础**：完整通读 ForgeOS 代码库（`forge-core/` 13 包, `harness/`, `.agent/`）+ Sprint 1-31 演进记录 + 二轮审阅文档 + Tech Lead 实施分析  
> **范围**：不聚焦单个方向的可行性，而是审视五方向背后的**系统级架构含义**

---

## 1. 架构评估

### 1.1 当前架构的核心优势

ForgeOS 当前架构呈现了罕见的**架构成熟度**——在 AI-native 系统领域几乎独一无二：

| 优势 | 具体表现 | 架构含义 |
|---|---|---|
| **信任最小化收敛控制** | `LoopEngine` + 带外 `converge` + 诚实代数（N/A ≠ PASS） | 执行器被排除在自身误差测量之外——这是对不可靠生成式执行者的 **SRE 级控制理论应用** |
| **中枢旋钮（mode×lifecycle）** | 一个设置同时驱动 Router + Harness 严格度 + Workflow 深度 | 复杂系统的**正交控制面**——大幅降低认知负荷 |
| **零外部依赖 Go 运行时** | 13 个包、`go.mod` 零 require | 可审计、可静态链接、无供应链风险——企业级基础 |
| **诚实文化制度化** | N/A 诚实标注、`ACTUAL EFFECT MAY VARY` 声明、`honesty` 作为测试断言 | **元信任**——系统不仅正确，还知道自己的局限 |
| **gate 三层执法** | edit-time（加速器）+ Stop（forge accept）+ CI | 纵深防御，且 host-independent |

### 1.2 关键设计决策的合理性

逐一评估核心设计决策：

| 决策 | 合理性 | 潜在代价 |
|---|---|---|
| **Go 自研编排 vs 复用 Temporal** | ✅ 当前正确：v2 阶段 13 包零依赖的薄控制面足够；Temporal 留给 v3 分布式 | 未来 Temporal 接入时 orchestration 逻辑需重构 |
| **YAML 解析走 Python shim** | ⚠️ 技术债务，但理性：Go 标准库无 YAML；引入外部 YAML 库会打破「零依赖」承诺 | 每 `forge run/evolve` 多一次进程 fork；路径依赖需在 v3 前解决 |
| **单趟 CLI 架构（forge run/evolve）** | ✅ 正确：无状态 CLI 比 daemon 更简单、更可审计、更易调试 | 无法支持 websocket 推送、实时协作、长时间等待 |
| **dry-run 默认** | ✅ 安全第一：默认不调 LLM，显式 `--executor command --agent-cmd claude` 才真跑 | 增加 onboarding 摩擦——但这是可接受的安全税 |
| **Python 在 harness 栈中** | ⚠️ 混合运行时：`check.py`、`yaml2json.py` 增加部署复杂度 | 目标仓需要 Python 运行时；`forge-init` 依赖 Python 可用 |

### 1.3 架构债务（技术债）

按严重程度列出：

| 债务 | 位置 | 严重度 | 触发条件 | 建议消除时间 |
|---|---|---|---|---|
| **Python shim 作为唯一 YAML 解析路径** | `harness/yaml2json.py` → Go 进程间 pipe | **高** | 每次 `forge run/evolve` 的冷路径——虽已缓存 JSON，但首次仍 fork Python | v3 前（引入 Go YAML 库或嵌入 yaml 解析） |
| **无跨平台进程锁** | 方向一缺口 | 中 | 多个 `forge evolve` 在同一个仓库上并行——当前无防护 | 方向一实施时一并解决 |
| **契约解析器散落在 `cost.go` 中的 switch 语句** | `parseReviewerVerdict`/`parseExecutiveVerdict`/`parseConfidenceScore` | **中** | 新增 agent 类型时需加 switch 分支；匹配规则硬编码 | 方向四实施时解决 |
| **`cmd/forge` 包持续逼近文件数上限** | 16 文件/17 门限 | **中** | 每新增一个 CLI 命令就需要一次「拆包」操作 | 长期：将 CLI 层和逻辑层更彻底分离 |
| **`converge.Signals` 仍有两个赋值处与消费者之间的间隙** | `RequirementConfidence`/`FileDelta` | 低 | Sprint 29-30 已修，但未经过真 agent 端到端验证 | 下一次真点火前确认 |
| **无配置校验的版本号** | 版本需求读取路径无 schema 校验 | 低 | 方向二的版本需求配置化——写错格式时静默兜底 | 方向二实施时解决 |

### 1.4 整体架构评级

```
可扩展性:      ████████░░  8/10  — 包边界清晰,接口正交
可测试性:      █████████░  9/10  — 零依赖 + 依赖注入 + fake/stub 模式成熟
可部署性:      ████████░░  8/10  — 单静态二进制(Go) + harness(Python/Node 依赖)
可观测性:      ██████░░░░  6/10  — trace/checkpoint/memory 已落地,但缺 run_id、缺存储健康、缺告警
安全性:        ███████░░░  7/10  — 硬 Opus 底线 + secret 扫描 + 递归防护,但缺策略审计追踪
治理完整性:    █████████░  9/10  — 架构 8 检查 + 治理完整性检查 + 中枢旋钮,行业领先
诚实度:        ██████████ 10/10  — honesty 是代码的 first-class 概念,非附属品质
```

---

## 2. 扩展方向分析

五个方向并非对等——它们的**架构深度**差异巨大。我将它们重新映射到两个不同层级的扩展：

### 2.1 层级归位

```
层级 1 — 水平面（基础设施/运营能力）
  ├── 方向四（策略审计追踪） → 治理信任根
  ├── 方向一 v1（Run Identity） → 可观测性基础设施
  └── 方向五（存储生命周期） → 运营成熟度

层级 2 — 核心语义变更（改变系统行为模型）
  ├── 方向二（本地 LLM 离线模式） → 执行模型扩展
  └── 方向一的「部分接受」→ 原子性模型变更
```

这一区分至关重要：**层级 1 的问题是「系统是否成熟」；层级 2 的问题是「系统是否正确」**。审阅文档没有明确区分这两者，导致优先级排序出现混乱。

### 2.2 五个高价值架构扩展方向

基于上述归位，重新排列为 5 个架构扩展方向（与输入文档的五方向不同，这里是**我的架构观点**）：

---

#### 方向 A（P0）：策略治理信任根 —— `forge-core/internal/trusted/`

**为何需要**：当前的治理模型存在递归信任问题——`policies.yml` 由谁保护？如果 agent 可以写 policies 文件，它就可以同时篡改政策和校验和。企业客户购买的不是工具而是**信任**——这是最重要的架构盲区。

**核心挑战**：
1. **自治 vs 不可篡改性**：ForgeOS 的核心承诺是 agent 可以修改文件——但策略文件必须例外
2. **签名链**：需要 Ed25519 签名，但密钥管理超出 CLI 工具的范围
3. **递归信任**：谁保护签名工具本身？

**建议架构方案**：

```
.forge/trusted/            ← agent 不可写
  ├── policies.signed.yml   ← 外部签名的策略（forge doctor --policy-sign）
  ├── policies.sig          ← Ed25519 签名
  └── trusted_keys/         ← 信任根公钥
.forge/policies.yml         ← agent 可编辑的工作副本（仅提示用）

运行时加载路径：
forge run/evolve
  → load .forge/trusted/policies.signed.yml
  → verify signature against trusted_keys/
  → if valid: enforce signed policies + warn about local override
  → if invalid/missing: fall back to local policies.yml + audit warning
```

**关键设计决策**：签名验证不能阻止 agent 忽略策略——但提供了**可审计的根信任边界**。与 `checkpoint.go` 的 atomic write 模式一致：不是完美防护，但使攻击可被检测。

**预期变更**：
- 新建 `internal/trusted/` 包（纯标准库 Ed25519 验证）
- `forge doctor --policy-sign` CLI 命令（外部签名工具）
- `gate/resolve.go` 加载路径改造（优先加载签名策略）
- `prompt_context.go` 注入「你不能修改 .forge/trusted/」系统提示

**对现有系统的影响**：最小——纯新增功能，不改变现有 policies.yml 加载路径。现有行为保持不变（无 signed 文件时 fallback）。

---

#### 方向 B（P0）：本地 LLM 离线模式 + 能力感知闭环

**为何需要**：没有它，整个企业市场不可用。但更关键的是——审阅文档指出的**动态连锁反应**是真正的新见解：本地模型能力不足 → 更多 loop-back → 更多 token 消耗 → 成本不降反升。

**核心挑战**：
1. **TierFor 接口扩展**：当前签名 `(agent, mode) → Tier`，需要第三个维度 `backend: local|cloud`
2. **能力建模**：需要为每个本地模型建立**能力因子**（推理质量、输出确定性、上下文窗口）
3. **动态补偿**：当 loop-back 率 > 历史基准 3x 时，自动触发 mode 切换或告警
4. **本地执行器**：`CommandExecutor` 需要 LocalModelExecutor 变体（ollama/vllm/llama.cpp）

**建议架构方案**：

```go
// internal/model/capability.go — 能力建模（新包）
type ModelCapability struct {
    Provider          string  // "ollama", "vllm", "openai", "anthropic"
    ModelName         string  // "llama3.1-70b", "claude-opus-4"
    Tier              Tier    // haiku/sonnet/opus 映射
    InferenceCost     float64 // 每 token 成本（美元）
    ContextWindow     int     // 上下文窗口大小
    ReliabilityScore  float64 // 0-1, 基于历史 loop-back 率的动态评分    
}

// 动态补偿
type CapabilityAwareLoopEngine struct {
    *LoopEngine
    historicalLoopbackRate float64
    baselineLoopbackRate   float64 // 云模型的历史基准
}

func (e *CapabilityAwareLoopEngine) compensate() {
    if e.historicalLoopbackRate > e.baselineLoopbackRate * 3 {
        // 自动降级 mode 或触发告警
        e.mode = degradeMode(e.mode)
    }
}
```

**对现有系统的影响**：**最大**——涉及 `orchestrator/`、`routing/`、`waves/` 三个包的接口变更。工作量估计 ~5 sprints。

---

#### 方向 C（P1）：契约注册表 —— 通用解析引擎

**为何需要**：当前 agent 产出解析器散落在 `cost.go` 中的三个 switch 语句，匹配规则硬编码、大小写敏感、缺乏可扩展性。这已经导致真实 bug（Sprint 30 `yaml2json` 测试失效事件——测试未被正确接入断言系统）。

**核心挑战**：
1. **向后兼容**：新引擎必须与旧 switch 行为**逐位一致**
2. **匹配模式设计**：exact / case-insensitive / prefix / regex 四种模式的选择和组合
3. **模糊匹配的 fail-closed 语义**：宁可不匹配也不误匹配

**建议架构方案**——与方向 A（信任根）有接口重叠：

```go
// internal/contract/registry.go（新包，非 asset 包——单一职责）
type MatchMode int
const (
    MatchExact       MatchMode = iota
    MatchCaseFold
    MatchPrefix  
    MatchRegex
)

type TokenDef struct {
    Name     string    // "VERDICT"
    Value    string    // "APPROVE"
    Mode     MatchMode
    Required bool      // fail if missing
}

type AgentContract struct {
    AgentType  string
    Tokens     []TokenDef
    ScopedLast bool    // only scan last non-empty line
}

// Registry loads from .agent/contracts/*.yml once at startup
type Registry struct { ... }
func (r *Registry) Parse(agentType, output string) (map[string]string, []Warning)
```

**对现有系统的影响**：中等——替换 `cost.go` 中的三处 hardcoded switch，需要 A/B 回归测试套件确保行为一致。

---

#### 方向 D（P1）：可观测性统一 —— Run Identity + 存储生命周期

**为何需要**：当前 trace/checkpoint/memory 三者均无 run_id/进程锁/健康告警。多 `forge doctor` 交错运行时 trace.jsonl 的 seq 重置，无法区分来自哪个 run。这是未来任何可观测性分析的基础。

**核心挑战**：
1. **跨平台文件锁**：Linux flock vs Windows LockFileEx
2. **向后兼容**：旧 trace/checkpoint/memory 文件无 run_id 字段时不能崩溃
3. **性能**：每 Emit 注入 run_id 的序列化开销

**对现有系统的影响**：最小——每个存储系统新增一个 optional string 字段 + omitempty。文件锁是新增功能，不影响现有单线程行为。

---

#### 方向 E（P2）：半自治 Co-Pilot（无部分接受）

**为何需要**：降低采纳坡道——允许人类在 agent 执行过程中逐变更审查。但审阅文档指出**部分接受与 Phase 原子性冲突**，这是正确的。

**核心挑战**：
1. **原子性模型**：当前 `asset.Phase` 要么全成功落地（`acceptEdits`），要么全回滚。部分接受需要 redesign converge 语义
2. **收敛歧义**：`roadmap_completion` 从标量变为每位图——`converge.Signals` 的架构级变更
3. **文件系统一致性**：半 phase 状态可能导致工作树不一致

**建议**：方向 E v1 仅支持**全接受/全跳过/全带回**——与当前原子性模型一致。部分接受放入 v2，与 `converge` 的 item 级评估联合设计。

**对现有系统的影响**：中等——新审批流程不改变现有 phase 执行模型，但需要 `forge approve --partial` 的 v2 架构设计。

---

## 3. 接口设计建议

### 3.1 关键原则

| 原则 | 理由 | 例子 |
|---|---|---|
| **所有新增字段 optional + omitempty** | 向后兼容是 ForgeOS 的最高优先级——旧文件永远不能崩溃 | `trace.Event.RunID` 为 `string,json:"run_id,omitempty"` |
| **新功能优先新包，不膨胀现有包** | `cmd/forge` 已达文件数上限；`internal/gate` 刚刚拆出 resolve | `internal/trusted/`、`internal/contract/`、`internal/model/` |
| **解析器统一入口，不新增散落 switch** | 方向四统一为 `contract.Registry.Parse()` | 替换三个散落 parser，而非新增第四个 |
| **配置层级清晰：CLI > 项目 > 默认** | 避免方向五的「配置扩散」问题 | `RetentionPolicy.Merge()` 定义明确的覆盖语义 |
| **fail-closed 重于 fail-open** | ForgeOS 的安全模型——宁可不匹配也不误匹配 | `VERDICT:APPROVE`（缺空格）v1 不匹配，v2 模糊匹配 |

### 3.2 需要引入的新抽象层

| 抽象层 | 包路径 | 理由 | 设计提示 |
|---|---|---|---|
| **信任根** | `internal/trusted/` | 策略签名的验证 + 加载路径——与 gate 执行分离 | 遵循 `checkpoint.go` 的 atomic write 模式 |
| **契约注册表** | `internal/contract/` | agent 输出契约的统一加载 + 解析——替代散落 switch | 保持与 `internal/asset` 的单向依赖（asset import contract） |
| **模型能力建模** | `internal/model/` | 能力因子 + 动态补偿——为方向二的本地模型提供支撑 | 不要与 `internal/routing` 合并（routing 是决策层，model 是描述层） |
| **存储策略** | `internal/persist/policy.go` | retention 配置——延续 `persist` 包的职责边界 | `DefaultRetention()` 必须等于当前硬编码值 |

### 3.3 保持向后兼容的关键接口模式

```go
// 模式 1：Optional 字段 + omitempty（trace.Event 范例）
type Event struct {
    RunID   string `json:"run_id,omitempty"`  // 新字段，旧文件无此字段不崩溃
    Seq     int    `json:"seq"`
    // ... 现有字段
}

// 模式 2：函数参数扩展（不破坏现有签名）  
// 旧签名
func NewTracer(path string) (*Tracer, error)
// 新签名（Option 模式）
func NewTracer(path string, opts ...TracerOption) (*Tracer, error)

// 模式 3：配置优先级链
// retention 优先级：CLI flag > .forge/policy.yml > project.yml > 硬编码默认值
type RetentionPolicy struct {
    Trace      TracePolicy      `yaml:"trace"`
    Checkpoint CheckpointPolicy `yaml:"checkpoint"`
    Memory     MemoryPolicy     `yaml:"memory"`
}
func (r *RetentionPolicy) Merge(src RetentionPolicy) {
    // 只覆盖 src 中非零值的字段
}
```

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈

**核心立场**：ForgeOS v2 的「纯 Go 标准库、零外部依赖」是企业级信任的基础。**不要为了便利性引入新依赖**。

| 场景 | 方案 | 理由 | 例外条件 |
|---|---|---|---|
| **YAML 解析** | 继续 Python shim + 缓存 JSON | 零依赖承诺更重要；YAML 库（如 `gopkg.in/yaml.v3`）是外部依赖 | 当 `python3` 不可用成为用户痛点时，引入 Go YAML 库 |
| **Ed25519 签名** | Go 标准库 `crypto/ed25519` | Go 1.13+ 标准库已包含——零新依赖 | — |
| **正则匹配** | Go 标准库 `regexp` | 方向四的 MatchRegex 模式直接使用 | — |
| **CVE/SCA DB** | 外部 OSV/NVD 数据（框架已就绪） | 数据是外部输入，非依赖 | DB 文件可随 forge-core 分发 |
| **LiteLLM 跨厂商池** | 方向二的本地模型执行器用独立接口抽象 | v3 之前不需要跨厂商池；本地模型先走 ollama API | 当用户要求多厂商路由时引入 |

### 4.2 第三方依赖评估标准

| 标准 | 权重 | 说明 |
|---|---|---|
| **纯 Go 实现** | 强制 | CGo 依赖不可接受（交叉编译、静态链接、安全审计） |
| **标准库替代** | 强制 | 优先用 crypto/ed25519 而非第三方签名库 |
| **零传递依赖** | 高 | 一个第三方库带入的传递依赖可能打破零依赖承诺 |
| **维护状态** | 中 | 活跃维护 + Go 版本兼容性承诺 |
| **许可证** | 中 | 必须是 BSD/MIT/Apache 2.0——禁止 GPL/LGPL |

### 4.3 自建 vs 采购决策矩阵

| 组件 | 方向 | 建议 | 理由 |
|---|---|---|---|
| **YAML 解析器** | 全局 | 继续 Python shim（当前）→ 评估引入 go-yaml v3（v3 前） | 自建 YAML 解析器（yaml2json 已是自建）可消除 Python 依赖 |
| **Ed25519 签名** | 方向 A | 自建 | Go 标准库已包含——零成本 |
| **模型执行器（ollama/vllm）** | 方向 B | 适配器模式 | 不嵌入 ollama——通过 HTTP API 交互，同 CommandExecutor 模式 |
| **进程锁** | 方向 D | 自建 | 用 `syscall.Flock`（Unix）/ `os.Create`+O_EXCL（Windows） |
| **SCA/CVE 数据库** | 方向二 | 采购（OSV/NVD 数据） | 框架已就绪（`sca.mjs`），缺的是 DB 文件 |

---

## 5. 实施路线图

### 5.1 优先级重新排序

我的排序与审阅文档的排序不同——我认为应该**先做架构基础设施（信任根 + 可观测性），再做功能扩展（本地模型 + 半自治）**：

```
Sprint N   (P0): 方向 A（策略信任根 ~1 sprint）+ 方向 D 基础（Run ID ~1 sprint）
Sprint N+1 (P0): 方向 B 基础设施（LocalModelExecutor + 能力建模 ~2 sprints）
Sprint N+2 (P1): 方向 C（契约注册表 ~1 sprint）+ 方向 D 存储生命周期（~1 sprint）
Sprint N+3 (P0): 方向 B 混合路由 + 动态补偿（~2 sprints）
Sprint N+4 (P1): 方向 E v1（无部分接受 ~1.5 sprints）
Sprint N+5 (P2): 方向 B 文档/doctor/environment 检测收尾（~1 sprint）
Sprint N+6 (P2): 方向 E v2 部分接受（~1 sprint，如仍需）
```

### 5.2 阶段划分和里程碑

| 阶段 | 时间 | 核心交付 | 验证标准 |
|---|---|---|---|
| **M1: 信任基础** | Sprint N | 方向 A 签名策略可验证 + 方向 D run_id 注入 + 进程锁 | `forge doctor --verify-policy` PASS；`trace.jsonl` 每行含 `run_id` |
| **M2: 执行模型扩展** | Sprint N+1~N+3 | 方向 B 本地 ollama 执行器 + 能力因子 + 动态补偿 | `forge run --model ollama/llama3` 可在无云 API 下完成简单 implement phase |
| **M3: 治理深化** | Sprint N+3~N+4 | 方向 C 契约注册表 + 方向 E v1 逐变更审批 | 新增 agent card 只需写 `contracts/*.yml` 即可自动解析；`forge approve --partial` 可用 |
| **M4: 运营成熟** | Sprint N+5~N+6 | 方向 B 环境检测 + 方向 E v2 部分接受 | `forge detect` 可准确推荐 mode；部分接受的 converge 语义正确 |

### 5.3 关键风险及缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|---|---|---|---|
| **R1**: 本地模型能力感知不准，动态补偿过于激进 | 中 | 中 | v1 只做告警不做自动 mode 切换；v2 在数据验证后上线自动补偿 |
| **R2**: 契约注册表替换 parser 时出现 A/B 行为不一致 | 中 | 高 | A/B 回归套件必须在合并前通过；模糊匹配 v1 仅 case-insensitive（最安全） |
| **R3**: 策略签名密钥管理成为运营负担 | 中 | 中 | v1 签名是可选的（无签名文件 fallback）；密钥管理文档化为外部流程 |
| **R4**: 方向 E「部分接受」触发 converge 语义歧义 | 低 | 高 | 方向 E v1 不做部分接受；v2 与 `converge.Signals` 的 item 级评估联合设计 |
| **R5**: 五个方向并行开发导致的集成冲突 | 中 | 中 | 每个方向有独立的包边界；共享接口（如 trace.Event）需提前达成接口契约 |

### 5.4 决策树

```
先做信任根（方向 A）？
├── 是 → 架构更稳固，企业合规性更高
│   └── 缺点：需要密钥管理流程，增加运营复杂度
├── 否 → 先做本地模型（方向 B）
│   └── 缺点：没有策略审计的本地部署是不完整的
└── 建议：做。信任是 ForgeOS 的核心价值主张

先做基础设施（方向 D Run ID）？
├── 是 → 所有后续方向的可追溯性基础
├── 否 → 能在没有 run_id 的情况下做其他方向
└── 建议：做。2 天的工作量，ROI 极高

部分接受（方向 E v2）是否值得做？
├── 是 → 降低采纳坡道，差异化竞争
│   └── 需要：converge.Signals 的 item 级评估重构
├── 否 → 全接受/全跳过已能覆盖 80% 场景
└── 建议：v1 不做，v2 与 converge 重构联合设计
```

---

## 6. 总结性观点

### 6.1 我同意审阅文档的核心判断

审阅文档指出的三个关键问题——**差异化引用错误、方向一的原子性冲突、方向二的接口变更量低估**——都是真实且有价值的发现。特别是：

> **「方向一的『部分接受』与 Phase 原子性冲突」** 是整个提案中最重要的架构洞察。`asset.Phase` 要么全成功要么全回滚——这是正确的设计。在 v1 版本移除「部分接受」是正确的决策。

> **「方向二的混合模式路由与 TierFor 当前签名冲突」** 是工作量低估的核心原因。`TierFor(agent, mode)` → `(string, string) → Tier` 需要变为 `TierFor(agent, mode, backend)` → `(string, string, Backend) → Tier`——但这不只是加一个参数，而是涉及 `resolveEnforce`、`GatesFor`、`waves.go` 的连锁变更。

### 6.2 我不同意审阅文档的两点

1. **方向四（策略审计追踪）是「独立盲区」**——审阅文档将其标记为 P1 独立方向，但我认为它不是独立的。策略审计的核心机制（签名 + 校验）本质上是**方向 A（策略信任根）的子集**。没有签名的审计日志与有签名的策略是同一个信任链的上下游。我建议将方向四吸收为方向 A 的「审计输出面」，而非独立方向。

2. **优先级排序的「方向二 P0、方向四 P1、方向三 P2」**——我同意方向二是 P0，但我认为**方向四（策略信任根）也应该是 P0**。没有它，方向二的意义大打折扣：可以在内网跑本地模型，但无法向审计员证明治理策略未被篡改。企业客户买的是「可审计的自治」，不是「盲目的自治」。

### 6.3 一个被所有文档遗漏的架构洞见

**ForgeOS 当前缺少的是「经济层」——这不是成本计量（已有），而是执行策略的经济合理性证明。**

当前 `runBudget` 和 `MaxIterations` 只回答「是否超限」，不回答「花的值不值」。如果一个 Phase 用了 5 次 loop-back 才过——这是模型能力不足还是任务定义不清？如果 `forge evolve` 跑了 20 次迭代才收敛——这是合理的迭代还是「LLM 在猜答案」？

**建议新增：`converge.Signals.EconomicWorth`**

```go
type EconomicWorth struct {
    TotalCostUSD       float64 // 该 run 的总成本
    LoopbackRate       float64 // 重跑次数 / phase 数
    IterationEfficiency float64 // 每次迭代收敛的 gate 数（gate 增量 / 迭代次数）
    WorthScore         string  // "good" / "acceptable" / "wasteful"
}
```

这不是一个高优先级（P2），但它是 ForgeOS 从「运行」到「优化」的关键飞跃——**不仅知道系统在运行，还知道系统运行得好不好**。

---

**总体结论**：ForgeOS 当前的架构处于极好的健康状态——罕见的零外部依赖、深刻的控制理论理解、成熟的治理框架。五个扩展方向的方向感正确，但差异化论证需要修复事实错误，优先级需要重新排列以**信任根 → 执行模型扩展 → 治理深化 → 运营成熟**为主线。建议在下个版本（v2.5）围绕这个主线重构实施顺序。
