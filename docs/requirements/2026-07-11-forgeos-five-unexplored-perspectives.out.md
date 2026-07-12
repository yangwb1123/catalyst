现在我已经对代码库进行了全面核查。以下是我的**独立架构评审**。

---

## 验证对照 + 架构反馈

### 数据点校正（小误差，不影响核心论点）

| 声明 | 实际 | 影响 |
|------|------|------|
| "42+ 处裸 `os.*` 调用" | **81 处**（非测试代码，含 `os.Stat`/`os.ReadDir`/`os.MkdirAll`） | 低估了50%，缺口实际上更大 |
| "0 个 Fuzz 测试" | 1 个存在：`internal/routing/routing_test.go:308` `FuzzTierForScore` | 轻微不准确——但这个 fuzz 覆盖面窄（仅 routing 评分边界），不影响 **yaml2json 零 fuzz** 的核心论据 |
| "~146 篇" docs/requirements | 247 个文件（含 120+ 对 `.md` + `.out.md`），原始分析约 120-130 篇 | ≈合理 |
| "1565 行 yaml2json" | 含 394 行测试；产品代码 ~1171 行 | 声明仍成立 |
| "4 个 Benchmark 文件" | ✅ 确认 | |
| "checkpoint 0o644" | ✅ 确认（`checkpoint.go:159`、`memory.go:199`） | |

### 五条的逐条反馈

---

#### ① 性能基准 — 最大的低估

你的分析说 "yaml2json 零 benchmark、TF-IDF 零 benchmark"——这是正确的，但我认为你把**范围定窄了**。

当前 4 个 benchmark 全部是**微基准**（单个函数的 ns/op）。真正有产品价值的是**端到端编排 benchmark**：
一次 `forge run` 从 yaml 解析→mode 决策→prompt 构建→LLM 调用（mock）→gate 评估→trace 写入
的完整延迟。缺少这个意味着：
- 无法回答 "forge evolve 24h 需要多久"—这是采用者最关心的问题
- 架构改动（如 parallel wave）的收益只能理论计算

建议在方向一追加一个**集成 benchmark**（`BenchmarkRunEndToEnd`），用 `httptest.Server` mock LLM，
测一次完整迭代的 wall time。

另外，你说 "benchmark 门是非载重的 advisory"——我不同意这应该成为设计原则。对于 0→1 阶段可以，但
如果目标是方向一 → P1，基准门**最终应成为载重门**。一个导致编排循环慢 3x 的代码变更不应该在
不被阻断的情况下合入 main。建议设两阶段：Phase 1 advisory → Phase 2 blocking with opt-out。

---

#### ② 文件系统韧性 — 最强但最重

这是五个方向中**产品价值最高但工程投入也最大**的方向。你想在 `internal/fsutil/` 封装重试+空间预检+
read-verify+权限校验——这是正确的模式，但需要补充三点：

**a) 范围收缩建议**
你说的 "只封装写路径" 是正确的，但目标清单（persist/memory/trace/checkpoint）还不够精确。
trace 写入（`Emit`）用的是 `*bytes.Buffer`，flush 到文件的其实是 `cmd/forge/evolve.go:483-485`——那里
才是真正的写路径。所以实际需要包装的是 **3 个文件**：`checkpoint.go`、`memory.go`、`evolve.go`（trace + scorecard flush）。

**b) 隐藏的竞态边界**
你没有讨论 `os.Rename` 在 Linux 上是原子的但仅在同一 filesystem。`persist/checkpoint.go:127` 和
`memory_compact.go:44` 的 `os.Rename(tmp, path)` 假设 tmp 和 path 在同一挂载点——如果 `TMPDIR` 是 `/tmp`（tmpfs）
而 `.forge/` 是 NFS，rename(2) 会跨设备失败（EXDEV）。当前代码对 `os.Rename` 的跨设备失败没有 fallback（
`memory_compact.go:45` 直接 `os.Remove(tmp)` 后返回错误，checkpoint.go 也是直接返回）。这在 CI 环境中
（容器内 TMPDIR=/tmp 是 tmpfs）是真实故障模式。

**c) NFS rename 原子性缺失**
你提到了但说得太轻："NFS 不保证跨节点 rename 原子性"。实际上即使**同一节点**的 NFS，rename(2) 也不是
原子的（NFS 协议本身不保证）。对于 checkpoint 这种依赖 rename 实现原子提交的契约，需要文档声明：
"checkpoint 原子性保障仅在本地文件系统（ext4/xfs/btrfs）上成立。在 NFS 上不保证原子性。"

**建议**：将 fsutil 的范围定为 **3 个写入文件 + 1 个接口定义**，而不是一个完整包。太重了在当前架构下。

---

#### ③ 纵深安全 — 最干净的分析，但有遗漏

这是五个方向中最清晰、边界最明确的。但我发现两个遗漏和一个错误信号设计。

**遗漏 1：子进程的 umask 继承**
`checkpoint.go:159` 虽然用了 `0o644`，但实际生效的权限是 `0o644 & ^umask`。如果 forge 进程的 umask 是 `022`，
结果是 `0o644`；但如果 umask 是 `002`（常见于共享 CI 环境），结果是 `0o666`——全局可写。
你需要讨论 umask 问题：要么在 `writeSynced` 中 `syscall.Umask(0o077)` → 恢复 → `f.Chmod(0o600)`。

**遗漏 2：内存中的 credential**
你说 trace 脱敏应该正则匹配 `sk-*`/`AKIA*`——这是好的，但忽略了 **环境变量快照**。
`command_executor.go` 构造的 `childEnv`（`strconv.Atoi(os.Getenv(agentDepthEnv))`）在子进程间传递
环境变量。如果 API key 在 `LLM_API_KEY=sk-xxx` 中，子进程的环境变量中能看到。当前代码没有清
理子进程的环境变量（只用 `os.Environ()` 透传 + 额外设 `FORGE_AGENT_DEPTH`）。

**信号设计错误**：你说 checkpoint 完整性校验用 SHA-256 追加到 JSON——这不应该是方向三的一部分。
这是正确性校验（checkpoint 数据完整性），不是安全（防篡改）。SHA-256 无密钥，不能防篡改，
只能检测位翻转。防篡改需要 HMAC（密钥由 forge operator 提供）。建议拆成两个设计点：
- **数据完整性**：SHA-256（方向五更合适——这是配置/数据完整性审计）
- **防篡改**：HMAC（如果 operator 提供 `FORGE_CHECKPOINT_KEY`）

---

#### ④ 测试平台 — 最务实的分析，fuzzing 策略需要深化

yaml2json fuzzing 是这五个方向中**单行代码产出价值最高**的提议。让我补充关键细节：

**yaml2json 的 fuzz 入口点不是 Decode**
你说 "`go test -fuzz=FuzzDecode`"——但 yaml2json 的 `Decode` 函数签名是 `Decode(r io.Reader) (any, error)`，
fuzz 测试需要 `func(*testing.F, []byte)`。所以标准模式是：
```go
func FuzzDecode(f *testing.F) {
    corpus := []string{"testdata/*.yml"}  // 7 个真实 workflow
    for _, c := range corpus { f.Add(c) }
    f.Fuzz(func(t *testing.T, data string) {
        Decode(strings.NewReader(data))
    })
}
```
这不是问题，只是需要明确。

**更重要的：fuzz 需要关注的边界**
当前 yaml2json 是递归下降解析器（`value.go → mapping.go → sequence.go → scalar.go → inline.go`）。
递归下降解析器的典型 fuzz 杀手是：
1. **极度嵌套**（1000+ 层 `mapping → sequence → mapping → ...`）→ 触发 Go 栈溢出
2. **超大 scalar**（64MB 的单行文本）→ OOM
3. **混合缩进**（tab + space 交替且缩进列数减半 → 解析器 tokenizer 状态爆炸）

这些应该在 fuzz 字典（corpus）中作为种子。

**并行测试问题**
你说 `t.Parallel()` 正确性依赖 `t.TempDir()`——这在当前代码中大部分已做到了（我数了 test 中的 `t.TempDir()` 使用）。
真正的风险不是测试之间的文件系统碰撞，而是**测试和 forge 子进程之间的耦合**：
`command_executor_test.go` 和一些集成测试确实启动了子进程，这些子进程继承父进程的 cwd 和 fd——这
才是并行安全的长尾风险。

---

#### ⑤ 运行时完整性 — 正确但缺一个重要用例

你的配置漂移检测逻辑是合理的，但遗漏了**最重要的运行时变动**：

**agent 卡机读部分的变更**
你说 "agents/*.md 的机读契约部分应被包含，散文描述部分排除"——但 agent 卡的 `VERDICT:`/`CONFIDENCE:`
行不是配置文件，它们是 agent 的**输出契约**。真正需要检测的是 agent 卡中 `## Skill`、`## Workflows`、
`## Gates` 这些**指令区域**的变更。一个正在运行的 evolve 中如果被人改了 planner 的 skill 绑定，
下一轮 phase 的行为会静默改变。

**更隐蔽的场景：`workflows/*.yml` 中 `depends_on` 的变更**
如果 `workflows/review.yml` 的 `depends_on` 从 `[build]` 变成 `[build, test]`，LoopEngine 的
phase 依赖图变了——但 `waves.go` 的 wave 计算只在 `forge run/evolve` 启动时做一次。这意味着
一个新的 `depends_on` 在运行中完全不生效。operator 会困惑。

**建议**：在配置快照中增加**依赖图 hash**。如果 `depends_on` 变了，默认策略应该是 Abort——因为
"用旧依赖图执行新配置"相当于无声地忽略了 operator 的意图。

---

### 优先级排序建议

你的 P0/P1/P2 标签我大致同意，但建议在实施顺序上重新排序：

```
Round 1 (P0 → immediate):  ③ 纵深安全（trace 脱敏钩子 + 0o600）
                            ④ 测试平台（yaml2json fuzzing）
  → 2-3 天，两条独立的低风险增量改进，零架构变更

Round 2 (P1 → near-term):  ② 文件系统韧性（fsutil 写路径封装）
                            ① 性能基准（benchmark.mjs + CI gate）
  → 5-8 天，但两条是互补的——fsutil 为 benchmark 提供可模拟的 I/O 层

Round 3 (P2 → medium):     ⑤ 运行时完整性
  → 需要方向③的 checksum 基础设施 + 方向②的文件系统层作为前提条件
```

理由：方向③和④是**低 hanging fruits**——文件级改动，不改变架构，可独立增量部署。
方向②和①需要更多的架构讨论（接口设计、CI 集成模式）。
方向⑤最重要的基础设施（配置快照格式、漂移策略）依赖于方向③的 checksum。

---

### 全局评价

这份分析在**五个维度上都接地气**——每个方向都有精确的文件行号引用、有边界条件分析、
有 "不做" 的界线。它超越了之前 120+ 份分析的碎片化水平（那些分析往往在 3-5 个方向上
各打一枪就走）。这五个方向如果系统性推进，确实可以把 ForgeOS 从 "功能完备" 推向
"生产就绪"。

最大的盲点我列在这：**你完全没有讨论运维可观测性（observability）**。
ForgeOS 现在有 trace（结构化事件流）、有 scorecard（迭代级度量）、有 doctor（健康检查），
但没有结构化日志（`log/slog` 或 zerolog）、没有 metrics 暴露（`expvar` 或 Prometheus 端点）、
没有 tracing 传播（`context.Context` 中的 trace ID）。一个 24h 无人值守的系统，
operator 需要的不只是 "它最后完成了没"，而是 "它现在在干什么、哪个 gate 卡住了、
memory 用了多少、当前这个 agent 调用花了多久"。**可观测性是生产就绪的通行证**——
比方向①和②更靠近用户价值。

如果你愿意，我可以把这作为一个第六方向来展开。还是说你希望我聚焦在现有五条的某一条
做深度设计评审？
