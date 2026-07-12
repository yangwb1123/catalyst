# 架构分析报告

## 1. 前置说明：输入异常

您提供的所谓"分析文档"实际上是一次任务执行失败的 stderr 输出，并非架构分析文档。截获的信息表明：

```
Task: TASK FAILED (exit=1, elapsed=1.4s)
Error: 429 GoUsageLimitError
Detail: 5-hour usage limit reached. Resets in 45min.
```

这是一个 **OpenCode/Goose AI 平台的配额耗尽错误**。系统尝试调用 AI 模型（推断为 `go` 模型）进行分析，但触发了速率限制（5 小时用量已达上限，45 分钟后重置）。

**因此，不存在可供分析的架构文档内容。** 如您本意是希望我分析 ForgeOS 项目的架构，请提供实际的 `.agent/ARCHITECTURE.md`、代码库结构或其他文档。以下分析将基于当前可获取的上下文进行推断性回应。

---

## 2. 从错误本身反推架构问题

即便只有错误输出，也可以从中提取出一些架构层面的观察：

### 2.1 当前集成模式分析

| 维度 | 观察 | 评价 |
|------|------|------|
| **AI 服务依赖** | 对 OpenCode API 有强依赖，`go` 模型是分析管线的一部分 | 单点依赖风险 |
| **错误处理** | 429 错误直接导致任务崩溃，无降级/重试机制 | 脆弱的容错设计 |
| **配额管理** | 未在调用前检查配额状态，失败是硬失败 | 缺少预检 |
| **超时/重试** | 1.4s 即失败退出，无指数退避或队列等待 | 缺少 resilience 模式 |

### 2.2 架构债务信号

- **缺少中间层抽象**：调用方直接暴露给底层 API 的配额状态，没有适配器或断路器
- **缺少优雅降级**：没有 fallback 策略（如切换模型、使用缓存、本地分析兜底）
- **缺少可观测性**：仅通过 exit code 和 stderr 报告，无结构化错误事件

---

## 3. 扩展方向（假设目标是 ForgeOS + AI 辅助分析管线）

以下假设错误来源于 ForgeOS 中某个"AI 架构分析"技能或流水线。

### 方向一：AI Provider 抽象层 + 断路器模式

- **为什么需要**：消除对单一 AI 服务的硬依赖；支持多 provider 自动切换（OpenCode / OpenAI / Anthropic / 本地模型）
- **核心挑战**：不同 provider 的 API 语义、费率、延迟差异大；同样的 prompt 在不同模型上的输出质量不一致
- **预期变更**：
  - 新增 `AiProvider` 接口（`chat()` / `analyze()` / `status()`）
  - 实现适配器：`OpenCodeAdapter`、`OpenAIAdapter`、`LocalAdapter`
  - 引入断路器（Circuit Breaker）：配额耗尽→快速失败→自动切至 backup provider
  - 配额预检模块：执行前先 `GET /v1/usage` 检查剩余量
- **影响**：分析管线核心逻辑无需修改，只需通过 DI 注入不同 provider；已有代码需适配接口

**选项对比**：

| 方案 | 复杂度 | 价值 | 风险 |
|------|--------|------|------|
| A. 仅加重试+等待 | 低 | 低（仅缓解当前问题） | 如果 45min 都不够，等待无意义 |
| B. 完整 Provider 抽象+断路器 | 高 | 高（彻底解耦） | 接口设计需要前瞻性，否则频繁改接口 |
| C. 本地模型兜底（如 Ollama） | 中 | 中（离线可用，但质量下降） | 本地模型能力可能不足 |

**推荐**：先做 B 的接口定义 + 断路器，再做 A 的重试策略，C 作为远期可选。

### 方向二：分析结果缓存层

- **为什么需要**：相同代码库的重复分析触发相同 API 调用，浪费配额和时间
- **核心挑战**：缓存失效策略（代码变更 → 缓存失效）；分析内容哈希计算
- **预期变更**：
  - 新增 `AnalysisCache` 模块，以 `(repo_path, file_hash, analysis_type)` 为键
  - 缓存后端可选：本地 JSON / SQLite / Redis（可插拔）
  - 分析前查缓存，命中则跳过 API 调用
- **影响**：无侵入，仅在分析管线入口加一层缓存检查

### 方向三：分片分析与增量分析

- **为什么需要**：大型代码库一次全量分析超出 token 限制或配额预算；增量变更只需分析变更部分
- **核心挑战**：分片粒度设计（按文件？按模块？按架构层？）；跨分片依赖的一致性
- **预期变更**：
  - 分析管线支持 `scope` 参数：`full` / `incremental(diff)` / `module(path)`
  - 新增 `DiffAnalyzer`：读取 git diff，只分析变动文件及其依赖
  - 合并器 `ReportMerger`：将分片分析结果合并为统一报告
- **影响**：分析管线从单一流程变为有向无环图的调度

### 方向四：异步作业队列

- **为什么需要**：分析任务耗时不定（受配额、网络、代码规模影响），同步阻塞 UX 差
- **核心挑战**：作业持久化、进度通知、取消机制
- **预期变更**：
  - 引入作业队列（in-process 或 Bull/Redis）
  - 分析结果通过回调/webhook/轮询获取
  - CLI 支持 `--wait`（同步）和 `--async`（异步）模式
- **影响**：CLI 入口需重构为 async-first；需持久化层

### 方向五：可观测性（Telemetry）

- **为什么需要**：目前没有结构化日志、指标、追踪，遇到问题只能看 stderr
- **核心挑战**：定义关键指标（API 调用量、延迟、错误率、配额消耗速度）
- **预期变更**：
  - 引入结构化日志（如 `pino` 或 `bole`）
  - 计数器：`analysis.api.calls`、`analysis.api.errors.429`、`analysis.cache.hits`
  - 追踪：每个分析请求的完整生命周期（start→provider→complete）
- **影响**：零架构变更，仅在关键路径加埋点

---

## 4. 接口设计建议

### 4.1 AI Provider 接口

```typescript
interface AiProvider {
  name: string;
  status(): Promise<ProviderStatus>;  // 配额、可用性
  analyze(context: AnalysisContext): Promise<AnalysisResult>;
  // 支持流式输出
  analyzeStream(context: AnalysisContext): AsyncIterable<AnalysisChunk>;
}

interface ProviderStatus {
  available: boolean;
  quotaRemaining?: number;
  quotaResetAt?: Date;
  rateLimited: boolean;
}
```

### 4.2 断路器接口

```typescript
interface CircuitBreaker {
  state: 'CLOSED' | 'OPEN' | 'HALF_OPEN';
  call<T>(fn: () => Promise<T>, fallback: () => Promise<T>): Promise<T>;
  recordSuccess(): void;
  recordFailure(): void;
}
```

### 4.3 向后兼容

- 现有调用方如果直接 `require('open-code-sdk')`，需迁移到 `AiProvider` 接口
- 提供过渡期兼容层：`LegacyProviderAdapter` 包装旧 SDK，实现新接口
- 配置支持 `provider: "legacy"` 以保持零改动

---

## 5. 技术选型建议

### 不需要引入新框架

ForgeOS 当前 stack（Node.js + Go + Python，零外部依赖的哲学）下，**不推荐**引入重量级框架。以下能力可以用标准库实现：

| 需求 | 自建方案 | 不选外部库的原因 |
|------|---------|-----------------|
| 断路器 | 20-30 行状态机 | 逻辑简单，不值得依赖 |
| 缓存 | Map / SQLite | 单机足够；Redis 过度设计 |
| 作业队列 | 自建内存队列 + 持久化到 JSON | 无需 Redis 的复杂度 |

### 可考虑引入的轻量依赖

| 场景 | 建议 | 理由 |
|------|------|------|
| 结构化日志 | `pino`（Node）/ `slog`（Go） | 生态标准，日志可消费 |
| HTTP 客户端重试 | 自建（指数退避） | 标准库足够，不必引入 axios/retry |
| 配置管理 | `env` + JSON/YAML 文件 | ForgeOS 已有此模式，无需扩展 |

### 自建 vs 采购决策

| 条件 | 自建 | 采购/集成 |
|------|------|----------|
| AI 模型调用 | 抽象层自建 | 底层 API 采购（OpenAI / Anthropic） |
| 分析引擎 | 自建（领域特定） | 不采购通用代码分析工具（如 SonarQube） |
| 配额管理 | 自建（简单计数） | 不采购 API 网关 |

---

## 6. 实施路线图

### P0（当前问题修复）— 1-2 天

- [ ] 在分析调用前增加配额预检
- [ ] 429 时增加指数退避重试（最多 3 次，间隔 30s/60s/120s）
- [ ] 增加详细的错误消息（告诉用户何时可重试）
- [ ] 增加 `--force` 参数跳过缓存和配额检查

### P1（架构改善）— 1-2 周

- [ ] 定义 `AiProvider` 接口，重构现有调用
- [ ] 实现断路器（Circuit Breaker）
- [ ] 实现分析结果缓存（SQLite）
- [ ] 接入至少一个 backup provider（如 OpenAI GPT-4o）
- [ ] 增加结构化日志

### P2（高级能力）— 1-2 月

- [ ] 分片分析 + 增量分析
- [ ] 异步作业队列
- [ ] 可观测性仪表盘（CLI + 可视化）
- [ ] Provider 自动选择（基于成本/质量/可用性）

### 风险矩阵

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| AI provider 接口频繁变更 | 中 | 高 | 适配器模式，每个 provider 独立 adapter |
| 缓存导致分析结果过时 | 低 | 中 | 缓存 TTL + 文件变更哈希校验 |
| 本地模型能力不足 | 中 | 低 | 仅作为降级兜底，不用于精确分析 |
| 增量分析的粒度不当 | 中 | 高 | 先以文件级粒度，观察后再细化 |

---

## 7. 结论

当前问题是 **运维层面的配额管理不足**，而非架构层面的根本缺陷。但该错误揭露了深层的架构隐患：

1. **缺少抽象层** → 单点故障
2. **缺少容错机制** → 硬失败
3. **缺少可观测性** → 诊断困难

建议从 **P0 配额预检 + 重试** 入手快速止血，随后按路线图逐步引入 Provider 抽象和断路器，最终实现弹性、可观测、可扩展的分析管线。

---

**如需我分析 ForgeOS 的实际架构（BOOTSTRAP.md · ARCHITECTURE.md · 代码库），请直接提供文档路径，我将从架构层面做深度分析。**
