现在我有了完整的上下文——原始分析文档和验证报告。让我以架构师视角进行深入分析。

---

# 架构师分析：ForgeOS 产品运营面系统性缺口

> **分析范围**: 基于 `2026-07-11-five-product-operations-systemic-gaps.md` 原始分析及独立验证报告
> **角色**: 资深架构师
> **核心观察**: ForgeOS 的 AI 控制面（编排/治理/安全/遥测）已经极其健壮，但**产品运营面**（版本/诊断/观测/恢复/溯源）尚处于空白状态。这是从"强大的个人工具"进化为"团队可依赖的平台"所必须跨越的鸿沟。

---

## 一、当前架构评估

### 1.1 架构优势：值得保护的资产

ForgeOS 的架构在以下几个维度上做得极其出色：

| 维度 | 现状 | 评价 |
|------|------|------|
| **分层架构** | forge-core (Go 运行时) ↛ harness (Node/Python 执法层) ↛ agents (AI 控制面) | 清晰的分工，执法层与执行层分离是架构上最正确的决策之一 |
| **数据平面独立** | checkpoint（进度持久化）、trace（审计事件流）、memory（知识持久化）三者独立 | 好设计。三者生命周期不同（checkpoint 高频更新，trace 追加写，memory 语义写），独立存储避免耦合 |
| **文件级持久化** | 全部数据以 JSON(L) 文件形式存储在 `.forge/` 目录 | 优点：透明、可手动检查、可用标准工具处理。这是 Unix 哲学的正确选择 |
| **带外 Gate 系统** | 治理闸门独立于执行宿主运行 | 防止了执行环境对治理的污染，是 ForgeOS 最独特的架构优势 |

### 1.2 关键局限性：运营视角的结构性缺口

```
                    ┌─────────────────────────────────┐
                    │      AI 控制面（已有深度覆盖）      │
                    │  编排 · 安全 · 预算 · 护栏 · 超时   │
                    └─────────────────────────────────┘
                                    ↕
                    ┌─────────────────────────────────┐
                    │      产品运营面（本文焦点 — 空白）    │
                    │  版本 · 诊断 · 观测 · 恢复 · 溯源   │
                    └─────────────────────────────────┘
                                    ↕
                    ┌─────────────────────────────────┐
                    │       基础设施层（Go 标准库）        │
                    │  无外部依赖 · 单二进制分发           │
                    └─────────────────────────────────┘
```

最根本的架构局限是：**运营面的五个方向各自独立来看是增量功能，合起来看是一个缺失的架构层**。具体来说：

1. **数据平面间缺乏跨切面身份** — checkpoint、trace、memory 不共享任何关联字段，导致无法回答"这次 run 的所有数据是什么"。这不是增量缺失，是**基础契约缺失**。

2. **运行时健康表面为零** — forge-core 完全是"执行并退出"的批处理模型，没有守护进程模式、没有健康端点、没有实时状态暴露。设计成 24h 无人值守的系统，但自身无法被监控。

3. **错误处理是面向引擎的，不是面向用户的** — Go 错误链逐层包装后直接打印给用户。没有错误分类、没有严重级别、没有修复建议。这决定了用户每次遇到错误都是"阅读 Go 错误链 → 读源码 → 猜测修复"的循环。

4. **系统自身无版本管理** — `forgeVersion = "dev"` 是一个架构信号：系统没有把自己当作需要管理生命周期的一等实体。版本号不写入任何持久化数据，意味着数据格式升级没有安全路径。

5. **故障模式只有"中止"没有"降级"** — checkpoing 损坏 → 退出、memory 坏行 → 全文件废弃、trace 截断 → scorecard 不可用。整个系统在面对自身故障时只有一种响应：报错退出。

### 1.3 架构债务度量

| 债务类型 | 严重程度 | 说明 | 偿还成本（随时间增长） |
|----------|---------|------|----------------------|
| 缺失 RunID | ⭐⭐⭐⭐⭐ | 三个数据格式都需要 schema 变更 | 线性增长——更多历史数据意味着更复杂的迁移 |
| 无版本管理 | ⭐⭐⭐⭐⭐ | 每个新功能都可能产生不兼容的数据格式 | 指数增长——不兼容变化越多，未来迁移越复杂 |
| 无结构错误分类 | ⭐⭐⭐⭐ | 所有错误处理代码需要重构 | 线性增长 |
| 单目录无命名空间 | ⭐⭐⭐ | `.forge/` 没有子目录结构 | 线性增长 |
| 无健康端点 | ⭐⭐⭐ | 需要新的运行时模型 | 稳定——不会自动恶化 |

---

## 二、架构扩展方向（5 个）

### 方向 A：RunID 作为架构基元（合并方向③ + 方向⑤）

> **为什么需要**: 这是五个方向中**最具杠杆的单一架构决策**。RunID 一旦成为系统级基元，方向③（运营可观测性）和方向⑤（跨运行身份）的基础需求同时满足。

**业务价值**:
- 每次 `forge run`/`forge evolve` 产生的所有数据可关联
- 多人/多系统共用 .forge/ 目录时数据可甄别
- 为未来的 forge trace list/forge run list 提供索引键

**核心挑战**:
- **时序正确性**：RunID 必须在进程启动时尽早生成（在解析 CLI 参数之前），不能等到 run 真正开始
- **存储兼容性**：现有 checkpoint/trace/memory 文件没有 RunID 字段，读取旧文件时需要优雅处理缺失字段
- **跨进程一致性**：当 `forge evolve` 被 `forge resume` 恢复时，新 run 是否生成新 RunID？正确的语义是：resume 生成**新 RunID**，但在 checkpoint 中保留 `parent_run_id` 指向原 run

**预期架构变更**:

```
新增：core/id/runid.go           — UUIDv7 生成器
修改：persist/checkpoint.go      — 增加 run_id, parent_run_id 字段
修改：trace/trace.go             — 每个 Event 增加 run_id 字段
修改：memory/memory.go           — 每个 Entry 增加 run_id 字段
修改：orchestrator/command_executor.go — 注入 FORGE_RUN_ID 环境变量
新增：cmd/forge/run_list.go      — forge run list 子命令
```

**方案权衡**：

| 选项 | RunID 生成时机 | 优点 | 缺点 |
|------|---------------|------|------|
| A1: 解析 CLI 后立即生成 | `main()` 中参数解析后 | 可以从 argv 生成确定性 ID | 无法用于进程启动阶段的 trace |
| A2: 惰性生成，首次写入时 | 第一次写 checkpoint/trace 时 | 避免为 `--version` 等命令生成 ID | 导致 run 开始时的 trace 事件可能无 ID |
| **A3: 推荐 — 启动即生成** | `run()` 函数入口处 | 覆盖全部生命周期，确定性 | 对非 run 命令也生成 ID（可忽略，存入日志） |

**对现有系统的影响**:
- 向后兼容：所有旧 checkpoint/trace/memory 没有 `run_id` 字段 → 读取时字段缺失视为 `unknown-run`，不阻塞
- 写入路径：全新增量，不影响现有写入路径的性能
- CLI：新增子命令，不影响现有 `forge evolve`/`forge run` 行为

### 方向 B：版本化数据契约（方向①）

> **为什么需要**: 当前所有持久化数据没有版本标记，意味着任何数据格式变化都是不安全的。版本治理是**规模化采用的前置条件**。

**业务价值**:
- 团队可以在不同版本间安全过渡
- CI/CD 流水线可以声明 `min_forge_version`
- 升级回滚不丢失数据
- 跨版本数据可审计（trace 知道自己是哪个 forge 版本写的）

**核心挑战**:
- **版本兼容性定义**：什么时候 `forge_v2.0` 可以读 `forge_v1.0` 的 checkpoint？这需要定义兼容性契约
- **迁移路径自动机**：大版本升级可能需要多步迁移（v1→v2→v3），而不是一步跨越
- **Harness 脚本版本联动**：gate 协议、check.py 格式等 harness 文件可能随 forge-core 版本变化

**预期架构变更**:

```
新增：core/version/contract.go    — 版本兼容性检查逻辑
修改：persist/checkpoint.go       — Save 时写入 forge_version，Load 时校验
修改：trace/trace.go              — 每个 Event 增加 forge_version
修改：memory/memory.go            — 每个 Entry 增加 forge_version
新增：cmd/forge/migrate.go        — forge migrate 子命令
修改：project.yml schema           — 增加 forge_version: ">=2.5.0" 字段
```

**兼容性策略**：

| 策略 | 行为 | 适用场景 |
|------|------|---------|
| **严格 (Fail-Closed)** | 版本不匹配时拒绝加载 | 核心 checkpoint 数据 |
| **宽松 (Pass-Through)** | 记录版本但不校验 | trace 事件（可接受轻度不一致） |
| **迁移 (Auto-Migrate)** | 检测旧版本，自动升级格式 | 项目配置文件（forge-init 生成的结构） |

**对现有系统的影响**:
- 三个数据格式都需要增加一个 `forge_version string` 字段（JSON 反序列化时兼容缺失值）
- Load 路径增加版本校验步骤（1-2ms 开销，可忽略）
- project.yml 读取路径需要处理缺失 `forge_version` 的旧项目（视为兼容任意版本）

### 方向 C：运行时健康表面与事件总线（方向③增强）

> **为什么需要**: 24h 无人值守系统的核心基础设施是**可被观测**。当前 forge-core 是黑箱，需要变为半透明。

**业务价值**:
- 运维人员可以在不 SSH 的情况下知道系统状态
- CI 系统可以实时获取 evolve 进度
- 为未来的 Web UI/仪表盘提供数据源

**核心挑战**:
- **运行时模型变更**：当前 forge-core 是"执行并退出"模型，健康端点需要某种形式的常驻进程或 IPC
- **最小化复杂度**：引入 HTTP 服务器是一个大锤子，需要更轻量的方案
- **兼容性**：健康端点必须可选（默认关闭），不改变现有行为

**预期架构变更**:

```
新增：core/health/socket.go       — Unix domain socket listener
新增：core/health/handler.go      — 状态序列化
新增：core/eventbus/bus.go        — 轻量事件总线（发布/订阅）
修改：orchestrator/orchestrator.go — 关键生命周期点推送事件
修改：cmd/forge/status.go          — --watch 模式
新增：core/hook/hook.go           — Webhook 触发器
```

**方案权衡**：

| 选项 | 健康暴露方式 | 优点 | 缺点 |
|------|-------------|------|------|
| **A: 推荐 — Unix Domain Socket** | 进程启动时监听 `.forge/forge.sock` | 零网络攻击面，不需端口管理，仅本机访问 | 不支持远程查询 |
| B: TCP HTTP 端点 | `localhost:9xxx` 端口 | 支持远程查询 | 增加网络配置复杂度，更多安全考虑 |
| C: 文件轮询 | 实时更新 `.forge/status.json` | 最简单实现，无新依赖 | 高频率写入产生 IO，竞态条件问题 |

**事件钩子设计**（影响最大的架构决策）：

```go
// 关键生命周期点 → 可配置的钩子
type EventKind int
const (
    PhaseStart    EventKind = iota  // 阶段开始
    PhaseEnd                        // 阶段结束
    GatePass                        // Gate 通过
    GateFail                        // Gate 失败
    ConvergeCheck                   // 收敛检查点
    BudgetWarning                   // 预算预警（超过 80%）
    BudgetExhausted                 // 预算耗尽
    ErrorNonFatal                   // 可恢复错误
    ErrorFatal                      // 致命错误
)
```

每个钩子可配置输出目标：stdout JSON / 文件追加 / HTTP webhook。外部系统通过钩子集成。

**对现有系统的影响**:
- 核心编排循环需要插入事件推送点 → 架构上最重要的变更，需要谨慎设计不增加编排路径的延迟
- 事件总线必须是零成本抽象（空接口时无开销）
- `forge status --watch` 可以复用健康端点的数据序列化逻辑，不增加新的数据源

### 方向 D：结构化诊断框架（方向②）

> **为什么需要**: 当前错误消息的输出方式决定了用户每次遇到问题都是从"Go 错误链 → 猜原因 → 查资料"开始的。一个产品级系统需要`用户先看得懂，再决定怎么做`。

**业务价值**:
- 降低采用门槛——好的错误消息比好的文档更重要
- 减少用户提问——用户能自己理解错误并修复
- 减少操作事故——对关键操作（如 budget 耗尽）提供明确的下一步动作

**核心挑战**:
- **错误分类法**：需要定义完整的错误分类体系（不遗漏、不重叠），并覆盖所有错误路径
- **现有代码改造**：所有 `fmt.Errorf` 调用需要评估是否改为结构化错误——这是一个大范围改动
- **修复建议的生成**：建议不是简单的字符串拼接，需要错误上下文 + 项目状态 + 最佳实践知识

**预期架构变更**:

```
新增：core/errors/kind.go          — 错误分类枚举
新增：core/errors/op_error.go      — 结构化错误类型
修改：全部 `fmt.Errorf` → `errors.NewOp(kind, msg, hint)` — 大规模重构
新增：cmd/forge/why.go             — forge why 诊断命令
新增：core/diagnose/analyzer.go     — 诊断分析引擎
```

**错误分类设计**（架构上最关键的决定）：

```
分类            │ 严重级     │ 用户行动
────────────────┼───────────┼─────────────────────────────
Configuration   │ ERROR     │ 自行修复（改配置/建文件）
Infrastructure  │ ERROR     │ 联系运维（磁盘满/网络超时）
BudgetExceeded  │ WARN      │ 调整预算或等冷却
AgentFailure    │ WARN/ERROR│ 重试或改 agent 卡
Internal        │ ERROR     │ 报告 bug
```

**重要架构原则**: 错误分类不是替代 Go 错误链，而是**增强**。`OpError` 内部保留原始 `error` 链用于开发者调试，外层展示分类 + 严重级 + 修复提示给用户。

**对现有系统的影响**:
- 这是五个方向中**改动范围最大**的——需要修改大量 `fmt.Errorf` 调用点
- 建议采用**渐进式改造**：先为新的错误路径使用结构化错误，再逐步改造旧路径
- 对外接口（gate 返回、trace 事件）需要向后兼容（旧的无分类错误默认标记为 `Internal`）

### 方向 E：优雅降级框架（方向④，P2 适用）

> **为什么需要**: 当前"一个坏行 = 全部废弃"的策略在开发期可以接受，在生产中是不可接受的。需要为数据损坏建立分级恢复策略。

**业务价值**:
- 24h 自治系统的保险策略——系统可以自己修复大部分问题
- 减少"运行时损坏 → 从头开始"的灾难场景

**核心挑战**:
- **恢复的正确性**：从备份恢复的状态可能不是精确一致的——需要明确每个恢复策略的准确性保证

**预期架构变更**:

```
修改：persist/checkpoint.go    — Load 增加备份回退链
修改：memory/memory.go         — decode 增加 Tolerant 模式
修改：trace/trace.go           — scorecard 读取增加截断容错
新增：cmd/forge/repair.go       — 修复子命令
新增：core/selfheal/crosscheck.go — 跨文件一致性校验
```

**恢复策略层级**（架构模式）：

```
L1: 备份回退
  checkpoint.json 损坏 → 尝试 checkpoint.json.1 → .2 → ... → .5
  验证：使用 checksum 或 format-valid 验证每个备份

L2: 跨数据重建
  checkpoint 完全丢失但 trace 存在 → 从 trace 推断最后已知健康状态
  验证：只能提供最佳近似（approximate），标记为 recovered 状态

L3: 干净冷启动
  全部不可恢复 → 新建 checkpoint，但保留旧 trace/memory 作为审计记录
  不丢历史，只丢进度
```

**对现有系统的影响**:
- checkpoint Load 路径增加备份回退逻辑（最小变更，单函数修改）
- memory decode 增加 `Tolerant` 选项（需要确认调用方是否需要）
- 整体来说是增量最小但价值显著的方向

---

## 三、接口设计建议

### 3.1 核心原则

```
向后兼容 > 功能完整 > 性能优化 > 代码优雅
```

ForgeOS 已经有一定的用户基础和数据积累。所有新接口必须遵循：

1. **读取容缺失**：所有新字段（`run_id`, `forge_version` 等）在旧数据中缺失时，不报错
2. **写入向前看**：新写入数据总是携带最新字段，但旧版本 forge 读取时能忽略不认识的新字段
3. **CLI 增而不改**：新子命令不改变现有子命令的行为；现有子命令的输出格式不变（除非语义错误）

### 3.2 关键接口契约

**数据格式契约**（checkpoint/trace/memory 三者统一）：

```
// 当前（隐式, 不声明兼容性）
FormatVersion: "forgeos.checkpoint.v1"  // 固定值

// 目标（显式, 可演进的兼容性）
{
  "_contract": {
    "format": "forgeos.checkpoint",     // 格式名称
    "version": 2,                       // 递增版本号
    "min_compat_version": 1,            // 最小兼容版本
    "forge_version": "2.5.0"           // 写入的 forge-core 版本
  },
  // ... 实际数据
}
```

**为什么引入 `_contract` 对象而非简单加字段**:

| 方案 | 优点 | 缺点 |
|------|------|------|
| **A: 推荐 — `_contract` 块** | 版本信息独立于业务数据，格式校验器可先读取 | 数据略有膨胀 |
| B: 每个结构体加字段 | 简单直接 | 每个新数据格式都要重复同样的字段，无统一校验点 |
| C: 外部元数据文件 | 版本信息不污染数据 | 原子性难保证（两份文件可能不一致） |

### 3.3 错误接口

```go
// 推荐的新错误类型
type OpError struct {
    Kind    ErrorKind   // Configuration | Infrastructure | Agent | Budget | Internal
    Severity Severity   // ERROR | WARN | INFO
    Message string      // 用户可读（"Budget exhausted after $12.50"）
    Detail  string      // 技术细节（"phase 'implementer' overspent by $2.50"）
    Hint    string      // 修复建议（"Use --run-budget-usd to increase limit"）
    Ref     string      // 文档链接
    Cause   error       // 原始错误链（开发者调试用）
}
```

**为什么需要 `OpError` 而非增强现有错误链**:

- 现有错误链的问题：`fmt.Errorf("persist: load checkpoint: %w", err)` 丢失了所有结构化信息
- `OpError` 与 Go 1.13+ 的 `errors.Is/As` 完全兼容（通过 `Unwrap()` 暴露 `Cause`）
- 不影响现有代码的 error 接口（`OpError` 实现了 `error` 接口）

### 3.4 RunID 接口

```go
// 推荐的设计
type RunID struct {
    ID        string    // UUIDv7, 如 "01J3VY8Z2W..."
    CreatedAt time.Time // 从 UUIDv7 解码
    ParentID  string    // 如果由 forge resume 启动，记录被恢复的 run ID
    Trigger   string    // "cli:forge evolve implementer" | "ci:build-1234" | "cron:nightly"
    User      string    // $USER | $BUILD_USER | "system"
    Host      string    // hostname
}
```

**为什么选择 Struct 而非 String**:
- `RunID` 作为一个显式类型让 Go 编译器在函数签名中强制传递（`func DoSomething(id RunID, ...)` vs `func DoSomething(id string, ...)`），避免字符串 ID 在参数传递中被混淆
- 内部可以包含元数据，减少对外部系统的查询压力

---

## 四、技术选型建议

### 4.1 总原则：零外部依赖

ForgeOS 的核心纪律——forge-core (Go) 零外部依赖，harness (Node/Python) 零外部依赖——必须保持。运营面的五个扩展方向**完全可以在零外部依赖前提下实现**：

| 方向 | 所需技术 | 现有满足情况 |
|------|---------|-------------|
| RunID (UUIDv7) | 时间有序 UUID | Go 标准库 `crypto/rand` + 手写编码（~50 行）或 `math/rand/v2` |
| Unix Socket | `net.Listen("unix", ...)` | Go 标准库原生支持 |
| JSON 序列化 | `encoding/json` 带未知字段忽略 | Go 标准库原生支持（`json:"...,omitempty"`） |
| 错误分类 | 自定义 struct + `errors.Is/As` | Go 标准库原生支持 |
| Webhook HTTP | `net/http` 客户端 | Go 标准库原生支持 |

### 4.2 具体选型决策

#### UUIDv7 生成：自建 vs 引入依赖

| 选项 | 代码量 | 优点 | 缺点 |
|------|--------|------|------|
| **A: 推荐 — 自建** | ~50 行 | 零依赖，完全控制，UUID 规范稳定 | 需自己处理边缘情况 |
| B: `github.com/google/uuid` | 1 行导入 | 经过实战检验 | 违背零外部依赖纪律 |
| C: 使用雪花 ID (Snowflake) | ~80 行 | 时间有序，可嵌入 worker ID | 需要 worker ID 分配，分布式复杂度 |

**推荐 A**。UUIDv7 规范 (RFC 9562) 已经是 IETF 标准，自建实现的复杂度极低。

#### 健康端点：Unix Socket vs TCP vs 文件

如前所述，推荐 Unix Socket。补充理由：
- forge-core 是 CLI 工具，不是 Web 服务器——导入 `net/http` 只是为了一个 HTTP 端点是大材小用
- Unix socket 的权限控制通过文件系统实现（`chmod 700 .forge/`），比 TCP 端口更安全
- 可以与现有的 `.forge/` 目录结构统一

#### 结构化错误：新包 vs 增强现有

| 选项 | 改动量 | 优点 | 缺点 |
|------|--------|------|------|
| **A: 推荐 — 新包 `core/errors`** | 新包 + 渐进式迁移 | 不影响现有错误路径，可逐步替换 | 新旧两种错误类型共存期可能混乱 |
| B: 修改现有 `orchestrator.ExecError` | 只改一个类型 | 改动集中 | `ExecError` 语义太窄（只覆盖执行错误），不适用于配置/预算/内部错误 |
| C: 全局 `errors` 函数辅助库 | 新包 + 辅助函数 | 极低侵入 | 可能导致不一致使用（有些地方用新函数，有些地方继续用 `fmt.Errorf`） |

### 4.3 不引入的决策

以下技术**不应引入**：

| 技术 | 被建议的理由 | 拒绝原因 |
|------|-------------|---------|
| protobuf / flatbuffers | 结构化序列化 | JSON 已经足够，且 ForgeOS 的"文件可读"原则比微小的性能增益更重要 |
| gRPC | 健康端点 | Unix socket + 自定义协议比 gRPC 轻 1000 倍 |
| OpenTelemetry SDK | 分布式追踪 | forge-core 是单进程系统，不需要分布式追踪的复杂度 |
| Database (SQLite/Bolt) | 结构化查询 | 文件 + 内存索引的复杂度远低于数据库，且无外部依赖约束 |
| Prometheus client | 指标暴露 | 简单的文本格式（或 JSON）足够，无须引入 Pull 模型 |

---

## 五、实施路线图

### 5.1 优先级矩阵（修正版）

基于验证报告的修正（方向④ P1→P2）：

| 方向 | 优先级 | 预估工作量 | 依赖关系 | 架构影响 |
|------|--------|-----------|---------|---------|
| A: RunID 基元（③+⑤ 合并） | **P0** | 2 sprints | 无 | ⭐⭐⭐（架构基元，影响所有数据路径） |
| B: 版本治理（①） | **P1** | 2 sprints | RunID（复用 _contract 结构） | ⭐⭐（新增字段 + 校验逻辑） |
| C: 健康表面+事件总线（③剩余） | **P1** | 2 sprints | RunID（作为查询键） | ⭐⭐⭐（新的运行时暴露机制） |
| D: 结构化诊断（②） | **P1** | 2 sprints | 无 | ⭐⭐⭐（大规模错误路径改造） |
| E: 优雅降级（④） | **P2** | 1 sprint | 无 | ⭐（局部增量改动） |

**为什么 RunID 是 P0**: 它同时是方向③和方向⑤的前置条件。没有 RunID，运营可观测性无法跨文件关联数据；没有 RunID，跨运行身份无法建立。这是一个解锁多个方向的架构基元。

### 5.2 阶段划分

```
Phase 1 — 基础契约层  (Sprint 1-2, P0-P1)
├── RunID 基础设施（core/id/）
│   ├── UUIDv7 生成器
│   ├── RunID struct + 上下文携带
│   └── FORGE_RUN_ID 环境变量注入
├── 数据格式 _contract 层（persist/trace/memory）
│   ├── 版本字段（write）
│   └── 容缺失读取（read）
└── 验证：forge run list（简单版本）

Phase 2 — 可观测性     (Sprint 3-4, P1)
├── forge status --watch
│   ├── 持续刷新模式
│   └── 状态摘要渲染
├── Unix socket 健康端点
│   ├── socket listener（可选启动）
│   └── /status 查询
├── 事件总线基础
│   ├── 轻量 pub/sub
│   └── 关键生命周期点插入
└── 验证：外部工具可以查询运行中状态

Phase 3 — 诊断        (Sprint 5, P1)
├── 错误分类框架（core/errors/）
│   ├── ErrorKind + Severity 枚举
│   ├── OpError struct
│   └── 关键错误路径的渐进式迁移
├── forge why 诊断命令
│   ├── 上次运行分析
│   ├── 错误分类摘要
│   └── 修复建议生成
└── 验证：常见失败场景有可读输出

Phase 4 — 版本治理     (Sprint 6, P1)
├── 版本兼容性矩阵文档化
├── forge migrate
│   ├── checkpoint 格式迁移
│   ├── project.yml 版本标记
│   └── 迁移回滚
├── project.yml min_forge_version
│   ├── 读取 + 校验
│   └── 拒绝 + 友好提示
└── 验证：不同版本 forge 的互操作场景

Phase 5 — 韧性增强     (Sprint 7, P2)
├── checkpoint 自动备份恢复
│   ├── Load 失败 → 回退链尝试
│   └── 恢复日志
├── memory/trace 容错读取
│   ├── Tolerant 模式
│   └── 损坏条目隔离
├── forge repair
│   ├── 目录完整性扫描
│   └── 交叉引用校验
└── 验证：模拟损坏后的自动恢复场景
```

### 5.3 风险点与缓解策略

| 风险 | 发生概率 | 影响 | 缓解策略 |
|------|---------|------|---------|
| **Phase 1 RunID 设计不当导致后续重构** | 中 | 高 | Sprint 1 先出设计文档（ADR），Reviewer 聚焦于 RunID 语义定义 |
| **Phase 2 事件总线过度设计** | 高 | 中 | 事件总线初期只支持同步发布 + stdout/file 输出。异步分发 + webhook 在第二版加入 |
| **Phase 3 错误分类覆盖面不全** | 中 | 中 | 分类法先以 ADR 形式审查，找边际案例测试（gate 失败、config 错误、网络超时等） |
| **Phase 4 版本迁移破坏已有项目** | 低 | 高 | migrate 必须支持 `--dry-run` 预览模式，且必须可回滚（备份旧数据） |
| **整体：版本升级导致用户抗拒** | 中 | 高 | 新增功能默认开启 + 旧行为完全保留。`forge_version` 字段在旧 forge 读取时被 JSON 省略标记忽略 |

### 5.4 关键决策点清单

每个阶段开始前需要解决的架构决策：

```
Phase 1 决策:
  [ ] RunID 生成时机：启动即生成 vs 惰性生成 → 推荐「启动即生成」
  [ ] Resume 语义：生成新 RunID + parent_run_id vs 复用原 RunID → 推荐「新ID+父指针」
  [ ] _contract 结构：放在顶层 vs 嵌套 vs 外部文件 → 推荐「顶层 _contract 块」
  [ ] UUID 实现：自建 vs 引入库 → 推荐「自建 ~50 行」

Phase 2 决策:
  [ ] 健康端点默认状态：默认关闭 vs 默认开启 → 推荐「默认关，--expose-health 开启」
  [ ] 事件总线 API：chan-based vs callback-based → 推荐「callback-based（零开销空接口）」
  [ ] 事件聚合策略：每个事件立即推送 vs 聚合延迟推送 → 推荐「立即推送 + 下游可自聚合」

Phase 3 决策:
  [ ] 错误分类法覆盖率边界：配置错误包含哪些子类？
  [ ] 修复建议源：静态模板 vs 动态生成 vs 混合 → 推荐「混合（模板 + 运行时上下文）」
  [ ] forge why 输出格式：文本 vs JSON vs 两者 → 推荐「默认文本，--json 模式」

Phase 4 决策:
  [ ] 版本号方案：SemVer vs 日期 vs 递增整数 → 推荐「SemVer（v2.5.0）」
  [ ] 兼容性承诺：什么构成 breaking change？
  [ ] 迁移策略：原地升级 vs 副本升级 → 推荐「副本升级（备份 → 迁移 → 原子替换）」
```

---

## 六、总体评估

### 6.1 核心发现

ForgeOS 的 AI 控制面架构已经达到了很高的成熟度——编排、安全、预算、护栏、超时等机制的设计和实现都经过了充分的验证。**但运营面不是增量功能，而是缺失的架构层**。五个方向合起来解决的是同一个根本问题：ForgeOS 把自己当作一个工具来构建，而不是一个平台。

### 6.2 最大杠杆决策

**RunID 作为架构基元**是这五个方向中唯一一个 P0 决策。它的连锁效应：
- 解锁运营可观测性（方向③）——没有 ID 就没有关联查询
- 解锁跨运行溯源（方向⑤）——没有 ID 就没有归属
- 简化版本审计（方向①）——同一个 `_contract` 结构可以同时携带版本和 ID
- 简化诊断（方向②）——`forge why` 可以基于 RunID 聚合数据
- 简化恢复（方向④）——恢复逻辑可以用 RunID 识别数据的连贯性

### 6.3 架构健康度预测

在完成五个方向后，ForgeOS 的架构健康度变化：

| 维度 | 当前 | Phase 1 后 | Phase 3 后 | Phase 5 后 |
|------|------|-----------|-----------|-----------|
| 数据关联性 | ❌ 无关联 | ✅ RunID 统一 | ✅ RunID + 版本 | ✅ 全链路关联 |
| 可观测性 | ❌ 黑箱 | ⚠️ 基础观测 | ✅ 实时 status | ✅ 事件驱动可观测 |
| 用户体验 | ❌ Go 错误链 | ⚠️ 无变化 | ✅ 结构化错误 | ✅ 诊断 + 修复建议 |
| 可靠性 | ⚠️ 核心逻辑可靠 | ⚠️ 无变化 | ⚠️ 无变化 | ✅ 自恢复容错 |
| 可运维性 | ❌ 无版本管理 | ⚠️ 有版本字段 | ✅ 版本兼容矩阵 | ✅ 安全升级路径 |

### 6.4 一句话总结

**这五个方向的核心架构主题是：ForgeOS 需要从"自己用的工具"进化为"被人用的平台"——而进化的第一步是让所有数据都有身份（RunID）、所有格式都有契约（版本）、所有错误都有分类（诊断）、所有运行都有窗口（观测）、所有故障都有退路（降级）。**
