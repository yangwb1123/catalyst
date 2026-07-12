# 架构分析报告：API 速率限制与系统弹性设计

## 1. 架构评估

### 当前暴露的问题

从错误信息来看，系统在调用 OpenAI Go 模型时遭遇了 `429 Too Many Requests`，具体为 5 小时用量上限已耗尽。

#### 核心架构短板

| 维度 | 现状 | 风险等级 |
|------|------|----------|
| **熔断与限流** | 调用方未前置检测用量配额，直接发出请求直到被服务端拒绝 | 🔴 高 |
| **回退策略** | 无降级/排队/重试机制，故障导致任务直接失败 | 🔴 高 |
| **可观测性** | 失败信息仅暴露在 stderr，缺乏用量预警、配额监控 | 🟡 中 |
| **调用封装** | 错误类型 `GoUsageLimitError` 表明已有一定抽象，但未在客户端侧做预检 | 🟢 低 |

#### 关键设计决策评估

**合理之处：**
- 使用结构化错误类型（`GoUsageLimitError`）而非裸 HTTP 状态码，利于调用方做类型化分支处理
- 错误携带了恢复时间提示（`Resets in 45min`），为客户端重试策略提供了锚点

**不合理或有隐患：**
- **调用方缺少配额感知**：用量限制是确定性信息（5h 窗口），调用方可以在每次请求前检查本地配额计数器，而不是让请求飞过去才被拒绝
- **缺少指数退避/重试**：429 是临时性错误，系统应自动重试而非直接失败退出
- **任务粒度与配额不匹配**：如果 task 体积很大（如长上下文分析），一次失败导致全部工作丢失，应考虑 checkpoint 或分片

---

## 2. 扩展方向

### 方向一：智能配额管理器（P0）

**为什么需要：** 避免盲目发出一定会被拒绝的请求，减少无效网络开销和延迟，提升系统确定性。

**核心挑战：**
- 配额模型多样（每分钟请求 / 每小时 token / 5 小时滚动窗口 / 并发数）
- 本地计数器与远端实际配额可能存在漂移（并发竞争、未捕获的失败请求）
- 多模型、多部署（Azure / OpenAI direct）的配额池管理

**预期架构变更：**

```
┌────────────┐   配额查询    ┌──────────────┐
│  Client    │ ──────────▶   │  QuotaManager │
│  (agent)   │               │              │
│            │ ◀─────────── │  · local counter
│            │   允许/拒绝   │  · server ack sync
│            │               │  · TTL-based refresh
└────────────┘               └──────────────┘
```

- 新模块：`QuotaManager`（独立于模型调用层）
- 接口变更：`Infer(prompt) → (response, error)` 需增加配额预检前置守卫

**对现有系统的影响：** 低。可做成透明中间件，不影响现有调用链。

### 方向二：优雅降级与回退链（P0）

**为什么需要：** 当主模型不可用时，自动切换到备用模型或简化策略，保证服务不中断。

**核心挑战：**
- 不同模型的能力差异可能导致输出质量断崖
- 降级后的结果需标记 provenance，方便下游判断可信度
- 回退链长度和 timeout 管理

**预期架构变更：**

```
Infer(prompt, opts)
  ├─ Primary:     gpt-4o (429 → fallback)
  ├─ Fallback 1:  gpt-4o-mini (429 → fallback)
  ├─ Fallback 2:  local model (slow path)
  └─ Fallback 3:  cache hit / degraded response
```

- 调用层引入 `FallbackChain` 概念
- 每个请求携带 `degradation` 标记，透传至结果

### 方向三：任务级重试与幂等性（P1）

**为什么需要：** 429 结束后应自动恢复，而不是让用户人工重跑。

**核心挑战：**
- 非幂等操作（如写入数据库）重试需谨慎
- 长任务重试的时机：等待配额恢复的时间窗口可能 > 任务 timeout

**建议方案：**
- 引入 `Retry-After` 头解析（OpenAI 的 429 响应通常带此头）
- 使用指数退避 + jitter，上限 5 分钟
- 对非幂等操作要求调用方提供 `idempotency_key`

### 方向四：可观测性增强——配额预警（P1）

**为什么需要：** 等到 429 发生再处理是被动响应；提前预警让运维有时间采取措施。

**核心变更：**
- `QuotaManager` 暴露 `UsageLevel` 指标（0-100%）
- 阈值预警：80% → warn，95% → critical
- 集成到现有监控体系（Prometheus / OpenTelemetry）

### 方向五：分片与 checkpoint（P2）

**为什么需要：** 超大任务（如全仓分析）一旦失败成本极高，支持断点续传。

**复杂度高，建议在基础弹性能力成熟后再考虑。**

---

## 3. 接口设计建议

### 当前接口评估

从错误签名推断当前调用接口：

```go
// 当前（推测）
func Infer(ctx context.Context, model string, prompt string) (string, error)
```

### 建议的新接口

**选项 A：带配额感知的接口（推荐）**

```go
type InferOptions struct {
    Model          string
    Prompt         string
    MaxRetries     int              // 默认 3
    RetryStrategy  RetryStrategy    // 指数退退 / 固定间隔
    FallbackModels []string         // 降级链
    Timeout        time.Duration
}

type InferResult struct {
    Content     string
    Model       string              // 实际使用的模型
    Degraded    bool                // 是否降级
    QuotaUsed   int                 // 本次消耗的配额
    RetryCount  int
}

func Infer(ctx context.Context, opts InferOptions) (*InferResult, error)
```

**选项 B：管道式（灵活性高，但入侵大）**

```go
pipe := NewPipeline()
pipe.Use(QuotaMiddleware(quotaManager))
pipe.Use(RetryMiddleware(maxRetries=3, backoff=Exponential))
pipe.Use(FallbackMiddleware(models=["gpt-4o", "gpt-4o-mini"]))
result, err := pipe.Infer(ctx, prompt)
```

**我的推荐：选项 A** —— 因为当前系统架构明确（forge-core is Go），选项 B 的 middleware 链在 Go 中需要泛型或 `any` 类型，增加复杂度。选项 A 的 options struct 模式是 Go 惯用做法，向后兼容性好。

### 向后兼容策略

1. 保留旧的 `Infer(ctx, model, prompt)` 签名，内部委托给新接口
2. 新选项字段全部有合理的零值默认（如 `MaxRetries=0` 表示不重试）
3. 错误类型保持向下兼容：`GoUsageLimitError` 继续存在，新增 `QuotaExceededError` 作为前者的超集

---

## 4. 技术选型

### 是否需要引入新技术栈？

| 能力 | 建议 | 理由 |
|------|------|------|
| 配额管理 | 自建（~200 行） | 业务逻辑简单，不值得引入外部依赖 |
| 重试/回退 | 自建（~150 行） | 标准模式，已有经典实现可参考 |
| 可观测性指标 | 接入已有监控系统 | 如果项目尚未有监控，考虑 OpenTelemetry SDK |
| 分布式配额同步 | 如需多进程共享，引入 Redis | 单进程用内存计数器足矣 |

### 第三方依赖评估标准

如果 forge-core 的目标是"零外部依赖"，那么：

1. **零依赖原则优先于功能丰富度**
2. 在极端情况下（如 Redis 客户端），评估标准：
   - 依赖深度（transitive deps 数量）
   - 许可证兼容性
   - API 稳定性（semver）
   - 社区维护活跃度

### 自建 vs 采购决策

对于配额管理器、重试策略等：**自建**。因为：
- 业务逻辑简单，实现成本低
- 零外部依赖是 forge-core 的架构约束（从 CLAUDE.md 已知）
- 这些属于基础设施级代码，不应受第三方 release cycle 影响

---

## 5. 实施路线图

### 优先级排序

| 优先级 | 内容 | 预估工作量 | 价值 |
|--------|------|-----------|------|
| **P0** | 配额管理器 + 指数退避重试 | 2-3 天 | 直接消除 429 导致的任务失败 |
| **P0** | Fallback 回退链（配置化） | 2 天 | 消除单点故障 |
| **P1** | 可观测性指标（配额预警） | 1 天 | 从被动响应转为主动管理 |
| **P1** | Retry-After 头解析 | 0.5 天 | 提升重试精度 |
| **P2** | 任务分片与 checkpoint | 5-7 天 | 边界场景优化 |

### 阶段划分

**Phase 1（本周）：P0 项**
- 实现 `QuotaManager`（本地计数 + TTL 同步）
- 在调用链中插入预检守卫
- 实现指数退避重试（带 jitter）
- 配置化 fallback 模型链

**Phase 2（下周）：P1 项**
- 暴露配额指标
- 集成预警通知
- 解析 `Retry-After` 头

**Phase 3（下月）：P2 项**
- 大任务分片策略设计
- checkpoint 持久化机制

### 风险与缓解

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| 配额计数器漂移 | 中 | 高 | 每次实际调用后 sync server 返回的剩余配额 |
| 并发竞争导致超发 | 低 | 中 | 使用 `sync.Mutex` 或 `atomic` 保护计数器 |
| fallback 模型质量不足 | 中 | 中 | 降级结果打标记，下游可据此调整策略 |
| 重试导致请求堆积 | 低 | 中 | 全局最大并发阈值，队列满则快速失败 |

---

## 总结

当前问题表面是 429 速率限制，深层是 **系统弹性架构的缺失**。核心建议按顺序：

1. **短痛**：实现配额管理 + 重试 + fallback，本周内消除 429 导致的硬失败
2. **中痛**：完善可观测性，从被动响应转为主动管理
3. **长痛**：大任务分片（视实际场景决定是否值得做）

所有修改应在 **零外部依赖** 约束下进行，保持 forge-core 的独立性。
