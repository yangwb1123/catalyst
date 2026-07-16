# ForgeOS — 架构师视角:五方向高价值扩展的综合架构分析

> **角色**: 资深架构师  
> **日期**: 2026-07-12  
> **基础**: 基于 [2026-07-12-five-highvalue-architect-pm-directions.md](2026-07-12-five-highvalue-architect-pm-directions.md) 的五方向分析 + 审阅反馈意见  
> **状态**: 初稿

---

## 目录

1. [架构评估](#1-架构评估)
2. [扩展方向（深化版）](#2-扩展方向深化版)
3. [接口设计建议](#3-接口设计建议)
4. [技术选型](#4-技术选型)
5. [实施路线图](#5-实施路线图)

---

## 1. 架构评估

### 1.1 当前架构优势

ForgeOS v2 的架构经过 31 个 Sprint 的 dogfood 迭代，已形成高度内聚的 Go 运行时（18 包 · 零外部依赖 · 纯标准库）。以下几项架构选择构成了其核心竞争力的地基：

| 架构决策 | 优势 | 经验证程度 |
|---|---|---|
| **Go 为核心的零依赖策略** | 静态二进制分发、无供应链漏洞面、快速的 CI 构建 | Sprint 1-31 持续验证 |
| **中枢旋钮 mode×lifecycle** | 单一设置驱动 Router 档位 + Harness 严格度 + Workflow 深度，正交性高 | Sprint 7-15 增量验证，全维度已齐 |
| **Learning loop 三维真数据** | Quality + Latency + Cost 真实值回灌 scorecard → HistoryTiebreak | Sprint 24-26 真 claude 坐实 |
| **四维资源护栏** | Recursion + Budget + Timeout + Output-size cap 形成完备保护 | Sprint 20-22 系统构造 |
| **带外执法层（Harness）** | 宿主独立，不受 CLI 能力限制，公正性可通过 CI 验证 | v0 起载重墙模式 |
| **契约驱动的 agent 输出解析** | VERDICT/CONFIDENCE 机读 token → 收敛信号 → 定向 loop-back | Sprint 27-31 逐步补齐 |

### 1.2 架构局限性

通读 31 个 Sprint + 全部现有分析文档后，以下局限性和架构债务需要诚实面对：

#### 1.2.1 ADR-0002 的多语言栈严重不均衡

| 层 | 语言 | 状态 | 债务程度 |
|---|---|---|---|
| forge-core | Go | ✅ 落地（18 包，35k LOC） | — |
| **forge-ai** | **Python** | ❌ **零代码** | 🔴 **架构承诺未兑现** |
| forge-runtime | Rust | ❌ 推迟至 v3 | 🟡 有计划推迟 |
| forge-web | TypeScript | ❌ 推迟至 v3 | 🟡 有计划推迟 |

ADR-0002 声明了 polyglot 架构，但 `forge-ai` 既无代码也无明确的推迟措辞（Sprint 30 刚补了一行）。当前 forge-core 的「智能」全部通过手写规则实现——路由是固定 tier 表、风险检测是路径子串匹配、记忆是简单计数+时间戳、检索是 TF-IDF BM25-lite。**这不是"规则够用"的问题，而是架构契约的未履行。**

#### 1.2.2 模块间通信模式单一

当前 forge-core 内部全部通过 Go 函数调用（in-process）。外部进程通信仅通过 `exec.Command` spawn 子进程（Python shim、agent 命令）。**无 Unix socket、无 gRPC、无共享内存。** 这意味着：

- Python 调用每次支付 50-100ms 进程启动延迟（`yaml2json.py` 每次 `forge run/evolve` 被调数次）
- 无法保持长连接状态（如果未来 forge-ai 需要加载 embedding 模型，每次启动 ~2-5s 的模型初始化时间是不可接受的）
- 无法做热更新/热加载（进程级隔离导致升级需杀旧启新）

#### 1.2.3 执行溯源完全空白

整个系统运行 >30 sprint、>190 源文件，但没有任何机制可以回答以下问题：

> "这个 `handler.go` 是哪个 agent 在哪个 iteration 用哪个 model 写的？"

trace 事件不含 agent 输入/输出内容、checkpoint 无 checksum、phase 产物无签名、artifact 无 manifest。**对于自称"AI 软件工厂"的系统，这是产品级信任的缺失。**

#### 1.2.4 CLI 架构阻碍管线自动化

当前 `forge run` 和 `forge evolve` 是单 workflow 入口。管线编排（Discover → Design → Review → Build → Evolve）完全依赖手动操作。`next_stage: review` 声明在 workflow YAML 中但无人消费——**它是文档注释而非代码。**

#### 1.2.5 质量可观测性不足

Scorecard 记录 gate 二元通过/失败，但不记录 agent 产出质量。路由的 `HistoryTiebreak` 基于的 `quality_score` 本质上是二进制 0/1（gate 是否通过），无法区分"过得很勉强"和"轻松通过"。没有 golden task 定义，没有可重复的评测流程。

### 1.3 关键架构债务一览

| # | 债务 | 影响范围 | 分类 |
|---|---|---|---|
| D1 | `forge-ai` 零代码 | 智能层靠规则硬撑，无 ML/embedding/统计 | 架构承诺未履行 |
| D2 | 子进程通信模式 | 每次调用 50-100ms 延迟，无法保持状态 | 通信模式瓶颈 |
| D3 | 无输出溯源 | 合规审计、供应链安全、根因分析不可行 | 安全/信任缺口 |
| D4 | 手动管线串联 | "自治软件工厂"愿景的关键阻碍 | 产品能力缺口 |
| D5 | 质量评测框架缺位 | 路由智能度上限被 gate 二元数据锁死 | 可评测性缺口 |
| D6 | YAML shim 依赖 | `yaml2json.py` 每次 Python 启动加 50-100ms，且是唯一的外部 Python 依赖 | 临时架构债务 |
| D7 | phase 间无契约验证 | 级联失败可传播 3+ phase 才被发现 | 数据完整性缺口 |
| D8 | 进程锁跨平台不完整 | Linux 已实现，Windows `TODO(windows)` | 平台兼容性债务 |

---

## 2. 扩展方向（深化版）

以下五个方向基于原始分析的五方向，纳入审阅反馈中的 tradeoff 补充和项目当前状态重新深化。每个方向包括：**架构驱动**（为什么从架构层面需要）、**tradeoff 分析**（含审阅反馈中的具体量化补充）、**设计选项及权衡**。

---

### 方向一 · forge-ai Python 智能层

#### 架构驱动

ADR-0002 声明了 polyglot 架构的四年愿景，2026 年已过半，forge-ai 的 Python 智能层仍是架构契约中的空白。这不是「加一个可选功能」，而是**履行架构承诺**——Go 擅长编排但非智能计算的舒适区，手写规则已触及复杂度瓶颈。

#### 审阅补充：通信开销量化

原始分析推荐 `exec.Command("python3", ...)` 的子进程调用模式。审阅反馈指出关键 tradeoff：

> **每次 `exec.Command("python3", ...)` 调用有 ~50-100ms 进程启动延迟。**

这意味着：
- 当前 `yaml2json.py` 每次 `forge run` 调用 2-3 次 → 150-300ms 开销（可接受，临时 shim）
- 未来 forge-ai 的 embedding 模块（模型加载 ~2-5s）如果走子进程模式 → **每次调用 2-5s 不可接受**
- 路由预测模块如果是 per-phase 调用（5 phase × 每次 100ms = 500ms）→ 可接受但浪费

#### 设计选项

| 选项 | 延迟 | 复杂度 | 推荐场景 |
|---|---|---|---|
| **A. 子进程（exec.Command）** | 每次 50-100ms | ⭐ 低 | 轻量计算（TF-IDF 增强、简单评分） |
| **B. Unix socket 长连接 daemon** | 首次 2-5s + 后续 <1ms | ⭐⭐⭐ 中 | 重量级服务（embedding 模型、ML 预测） |
| **C. Unix socket + 懒启动 daemon** | 首次触发热加载，后续 <1ms | ⭐⭐⭐⭐ 中高 | 混合方案（推荐） |

**推荐方案：C——Unix socket 长连接 daemon + 按需懒启动**

```
forge-core (Go)                  forge-ai (Python daemon)
     │                                  │
     │─── exec.Command("forge-ai       │ (第一次调用时启动)
     │    daemon --socket /tmp/        │
     │    forge-ai.sock") ──────────→  │ 加载模型 ~2-5s
     │                                  │
     │─── JSON over Unix socket ─────→ │ <1ms 后续调用
     │←── JSON response ────────────│  │
     │                                  │
     │─── SIGTERM ──────────────────→  │ daemon 退出
```

**tradeoff 明细**：

| 维度 | 子进程方案 | Daemon 方案 | 选择依据 |
|---|---|---|---|
| 冷启动延迟 | 50-100ms（无需加载模型） | 2-5s（需加载 embedding 模型） | 重量级任务选 daemon |
| 后续调用延迟 | 50-100ms（每次重新启动） | <1ms（通过 socket） | 高频调用选 daemon |
| 进程管理复杂度 | 低（Go 原生管理） | 中（需 health check + 重启逻辑） | 可接受的复杂度 |
| 崩溃隔离 | 强（进程重置） | 中（daemon 崩溃需重连） | 加 watchdog 缓解 |
| 内存占用 | 低（用完即释放） | 中（模型常驻内存 ~500MB-2GB） | 按需启动，空闲时释放 |
| 向后兼容 | ✅ 零依赖 | ✅ 不可用时降级纯规则 | forge-core 零依赖不变 |

#### 推荐的带件优先级

```
Sprint 1:  forge-ai daemon 骨架 + Unix socket 协议定义
Sprint 2:  第一个模块（embedding/语义检索）替换手写 TF-IDF
Sprint 3:  路由预测模块 + 降级测试
Sprint 4+  anomaly 检测 + 成本预测（可选，按需）
```

---

### 方向二 · Agent 输出溯源与可验证性

#### 架构驱动

系统运行 31 个 Sprint 后，**没有任何机制证明输出的来源、完整性和未被篡改性。** 对于被称为"AI 软件工厂"的产品，这是信任地基的缺失。

#### 审阅补充：Checkpoint 签名性能 tradeoff

原始分析提出 `--verifiable` 标志启用 provenance 记录。审阅反馈指出关键 tradeoff：

> **若启用 `--verifiable`，checkpoint 写性能会从 O(1) 降为 O(n)——需计算所有 phase 产物的 SHA256。**

量化评估：

| 场景 | phase 产物数 | 平均文件大小 | SHA256 计算耗时 | 写放大 |
|---|---|---|---|---|
| 小型项目（starter） | 3-5 个 | 1-5 KB | <1ms | 可忽略 |
| 中型项目（url-shortener） | 5-15 个 | 5-50 KB | 1-5ms | 可接受 |
| 大型项目（ForgeOS 自己） | 15-50 个 | 50-500 KB | 5-50ms | 注意 |
| 超大产物（含图片/二进制） | 1-5 个 | 1-100 MB | 100ms-5s | ⚠️ 显著 |

**缓解策略：**

1. **增量哈希**：只对 `emits:` 声明的文件做哈希，而非全目录扫描（大多数 phase emit 3-5 个文件）
2. **异步写入**：checkpoint 主路径写入 metadata，文件哈希在后台 goroutine 完成后追加
3. **跳过二进制**：默认对二进制文件（图片、压缩包）只记录大小 + 最后修改时间，而非全文件 SHA256
4. **诚实降级**：超大产物 → trace 记录 `provenance_skip:true` + 原因，验证时报告 `PARTIAL`

#### 设计选项

| 选项 | 信任保证 | 复杂度 | 性能影响 |
|---|---|---|---|
| **A. 纯哈希链（SHA256 清单）** | 防篡改检测，非防伪造 | ⭐ 低 | O(n) 可优化 |
| **B. 哈希链 + Ed25519 签名** | 防篡改 + 防伪造（谁产生） | ⭐⭐⭐ 中 | O(n) + 签名 1-2ms |
| **C. 哈希链 + TUF/in-toto 元数据** | 完整软件供应链元数据 | ⭐⭐⭐⭐⭐ 高 | 过重，不适合 |

**推荐：方案 A 做 v1，方案 B 做 v2。** 理由：
- Ed25519 签名引入密钥管理：谁持有签名密钥？agent 没有身份、forge-core 有但密钥存储在哪里？
- v1 的哈希链已经能回答"文件是否被篡改"（检测型信任）
- v2 的签名才能回答"谁产生并签署了文件"（归属型信任），但这需要 PKI 骨架
- 哈希链的诚实定位：**它不是防攻击，而是防静默漂移**——工程师不小心编辑了文件、git 切换分支导致内容变化，这些是溯源系统的主要价值场景

---

### 方向三 · 跨 Workflow 管线编排引擎

#### 架构驱动

当前 `forge run discover && forge run design && forge run review && forge run build && forge run evolve` 的五步手动操作与 ForgeOS "自治软件工厂"的愿景之间的差距，是**目前最大的产品级缺口**。架构上，单次 `forge run` 的运行时已高度完善（orchestrator + converge + loop-back + checkpoint/resume），但跨 run 编排是空白。

#### 核心架构挑战

| 挑战 | 描述 | 难度 |
|---|---|---|
| **状态传递** | Pipeline stage 间传递产出路径、收敛信号、上下文 | ⭐⭐ |
| **条件分支** | `on_approve` vs `on_redesign` 等条件跳转 | ⭐⭐⭐ |
| **并行 stage** | 同时跑 review 和 security scan | ⭐⭐⭐⭐ |
| **恢复语义** | Pipeline 中途崩溃后从当前 stage 恢复 | ⭐⭐⭐ |
| **git commit 一致性** | Pipeline 跨越多个 stage 时代码状态冻结 | ⭐⭐ |

#### 设计选项

| 选项 | 描述 | 复杂度 | 推荐 |
|---|---|---|---|
| **A. 声明式 YAML pipeline 定义** | 新 `pipelines.yml` + `forge pipeline run` | ⭐⭐ | ✅ **v1 推荐** |
| **B. Go DSL（嵌入式）** | Go 代码定义 pipeline，类型安全 | ⭐⭐⭐⭐ | v2 候选 |
| **C. 复用 workflow YAML + 注释** | `next_stage` 从注释升级为真信号 | ⭐ | ❌ 不够表达条件分支 |

**推荐方案 A 的详细设计：**

```yaml
# .agent/pipelines/full-build.yml (v1 最小可行)
name: full-build
version: 1                        # schema 版本，方便演进
stages:
  - id: discover
    workflow: discover
    mode: engineering
    on_success: design             # 线性串联
    timeout: 30m                   # stage 级别超时

  - id: design
    workflow: design
    mode: engineering
    require: human_approval        # 自动暂停等待批准
    on_success: review
    on_redesign: discover          # 条件分支 ↓

  - id: review
    workflow: review
    mode: engineering
    on_approve: build
    on_reject: post-mortem         # 跳转到修复管线

  - id: build
    workflow: build
    mode: engineering
    require: convergence
    on_success: evolve
    parallel:                      # 并行 stage
      - workflow: security-scan
        mode: engineering

  - id: post-mortem               # 修复管线
    workflow: post-mortem
    mode: engineering
    on_success: discover           # 修复完重新开始
```

**v1 不做**：
- ❌ 并行 stage（需解决状态冲突）
- ❌ DAG（有向无环图）执行
- ❌ 跨仓库 pipeline
- ❌ 动态 stage 生成

#### 与现有 checkpoint 系统的交互

Pipeline 需要复用但不破坏现有 checkpoint/resume 机制：

```
.forge/
  pipeline-current.json           # 当前 pipeline 状态（stage id + seq）
  pipeline-discover/              # stage 级独立目录
    checkpoint.json               # 复用现有 checkpoint 路径
    trace.jsonl
  pipeline-design/
    checkpoint.json
    trace.jsonl
```

关键设计：**pipeline 在不同 stage 的不同 checkpoint 文件上操作**，不修改现有 checkpoint 结构的任何字段。新增 `internal/pipeline` 包处理编排逻辑，`cmd/forge/pipeline.go` 处理 CLI。

---

### 方向四 · 阶段间工件契约系统

#### 架构驱动

Sprint 27-31 已经证明：**契约驱动是 ForgeOS 的正确模式。** agent 卡中的机读 token（VERDICT/CONFIDENCE）驱动收敛信号、loop-back、熔断决策。但当前这个模式只覆盖了 agent 输出中的**决策 token**，没有覆盖 **artifact 本身的结构和完整性**。

#### 审阅补充：与 V49 的关系

原始分析已经做了差异化声明（V49 = 单阶段内文档结构验证，方向四 = 跨阶段接口契约）。审阅反馈建议进一步澄清：

> **"V49 是契约的下层基础"——先有 phase 内结构验证，才能做 phase 间契约验证。这是建造顺序依赖，非纯竞争关系。**

架构序列依赖：

```
V49 (单阶段文档结构完整性)
    └── section headers 存在、格式正确、required fields 非空
         ↓ 依赖（先有）
方向四 (跨阶段接口契约)
    ├── Phase A 的输出满足 Phase B 的输入预期
    ├── 文件存在性、最小内容、所需字段
    └── Phase B 消费前验证，失败时 WARN/BLOCK
```

换句话说：**V49 是方向四的必要前提，但不是充分条件。** 没有 V49，方向四只能验证文件存在性；有了 V49，方向四才能验证"文件的 section X 存在且非空"。

#### 契约定义的演进路径

```yaml
# v1: 存在性验证（当前可直接实现）
input_contract:
  min_files: 1
  required_files:
    - path: docs/discovery/prd.md
    - path: docs/discovery/market-research.md

# v2: 结构验证（依赖 V49 机制）
output_contract:
  min_files: 1
  required_files:
    - path: docs/design/architecture.md
      must_contain_sections:     # 借用 V49 的验证器
        - "## System Architecture"
        - "## Component List"

# v3: 语义验证（依赖 forge-ai embedding）
input_contract:
  semantic_checks:
    - description: PRD must define success metrics
      type: embedding_similarity
      reference: "The document shall contain measurable success metrics"
      min_similarity: 0.7
```

**架构决策**：契约验证器采用 `adapter` 插件模式（镜像现有 lint/coverage 适配器），这样：
- v1 的 `min_files`/`required_files` 由内置 Go 验证器直接处理（零额外依赖）
- v2 的 `must_contain_sections` 由 V49 验证器提供（两系统独立但协议兼容）
- v3 的 `embedding_similarity` 由 forge-ai 提供（延迟到方向一就绪）
- 不可用的验证器 → 诚实 N/A，不假装验证

---

### 方向五 · Agent 产出质量评测框架

#### 架构驱动

路由引擎的 `HistoryTiebreak` 目前使用基于 gate 二元结果的二进制 0/1 `quality_score`。这意味着：
- 无法区分"代码通过了 gate 但质量很差"和"代码质量优秀"
- 无法在不同模型之间做质量比较（Sonnet vs Opus vs future models）
- 无法评估 prompt template 变更对产出质量的影响
- 没有回归测试能力（prompt 更新后质量是上升还是下降？）

#### 架构设计与现有系统的关系

```
┌─────────────────────────────────────────────────────────────┐
│                      eval 框架（新建）                        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐    │
│  │ golden   │  │ golden   │  │ golden   │  │ custom   │    │
│  │ task A   │  │ task B   │  │ task C   │  │ tasks    │    │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘    │
│       │              │              │              │         │
│       ▼              ▼              ▼              ▼         │
│  ┌──────────────────────────────────────────────────┐       │
│  │               Eval Runner                         │       │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐        │       │
│  │  │ structural│  │complexity│  │coverage  │        │       │
│  │  │ checker   │  │ checker  │  │ delta    │        │       │
│  │  └──────────┘  └──────────┘  └──────────┘        │       │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐        │       │
│  │  │ document │  │ semantic │  │ custom   │        │       │
│  │  │ quality  │  │ (forge-ai)│  │ plugins  │        │       │
│  │  └──────────┘  └──────────┘  └──────────┘        │       │
│  └──────────────────────────────────────────────────┘       │
│       │                                                      │
│       ▼                                                      │
│  ┌──────────────────────────────────────────────────┐       │
│  │        多维质量分数 → scorecard                     │       │
│  │  quality_score: [completeness:0.9,                │       │
│  │                  maintainability:0.7,             │       │
│  │                  test_coverage:0.8, ...]          │       │
│  └──────────────────────────────────────────────────┘       │
│       │                                                      │
│       ▼                                                      │
│  ┌──────────────────────────────────────────────────┐       │
│  │  HistoryTiebreak 从 binary 0/1 升级为多维路由      │       │
│  └──────────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────────┘
```

#### 关键架构决策：评测器适配器模式

每个评测器是独立可执行体（shell out 适配器模式，同 lint/coverage/SCA），而非 Go 包：

```yaml
# eval/checkers/structural.yml
name: structural
command: python3 harness/eval/checkers/structural.py {task_dir} {output_dir}
timeout: 30s
output_format: json  # 输出 {score: 0.85, details: {...}}
```

这个选择的原因：
- **隔离性**：评测器崩溃不影响主 pipeline（非载重路径）
- **语言无关**：评测器可以是 Go/Python/Node——每个 checker 选最适合的语言
- **可扩展**：用户自定义评测器不需重新编译 forge-core
- **诚实降级**：评测器不可用 → score 字段 omit（不阻塞 pipeline）

#### tradeoff：评测耗时 vs 频率

| 场景 | 评测耗时 | 频次 | 策略 |
|---|---|---|---|
| golden task 离线评测 | 5-30 分钟 | 每次 prompt/model 变更 | 独立 `forge eval run` 命令 |
| 管线内轻量评测 | 1-10 秒 | 每次 gate phase | 结构检查 + 复杂度不阻塞 |
| 语义质量评分 | 10-60 秒 | 每次 build 完成 | 异步，不阻塞收敛判定 |

**设计原则**：`forge eval` 是**分析命令**（同 `forge doctor`），**非** `forge run/evolve` 的阻塞步骤。质量分数异步回灌 scorecard，当前 run 不受影响，下一个 run 的路由使用更新后的分数。

---

## 3. 接口设计建议

### 3.1 关键模块接口设计原则

基于当前 forge-core 的架构风格（零依赖、纯标准库、适配套接合），以下原则适用于五个方向的接口设计：

| 原则 | 说明 | 反例 |
|---|---|---|
| **P1. 新增字段必须 optional + omitempty** | 旧 trace/checkpoint/memory 文件在新代码读取时不崩溃 | ❌ 新 `RunID` 字段不加 `omitempty` |
| **P2. 降级链优先于错误传播** | 外部组件不可用时静默降级，非崩溃 | ❌ forge-ai 不可用时报错退出 |
| **P3. 适配器模式而非 Go interface 注入** | shell out 到独立进程，而非 Go 包 import | ❌ Go interface 依赖注入容器 |
| **P4. 新增包零循环依赖** | 新包 `import` 图是 DAG 的叶子 | ❌ `internal/pipeline` import `internal/orchestrator` |
| **P5. CLI 纯胶水，逻辑进 internal** | `cmd/forge` 只做 flag 解析和组合 | ❌ 把 pipeline 编排写在 `cmd/forge/pipeline.go` |
| **P6. 配置通过显式层级覆盖** | CLI flag > `.forge/policy.yml` > `project.yml` > 硬编码默认 | ❌ 配置散落在三处无优先级定义 |

### 3.2 新增抽象层

五方向引入以下新抽象层，按语义边界组织：

```
forge-core/
  internal/
    pipeline/        ← 新增（方向三：管线编排）
      pipeline.go    — Pipeline 结构体定义
      run.go         — 执行引擎
      state.go       — checkpoint/resume
      dsl.go         — YAML schema 解析

    contract/        ← 新增（方向四：契约验证）
      contract.go    — Contract 定义 + 注册表
      verify.go      — 存在性/结构/语义验证器
      adapter.go     — 适配器注册

    eval/            ← 新增（方向五：质量评测）
      task.go        — Golden task 定义
      runner.go      — 评测执行器
      checker.go     — 评测器注册
      score.go       — 多维分数聚合

    provenance/      ← 新增（方向二：溯源）
      manifest.go    — ArtifactManifest 生成
      chain.go       — 哈希链
      verify.go      — 验证命令
```

**不新增的抽象层（有意识的不做）**：

- ❌ 不做 `internal/ai` 或 `internal/python` 包装——forge-ai 是独立 Python 包，不在 Go 内部做抽象
- ❌ 不做 `internal/plugin` 或 `internal/hotplug`——适配器模式 shell out 已经提供了足够的扩展性
- ❌ 不做 `internal/pipeline/dag.go`——v1 只做线性 + 简单条件分支，不做通用 DAG 执行引擎
- ❌ 不做 `internal/eval/ai_scorer.go`——语义评分延迟到 forge-ai 就绪

### 3.3 向后兼容策略

| 变更 | 影响 | 兼容策略 |
|---|---|---|
| trace.Event 加 RunID | `trace.jsonl` 读取 | `omitempty`，旧行解析 `run_id` 缺 = 空字符串 |
| checkpoint 加 Checksum | `--resume` 路径 | 旧 checkpoint 无 checksum → 跳过验证 |
| workflow YAML 加 `input_contract` | 旧 workflow 无此字段 | 空 contract = 不验证 |
| 新增 `pipelines.yml` | 不存在该文件 | `forge pipeline run` 报"无 pipeline 定义"，不影响 `forge run` |
| mode 加 `review/eval` 深度 | 旧 `modes.yml` 缺字段 | 缺省 = 不启用 review/eval |
| scorecard 加 quality_score 多维 | 旧 scorecard 无此字段 | omitempty，二维回退为 gate 二进制 |

---

## 4. 技术选型

### 4.1 各方向技术选型矩阵

| 方向 | 语言 | 运行时 | 外部依赖 | 进程模型 |
|---|---|---|---|---|
| **forge-ai** | Python 3.10+ | 独立 daemon | `pip` 生态（numpy, scikit-learn, sentence-transformers） | Unix socket daemon |
| **溯源** | Go（纯标准库） | 内嵌 forge-core | 零外部依赖 | in-process |
| **管线编排** | Go（纯标准库） | 内嵌 forge-core | 零外部依赖 | in-process |
| **契约系统** | Go（纯标准库） | 内嵌 forge-core | 零外部依赖 | in-process（+ adapter shell out） |
| **质量评测** | Go + 适配器 | 内嵌 + 独立 | 零外部依赖（适配器可选） | in-process + shell out |

### 4.2 forge-ai 的 Python 包管理策略

| 选项 | 优点 | 缺点 | 推荐 |
|---|---|---|---|
| **A. `pip install forge-ai`** | 标准 Python 包管理 | 用户需要手动安装，版本管理独立 | ❌ 非自包含 |
| **B. `pip install -r requirements.txt`** | 声明式依赖 | 同 A | ❌ |
| **C. `uv tool install forge-ai`** | 现代 Python PM，隔离环境 | 依赖 `uv` | 🟡 候选 |
| **D. forge-core 内嵌 Python 包（embed）** | 自包含 | Go 标准库无 embed Python，需 cgo | ❌ |
| **E. forge-core spawn + 自动 `pip install`** | 极简用户接口 | 但需用户有 Python 环境，联网 | ✅ **推荐** |

**推荐方案**：forge-ai daemon 启动时检测依赖完整性，缺失时打印安装命令并退出（不静默安装）。用户显式运行 `forge-ai setup` 完成安装。forge-core 在 daemon 不可用时完全降级。

**依赖锁定的 tradeoff**：

```
刚性锁（pinned requirements.txt）
  └── ✅ 可复现的 Python 环境
  └── ❌ 用户项目要求 Python 3.12 但 forge-ai 锁了 3.10
  └── ❌ scikit-learn 1.6 安全修复但 forge-ai 锁了 1.5

柔性锁（>= 版本范围）
  └── ✅ 兼容性更好
  └── ❌ 更不可复现
  └── ❌ "works on my machine" 风险
```

**建议**：发布时锁文件（`requirements-lock.txt`）供 CI/CD 使用，开源版声明版本范围（`>=3.10,<4`）供用户灵活安装。

### 4.3 哈希 vs 签名（方向二）

| 维度 | SHA256 哈希链 | Ed25519 签名 |
|---|---|---|
| 证明能力 | 文件未被篡改 | 文件由持有私钥者签署 |
| 密钥管理 | 不需要 | 需要：谁持有私钥？存储在哪？ |
| 性能 | 快（~1ms 每 MB） | 慢（~1-2ms 签名 + 哈希时间） |
| 供应链安全 | 检测篡改 | 检测篡改 + 证明来源 |
| 复杂度 | ⭐ 低 | ⭐⭐⭐ 中（需引入密钥生命周期） |

**结论**：v1 用纯哈希链。v2 再考虑签名——而且签名应该用 forge-core 的实例密钥（非用户密钥），在 `forge init` 时生成。

### 4.4 YAML 解析器的最终替换策略

当前 `yaml2json.py` 是 Python shim 的唯一调用者。Sprint 27 已重写 `internal/yaml2json`（Go 原生实现），但仍在并行使用。替换路线：

```
阶段 1（当前）: Python shim → Go 原生（dual, 两路同跑，对比输出）
阶段 2（Sprint N+2）: Python shim → Go 原生（单独，Python shim 作为 fallback）
阶段 3（Sprint N+4）: 完全移除 Python shim（forge-core 零外部运行时依赖达成）
```

这一替换与 forge-ai 方向正交——即使 forge-ai 引入 Python daemon，`yaml2json.py` 仍应被 Go 原生替换，因为：
- 它是每次 `forge run` 的阻塞调用（跑不过就不能开始）
- 它不需要 Python 生态的任何智能能力（纯 YAML→JSON 机械转换）
- forge-core 零依赖的目标要求消除这个外部依赖

---

## 5. 实施路线图

### 5.1 优先级重排序

基于以下因素重新评估原始五方向的优先级：

1. **方向二（溯源）**：审阅确认其差异化声明唯一"零接近匹配"（~110 份分析未涉及），且是信任地基——**维持 P1**
2. **方向一（forge-ai）**：审阅补充了通信开销 tradeoff，但 ADR 履约仍是架构债务——**维持 P1**，但建议先做 daemon 骨架
3. **方向三（管线编排）**：产品落差最大，"自治软件工厂"愿景的关键阻碍——**维持 P1**
4. **方向四（契约系统）**：依赖方向二的哈希基础，且 V49 是下层前提——**从 P2 提升至 P1**（审阅反馈确认其与 V49 非竞争关系）
5. **方向五（质量评测）**：依赖方向一的 forge-ai 提供语义评分——**维持 P2**

**修正后的优先级**：

| 新排序 | 方向 | 优先级 | 理由 |
|---|---|---|---|
| **1** | 方向二 · 溯源 | **P1** | 唯一"零覆盖"方向；信任地基；零外部依赖 |
| **2** | 方向一 · forge-ai | **P1** | ADR 履约 + 架构补全；先做 daemon 骨架即可解耦 |
| **3** | 方向四 · 契约 | **P1** | Sprint 27-31 已证明契约模式有效，扩展到 artifact 层面 |
| **4** | 方向三 · 管线 | **P1** | 产品落差最大，但工程复杂度最高，推后到 daemon 骨架之后 |
| **5** | 方向五 · 评测 | **P2** | 依赖 forge-ai embedding，可推迟 |

### 5.2 阶段划分

```
Phase 1: 信任地基（Sprint N ~ N+2）
┌──────────────────────────────────────────────────────┐
│ 方向二（溯源）:                                       │
│   - ArtifactManifest 生成（每 phase 完成后）           │
│   - 哈希链（checkpoint 加 prev_checksum）              │
│   - forge verify provenance 命令                       │
│   - trace ↔ artifact 关联                              │
│                                                       │
│ 方向四（契约 v1）:                                     │
│   - Contract struct + 注册表                           │
│   - input_contract/output_contract YAML schema         │
│   - 存在性验证器（文件存在 + min_files）                │
│   - 契约声明的 check.py 治理检查                        │
└──────────────────────────────────────────────────────┘

Phase 2: 智能骨架（Sprint N+2 ~ N+4）
┌──────────────────────────────────────────────────────┐
│ 方向一（forge-ai daemon 骨架）:                        │
│   - Unix socket 协议定义                               │
│   - forge-ai daemon 启动/停止/health                   │
│   - 第一个模块：语义检索（替换 TF-IDF）                  │
│   - 降级测试全绿                                       │
│                                                       │
│ 方向三（管线编排 v1）:                                  │
│   - pipelines.yml schema + 解析                        │
│   - 线性 stage 执行（`forge pipeline run`）             │
│   - stage 级 checkpoint/resume                        │
│   - 条件分支（on_success/on_failure）                   │
└──────────────────────────────────────────────────────┘

Phase 3: 质量闭环（Sprint N+4 ~ N+6）
┌──────────────────────────────────────────────────────┐
│ 方向五（评测 v1）:                                     │
│   - Golden task 定义                                   │
│   - `forge eval run`/`forge eval compare`              │
│   - 结构 + 复杂度评测器                                │
│   - 多维 quality_score 回灌 scorecard                  │
│   - HistoryTiebreak 从 binary → 多维                   │
│                                                       │
│ 方向四（契约 v2）:                                     │
│   - 结构验证（must_contain_sections）                   │
│   - BLOCK/WARN 失败模式                                │
│   - 跨 phase 契约的自动化回归测试                       │
└──────────────────────────────────────────────────────┘

Phase 4: 深化（Sprint N+6+）
┌──────────────────────────────────────────────────────┐
│ forge-ai 深化:                                        │
│   - 路由预测模块（基于历史数据动态调 tier）              │
│   - Trace 异常检测                                     │
│   - 成本/时间预估                                      │
│                                                       │
│ 管线编排 v2:                                          │
│   - 并行 stage 执行                                    │
│   - Git commit 冻结                                    │
│   - Pipeline 级统计/报告                               │
│                                                       │
│ 契约 v3 + 评测 v2:                                    │
│   - 语义契约检查（forge-ai embedding）                  │
│   - 自动 golden task 生成                              │
│   - 跨项目质量基线                                     │
│                                                       │
│ yaml2json 最终替换:                                    │
│   - 移除 Python shim                                   │
│   - forge-core 零外部运行时依赖达成                     │
└──────────────────────────────────────────────────────┘
```

### 5.3 风险点与缓解策略

| 风险 | 方向 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| **R1: forge-ai daemon 崩溃导致 phantom 进程** | 方向一 | 中 | 高 | watchdog timer + daemon 退出时自动清理 socket 文件 + forge-core 检测 socket 断开后自动重启 |
| **R2: 哈希链的 SHA256 计算被误认为安全证明** | 方向二 | 高 | 低 | 文档明确标注"哈希链检测篡改但非防伪造签名" + 输出 `forge verify provenance` 时打印诚实 disclaimer |
| **R3: Pipeline 恢复时 stage 间状态不一致** | 方向三 | 中 | 高 | 每个 stage 运行前 snapshot checkpoint + 恢复时校验前驱 stage 产物一致性；不一致时 REPORT 而非自动修复 |
| **R4: 契约验证过于严格导致假阳性** | 方向四 | 中 | 中 | 默认 `WARN` 而非 `BLOCK`；`engineering` mode 以下才 `BLOCK`；提供 `contract: off` 全局关闭 |
| **R5: Golden task 太简单或太难，偏离真实项目** | 方向五 | 高 | 中 | Golden task 定位为"参考性指标"而非"生产 gate"；项目可自定义评测器；多 task 取分位数 |
| **R6: yaml2json 替换过程中出现 Python shim 和 Go 实现不一致** | 跨方向 | 中 | 高 | 双路同跑期（阶段 1）每轮 CI 对比两路输出，diff 非空则 FAIL |

### 5.4 硬闸门检查清单（每方向完成的必要条件）

以下清单是每个方向完成后必须通过的硬闸门（forge accept 聚合）：

| 检查项 | 工具 | 方向一 | 方向二 | 方向三 | 方向四 | 方向五 |
|---|---|---|---|---|---|---|
| Go 编译通过 | `go build ./...` | ✅ | ✅ | ✅ | ✅ | ✅ |
| Go 测试全绿 | `go test -race ./...` | ✅ | ✅ | ✅ | ✅ | ✅ |
| 无循环依赖 | `arch-check.mjs` | ✅ | ✅ | ✅ | ✅ | ✅ |
| 函数长度 ≤ 50 | `arch-check.mjs` | ✅ | ✅ | ✅ | ✅ | ✅ |
| 治理完整性 | `check.py` | ✅ | ✅ | ✅ | ✅ | ✅ |
| 无硬编码 secret | `secret-scan.mjs` | ✅ | ✅ | ✅ | ✅ | ✅ |
| 向后兼容（旧格式读） | 特定回归测试 | ✅ | ✅ | ✅ | ✅ | ✅ |
| 诚实降级（组件不可用） | 模拟故障测试 | ✅ | N/A | N/A | ✅ | ✅ |
| 文档更新 | `ROADMAP.md` + `.agent/` | ✅ | ✅ | ✅ | ✅ | ✅ |

### 5.5 不做清单（明确排除）

以下是有意排除在本次路线图之外的方向，诚实标注原因：

| 排除项 | 原因 | 条件（如有） |
|---|---|---|
| Web UI / Dashboard | 偏离 CLI 声明式核心；v3 方向 | v3 再评估 |
| Firecracker 沙箱 | 需 KVM 特权 + 内核模块；forge-core 当前容器内不可用 | 等到有 KVM 环境 |
| 跨厂商模型池（LiteLLM） | 需多厂商 API keys + 计费管理；已标注 BLOCKED-EXTERNAL | 用户提供 keys 后可启用 |
| Agent 输出自动部署 | Agent 写代码 → 自动 git push + deploy 是生产交付问题，非管线问题 | v3+ |
| JWT/signed run_id | 签名引入密钥管理，当前 UUIDv7 够用 | 多机分布式场景时再引入 |
| 跨进程 trace 合并 | `forge doctor` 读多 run trace 需手动指定 | 未来如需，先做单 run 全量 |
| NLP-based 契约解析 | 方向四 v1 的模糊匹配限于 token-level（大小写/空格容错），不做语义理解 | forge-ai embedding 就绪后考虑 v3 |
| trace TTL 自动删除 | 方向五 v1 retention 只轮转/压缩不自动清理 | 用户显式配置 `max_age_days` 时开启 |

---

## 附录 A：审阅反馈逐项响应

| 审阅建议 | 接收状态 | 本文处理 |
|---|---|---|
| 方向一：补充 `exec.Command("python3", ...)` 通信开销 ~50-100ms 启动延迟分析 | ✅ 已纳入 | §2 方向一设计了 Unix socket daemon 方案 + 量化表格 |
| 方向一：建议 keep-alive daemon 或 Unix socket 长连接方案 | ✅ 已纳入 | 推荐方案 C（Unix socket 懒启动 daemon） |
| 方向二：`checkpoint.go` 有 `_format string` 但无 checksum；`--verifiable` 使写性能从 O(1) 降 O(n) | ✅ 已纳入 | §2 方向二补充了性能量化表 + 增量哈希/异步写入/跳过二进制三种缓解策略 |
| 方向四：与 V49 的关系——V49 是契约的下层基础 | ✅ 已纳入 | §2 方向四补充了架构序列依赖图，明确"V49 是必要前提非充分条件" |
| 保存为文件 | ✅ 已执行 | 本文件即为保存产物 |

---

## 附录 B：与已有 110+ 分析文档的关系

| 本文方向 | 最接近的已有分析 | 差异化 |
|---|---|---|
| forge-ai Python 智能层 | `eighth-wave-adr-decay.md` ADR 审计表标注"❌ 未开始" | 首次作为系统性架构方向展开 |
| Agent 输出溯源与可验证性 | **零接近匹配** | 全 110+ 份分析零触及 |
| 跨 Workflow 管线编排引擎 | `expansion-horizon-three.md` 方向一概念草图 | 本文首次提供完整 DSL/状态传递/条件分支/恢复设计 |
| 阶段间工件契约系统 | `five-novel-extension-frontiers-v49.md` 方向四（文档结构验证） | V49 = 单阶段内；本文 = 跨阶段接口契约 |
| Agent 产出质量评测框架 | `expansion-production-readiness.md` Prompt QA | Prompt QA = prompt 渲染正确性；本文 = agent 产出质量系统化评测 |

---

*本文基于 2026-07-12 工作树完整代码扫描 + 审阅反馈 + 本人架构判断。所有方向均已在建议文件名的文件中单独展开。*
