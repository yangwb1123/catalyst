Now I have a thorough understanding of the codebase. Let me provide the verification-based review.

---

## 验证结果：代码库事实对照分析文档

我已逐项验证了所有 5 个方向的具体代码事实。以下是精度评估：

---

### ✅ 方向一：记忆语义化 & 知识压缩 — **大部分准确，有细节偏差**

**已确认准确：**
- `memory.go` Load 读全部 JSONL → O(N)，无 streaming/分页
- `memory.go` Query 是 exact map filter (kind + topic)，无语义检索
- `prompt/retrieve.go` TF-IDF **不通 memory** — 只用于 ADR 选取
- `cache.go` 明确声明 "does NOT save token cost"
- `loadCache` 在 Append 时被 invalidate → 每次迭代重新读盘

**需修正的细节：**
| 分析文档原文 | 实际代码 | 偏差程度 |
|---|---|---|
| `storeEvolvingMemory`/`loadEvolvingMemory` | 实际是 `recordMemory` (evolve.go line ~357) | 函数名不同但语义一致 |
| `Compact` 只是"保留最近 N 条" | `memory_compact.go` 有年龄分片 + kind 分组 + 生成 summary entry 的精化逻辑，比分析描述的复杂 | **中等** |
| memory 中无"不完整尾行守卫" | 确认无(如 trace.go 的 lastLine 检查) | 准确 |
| `LoopMemory` 字段在 loop.go 行 80+ | 实际在 `orchestrator/loop.go` 中 `LoopEngine` struct 内没有 `LoopMemory` 字段 — signals 通过 `func() converge.Signals` 注入 | **小偏差** |

**Edge Cases 验证：**
- "损坏行直接抛错" → `decode` 确实在坏行返回 error，从不跳过 → 准确
- "超大 entry" → `decode` 有 16MB buffer cap → 有防护
- "并发 Append 不交错" → O_APPEND + 单行写入，准确

---

### ✅ 方向二：弹性质量策略 — **核心论点正确，低估了已有修饰符系统**

**已确认准确：**
- `harness/policies.yml` 是全局常数（max_file_lines:500, max_function_lines:50）
- `internal/gate/resolve.go` 中无 per-path 策略
- `converge.go` CodeTestRatio 只用于 warning，不 gate-breaking
- `routing.go` `opusFloorAgents` 硬编码，不按风险动态调整

**需修正的细节：**
| 分析文档原文 | 实际代码 | 偏差程度 |
|---|---|---|
| "modes.yml 的 coverage_threshold: 80 — 全项目单一阈值" | 真实系统有 **lifecycle_modifiers** — `coverage_delta` 在不同 lifecycle 叠加(production +20pp, cap 95) | **显著低估**：已有部分弹性 |
| "无生命周期演进策略" | 实际已有生命周期修饰符覆盖阈值、enforce 级别、require_min_gates 等 | 不准确 |
| "测试健康复合指标" 缺失 | 已在 `converge.Signals` 中有 CodeTestRatio + FileDelta，但确不用于 gate-breaking | 核心论点仍成立 |
| "opencost 硬编码" | 不贴切 — `opusFloorAgents` 是安全地板，不是"成本编码" | 术语偏差 |

**Edge Cases 验证：**
- "策略冲突" → `resolve.go` 的 N/A matrix 有明确的 category×lifecycle 二维豁免规则，已经有优先级语义
- "策略衰减" 不存在 → 准确

---

### ✅ 方向三：并发安全 — **完全准确，无偏差**

这是分析中最准确的一个方向。所有具体位置均验证通过：

| 位置 | 分析描述 | 验证 |
|---|---|---|
| `persist/checkpoint.go` Save | 原子 rename 不防多进程覆盖 | ✅ 确认：tmp→rename 只防单进程崩溃 |
| `trace/trace.go` Emit | 无进程级锁 | ✅ 确认：有 sync.Mutex 仅协程安全 |
| `memory/memory.go` Append | O_APPEND 安全但无归属标记 | ✅ 确认 |
| `cmd/forge/gates.go` loopProbe | 进程内 cache | ✅ 确认 |
| `cmd/forge/main.go` cmdRun | 无 --lock 语义 | ✅ 确认 |

**Edge Cases 验证：**
- "flock 在进程终止时自动释放" → 准确
- "只读执行不需要写锁" → 准确
- "跨容器锁不存在" → 准确

**建议：** 这是分析文档中最具操作性的方向 — 实现成本最低、验证收益最高。

---

### ✅ 方向四：Prompt 预算 — **基本准确，但 memoryContext 已部分改进**

**已确认准确：**
- `prompt/prompt.go` Build + `prompt_context.go` buildPrompt → 每次 full rebuild
- `prompt/cache.go` → 明确说 not saving tokens
- TF-IDF 不通 memory → 准确
- `routing.go` TierFor 不影响 prompt 内容 → 准确

**需修正的细节：**
| 分析文档原文 | 实际代码 | 偏差程度 |
|---|---|---|
| "全量 memory 注入" | `memoryContext` 已改为 `boundMemory`：recency-floor + relevance 选择，不是全量 | **显著差异** — Sprint 22/23 已加 bound |
| "各种优化都不存在" | memory 注入已有 `memoryCap`(行 38-42)和 `boundMemory` 选择逻辑 | 部分已实现 |

**Edge Cases 验证：**
- "prompt 缓存失效" → `cache.go` 的 CORRECTNESS INVARIANT 说 ROADMAP 不缓存（没有对应字段），设计正确 ✅
- "cache_control 跨厂商" → 当前无此抽象 → 准确指出现状
- "prompt 裁剪的 honesty" → 当前无裁剪告警 → 准确

---

### ✅ 方向五：Gate 增量计算 — **核心准确，但 gate.mjs 排序描述有误**

**已确认准确：**
- `internal/gate/gate.go` ProbeAll → 全量 spawn `node harness/acceptance.mjs`
- `harness/arch/arch-check.mjs` → `scan()` walkAll 全部文件做 AST
- `harness/arch/scan.mjs` → 对全部 Go 文件做 AST imports/exports/functions 解析
- `harness/secret-scan.mjs` → 对全部 scannable 文件做扫描
- `internal/risk/risk_diff.go` FromChangedPaths → 每调用全量 git diff

**需修正的细节：**
| 分析文档原文 | 实际代码 | 偏差 |
|---|---|---|
| "gate.mjs 按时间排序最近修改的文件" | `gate.mjs walk()` 只是 `readdirSync` 遍历，无 mtime 排序 | **小偏差** — 功能描述不准确 |
| "或chestrator/parallel.go 的 depends_on 是精确的先例" | 确实存在 `parallel.go` 带 `depends_on` 逻辑 | 准确 |

---

## 总体评估

| 方向 | 准确度 | 最大偏差 |
|---|---|---|
| ① 记忆语义化 | 85% | Compact 的精化程度被低估 |
| ② 弹性质量策略 | 70% | 低估了 lifecycle_modifiers 系统的已有复杂度 |
| ③ 并发安全 | **95%** | 几乎完全准确 |
| ④ Prompt 预算 | 80% | memoryContext 已部分 bounded，分析未能反映当前状态 |
| ⑤ Gate 增量 | 90% | gate.mjs 排序细节有误 |

**总体评分：约 85% 事实准确**。分析在架构级洞察上非常出色（正确识别了真正的设计空白），但在若干具体实现细节上（特别是方向二的生命周期修饰符和方向四的 memory bounding）需要更新以反映当前代码状态。方向三和方向五几乎无偏差。
