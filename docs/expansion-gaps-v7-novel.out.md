好。以下是我的**架构师级交叉验证与微调响应**。

---

## 验证结论：文档中的代码就绪点全部属实

我已逐项对照代码库做了正面对质：

| 方向 | 分析中声称的就绪点 | 验证结果 |
|------|-------------------|---------|
| 1.联邦 | `memory.Entry` 有 Topic/Confidence/Supersedes | ✅ `memory.go:72-95` 确切定义 |
| | `prompt.Retrieve` BM25-lite | ✅ `retrieve.go` 纯 token-frequency + IDF-lite，14 个 test 覆盖 |
| | `prompt.ContextCache` | ✅ `cache.go` 完整实现了 ADR/AGENTS 懒加载+缓存 |
| 2.质量自适应 | `scorecard_wind.go` 离线记分卡 | ✅ 确凿存在，shell out 到 `harness/scorecard-update.mjs` |
| | `routing.HistoryTiebreak` | ✅ `scorecard.go:137`，按 score 排序，fallback 到候选列表首位 |
| | `Engine.OnGateResult` / `AgentVerdict` | ✅ `engine_build.go:207-208` 注入 `gates.record` / `verdicts.get` |
| | `trace.Event` 有 DurationMs + CostUsdMicros | ✅ `trace.go:70-75` 确切字段，已带 `omitempty` |
| 3.编排测试 | `asset.LoadWorkflowJSON` | ✅ `asset.go:276` |
| | `DryRunExecutor` | ✅ `executor.go:28` 零 LLM 叙述器 |
| | `Engine.RunGate` 注入假 gate | ✅ `orchestrator.go:91` 类型 `func(name string) gate.Result` |
| | `Engine.OnGateResult` / `AgentVerdict` spy | ✅ 测试已大量使用 `gateTracker` 和 `fakeVerdict` |
| 4.多租户 | `persist.Save` 原子写 | ✅ `checkpoint.go:102` temp+fsync+rename |
| | `memory.Append` 用 O_APPEND | ✅ `memory.go` 注释明确描述行级原子 |
| | `trace.Tracer` 的 `mu sync.Mutex` | ✅ `trace.go` 注释明确说 "future phases run in parallel → lock now" |
| 5.告警升级 | `asset.OnFail.Action` 可扩展 | ✅ 当前 `Action=="loop_back"`，扩展为 `"escalate"` 只需加 case |
| | `LoopEngine.NoProgress` 分级行为 | ✅ `loop.go:37`，已有计数→触发的分级设计 |
| | `resolveAutoRisk` | ✅ `engine_build.go:328` 从 git diff 推导风险级别 |

---

## 四个微调点

### 1）方向 1（联邦）的检索层依赖需要澄清

你指出 BM25-lite 的 `prompt.Retrieve` "可直接扩展到跨项目文档集"，这是对的。但当前 `Retrieve` 的输入接口是：

```go
type Doc struct {
    ID   string
    Text string
}
```

跨项目联邦需要 `Doc` 携带**项目来源元数据**（`ProjectID string`、`Namespace string`），否则检索结果无法做权限过滤。建议联邦 v1 的 Doc 扩展中加入 `Scope` 字段（`"public"` / `"project:<name>"`），在 `Retrieve` 之前由调用层做 scope-based pre-filter，而不是污染检索器本身。

### 2）方向 2（质量自适应）的 "在线信号" 启动成本比你估计的更高

你说：

> `Engine.OnGateResult` 和 `Engine.AgentVerdict` 回调已经是实时信号通道

是的，回调接口存在。但**当前的回调都是无返回值的记录器**：

- `OnGateResult func(name string, r gate.Result)` — 写死签名，只记录
- `AgentVerdict func() string` — 是 puller，不是 push signal

要让质量降级**实时反作用于当前 run 的模型路由**，需要：
1. 新增一个 `type QualitySignal struct { Phase, Model string, Reworked bool }` 类型
2. 在 `orchestrator.Engine` 上增加一个 `QualityMonitor func(QualitySignal) (action string)` 字段
3. 在 `Engine.runPhase` 循环中（`orchestrator.go`），每次收到 `AgentVerdict==REQUEST_CHANGES` 时，调 `QualityMonitor`
4. 如果 `QualityMonitor` 返回 `"down_tier"`，当前 run 的剩余 phase 降档

这三步改动不算大，但涉及**预热期**设计：run 的前 K 个 phase 不应触发降档（滑动窗口还未填满）。建议参考 `loop.go` 的 `NoProgress` 模式——用一个 `staleCount` 滑动窗口，连续 N 个 REQUEST_CHANGES 才触发。

### 3）方向 3（编排测试）的断言面可以更精确

你列出 6 个断言面。我认为还缺一个更关键的：

- **assertion 7: 在 loop-back 场景中，`phaseOutputLedger` 的跨 phase 数据传递是否按 `feeds_forward` 正确串联**

当前 `engine_build.go:205` 的 `FeedsForward` 查找不是通过 Engine 字段注入的，而是在 `agentExecutor` 闭包里。编排测试需要验证：planner 的 output → 下游 implementer 的 input 这个链在 mode gating 下是否仍然正确。

方法是写一个测试用 `FeedsForward` spy，验证 `Implementer` 看到上一 phase 的输出正确地出现在了 prompt 中。

### 4）方向 4（多租户）的安全性场景需要修正

> `persist.Save` 的原子写是单进程安全的

这是对的。但 trace 路径的问题不完全是锁问题——`trace.Emit` 已注明锁定正确性依赖的是**每个进程内的 mutex**，但只要文件以 `O_APPEND` 打开，内核 `write(2)` 本身就是原子的（POSIX 保证 < PIPE_BUF 的写入不交错）。trace 单行 JSON 远小于 4KB。所以**两个进程同时 append 到同一个 trace.jsonl 不会损坏行**，只会交叉行的顺序，导致 seq 不对。

真正的破坏性冲突在 **checkpoint**（`persist.Save` 的 temp+rename 是最后写入者赢）和 **scorecard**（`memory.jsonl` 的行级安全但逻辑污染）。你的建议 `flock` + 会话 ID 是正确且轻量的方案。补充一点：`flock` 在 NFS 上不完全可靠，但 `.forge` 目录天然是本地文件系统，所以这不是问题。

---

## 优先级重排：我同意方向 3 排第一，但理由微调

你的分析说方向 3（编排测试）杠杆高、成本低，我同意。但原因需要补充一个战略角度：

**中枢旋钮（mode × lifecycle × gates × reviewer × discover × design × adr × enforce）目前已到 7 维。每个新加入的维度都会使先前所有模式组合的测试覆盖率隐性下降，因为没有编排级断言护栏。**

当前 7 维的理论组合数是 `4 mode × 4 lifecycle × 2 (reviewer on/off) × 2 (discover on/off) × ... = O(10²) 量级`。手工验证在下一轮迭代（再加一个维度）后就不可行了。

### 我对微调后的优先级建议

| 方向 | 杠杆 | 成本 | 依赖关系 | 建议时机 |
|------|------|------|---------|---------|
| **3. 编排测试** | 长期维护护栏 | 低 | 无 | **Sprint 27 启动** |
| **4. 多租户隔离** | 生产安全前提 | 低 | 无 | **Sprint 27（与 3 并行）** |
| 1. 跨项目联邦 | 组织级 G4/G5 | 中 | 依赖方向 3 的质量护栏 | Sprint 28-29 |
| 2. 质量自适应 | 在线调优 G3 | 中 | 无需阻塞，可与 1 并行 | Sprint 28-29 |
| 5. 渐进式告警 | 人机协作 | 高 | 需要方向 4 的进程隔离基础 | Sprint 30+ |

方向 4 和 3 没有依赖冲突，可以并行启动——方向 4 是 `forgeDir` 加锁（< 50 行改动的），方向 3 是 `orchestrator_test.go` 加编排用例（~200 行），两个独立。
