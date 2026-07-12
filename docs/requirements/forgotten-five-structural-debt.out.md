感谢分享这篇深入的分析。我已经对照代码库逐一验证了主要论断。以下是我的反馈：

---

## 验证结果总览

| 方向 | 核心论点 | 准确性 | 关键修正 |
|---|---|---|---|
| ① YAML碎片化 | 3条YAML→JSON路径，可能产生不同结果 | **基本正确** | 对比测试在`yaml2json_test.go`(Go)，非`test_yaml2json.py`(Python)；用的是`t.Errorf`不是`t.Logf` |
| ② `cmd/forge`结构 | CLI层臃肿，含非胶水逻辑 | **基本正确** | 少量函数位置有误(见下) |
| ③ 存储累积 | 无自动存储预算/轮换 | **部分过时** | `evolve.go`已调用`compactMemoryIfDue`每10轮一次 |
| ④ 跨进程文件冲突 | 无进程锁，O_APPEND不安全 | **完全正确** | 零个`flock/LOCK_EX`匹配 |
| ⑤ 库API缺失 | 全部`internal/`不可外部导入 | **完全正确** | 设计决策，建议P2 |

---

## 逐条点评

### 方向一 · YAML解析器三重碎片化 —— 真实且有价值

**已验证**：
- ✅ `internal/yamlpath` 被零个非测试包调用——一个完整可工作的解析器，全仓无人使用
- ✅ Go解析器（`yaml2json.Decode`）明确声明不支持anchor/alias/tag
- ✅ Python shim路径是运行时依赖（`preflight.go:112`警告python3缺失）
- ✅ 差分测试`TestToJSON_MatchesPythonShim`确实存在

**小修正**：
- 该测试在 `forge-core/internal/yaml2json/yaml2json_test.go:320` 而不是 `harness/test_yaml2json.py`；使用的是 `t.Errorf` 而非 `t.Logf`
- 这其实强化了你的论点——测试**确实会失败**，但只对7个已知workflow文件运行，不覆盖所有YAML消费路径

**补充证据**：我注意到 `internal/yamlpath` 的 `Resolve` 函数**硬编码调用** `exec.Command("python3", shim, absFile)`，不经过Go原生解析器。这意味着任何启用yaml路径引用的workflow功能都有一条强制性的Python依赖路径，与 `loadWorkflow` 的双路径设计不一致。

---

### 方向二 · `cmd/forge`包结构债务 —— 核心论点成立，细节需校准

**已验证**：
- ✅ 16个非测试文件，12,513行
- ✅ `cost.go`(471行) 内含大量纯业务逻辑（`parseClaudeCostUsd`、`runBudget`、`costEmitter`、`classifyClaudeOverload`）
- ✅ `prompt_context.go`(454行) 管理4种ledger + 互斥锁状态
- ✅ 项目已有成功拆分先例（`internal/doctor`、`internal/attribution`、`internal/gate/resolve.go`）

**需修正的细节**：
- `checkRunBudget` 在 `internal/orchestrator/budget.go`，不在 `cost.go`
- `accumulateCosts` 函数名不存在（逻辑通过 `runBudget.feed` 实现）
- `observeFor` 在 `prompt_context.go:180`，约52行（非100+行）且不在 `cost.go`
- 文件未超过 `max_file_lines:500` 硬限制（main.go 499行、engine_build.go 498行），但非常接近

**这些细节不影响核心论点**：CLI层仍包含大量本应属于 `internal/` 的逻辑。

---

### 方向三 · 长运行时存储累积效应 —— **需要重写，分析过时**

**关键事实错误**：`forge evolve` **已经** 自动调用 `memory.Compact`。

`evolve.go:407`:
```go
compactMemoryIfDue(root, i, logln)
```

`evolve.go:434` 实现为每10轮执行一次，超过阈值时压缩（`memory.DefaultCompactThreshold`）并按kind分组保留最近条目。

这不是说方向三没价值——而是**分析断言错误**。正确的故事是：
1. ✅ 存在压缩机制，但 `trace.jsonl` 仍无限增长（`Compact` 只针对memory，不处理trace）
2. ✅ `LoopEngine` 确实没有 `MaxStorage` 字段（你列出的四维资源护栏缺失存储维度的论断依然成立）
3. ✅ `.forge/` 目录**整体**没有大小上限或预警
4. ✅ `trace.jsonl` 被 `--resume` 完整重读

**建议重写方向三的叙事**："trace.jsonl 无限增长 + 缺少 .forge/ 总存储预算 + memory compaction 存在但仅在 evolve 中触发（非全局策略）"

---

### 方向四 · 跨进程状态文件冲突 —— **最干净的新方向**

**零瑕疵**：
- `grep "flock\|LOCK_EX" forge-core/` → 0 匹配 ✅
- `O_APPEND` 不保证跨进程行原子性 ✅
- memory缓存的mtime失效风险 ✅
- CI/CD并行的真实威胁 ✅

**补充**：`checkpoint.json` 写覆盖但不原子——`persist.SaveCheckpoint` 使用 `os.WriteFile`（非原子重命名）。两个进程同时写检查点可能导致截断读。

---

### 方向五 · forge-core不可编程集成 —— **正确的方向，恰当的优先级**

对P2的判断我完全同意。但我想补充一个你未展开的点：

`pkg/` vs `internal/` 的抉择不只是重命名——它还涉及**版本契约**。当前 `go.mod` 是 `module forgeos/forge-core` 无版本，而 `cmd/forge/main.go` 里的 `forgeVersion` 是 `-ldflags` 注入的发布字符串。要把包提升到 `pkg/`，需要：
1. 决定 API 稳定性的承诺等级
2. 建立 `go.mod` 版本依赖路径
3. 处理 `internal/` 包之间的现有耦合（`cmd/forge` 引用了 **11个** `internal/` 包）

这些都可能 >3 sprints。

---

## 修正后的优先级评估

| 方向 | 优先级 | 杠杆 | 推荐 |
|---|---|---|---|
| ① YAML碎片化 | **P1** | ⭐⭐⭐⭐⭐ | **下个sprint** |
| ② `cmd/forge`结构 | **P1** | ⭐⭐⭐⭐⭐ | **下个sprint**(与①并行) |
| ③ 存储累积（修正版） | **P2** | ⭐⭐⭐ | 修复trace增长 + memory压缩策略全局化 |
| ④ 跨进程文件冲突 | **P2→P1 for CI/CD** | ⭐⭐⭐ | CI/CD启用前必须做 |
| ⑤ 库API | P2 | ⭐⭐ | 等社区需求触发 |

---

**总体评价**：这是一篇质量非常高的架构分析。5个方向中有4个在事实层面完全成立，1个（方向三）的核心关切仍有效但需要修正叙事。方向④是我认为**最有原创价值**的发现——一个团队显然未系统思考过的 attack surface。方向①的YAML三重碎片化是典型的"有意识但不系统"的技术债，作为P1的提名合理。

如果你希望我针对某个方向展开更深的代码挖掘、或者协助编写对应的工作项拆分，请说。
