好，现在我有了完整图景。这是一份全面的状态报告。

---

## 当前状态：缺口分析文档 vs 实际工作树

你发送的 `docs/scan-current-gaps.md` 基于 `b0c80e4` 提交分析。**工作树已在并行推进**——2,774 行新增、651 行删除。以下是逐项对照：

### ✅ 方向 1: CI 全链路 — **已实现（未提交）**

`.github/workflows/forge.yml` 已增加：
- `go build ./...` — 编译错误拦截
- `go test -race ./...` — 数据竞争检测
- `node --test harness/` — harness 单元测试
- `forge run build --executor dry` — 端到端冒烟测试

### ✅ 方向 2: Memory 置信度/撤回 — **已实现（远超分析范围）**

分析文档预测：~300 行。实际：
- **+399 行** 到 `memory.go`
- `Entry` 新增 `Confidence`（omitempty，0→1.0 向后兼容）
- `Entry` 新增 `Supersedes`（Topic 级 retraction）
- `filterSuperseded` 双遍纯过滤（右→左标记活动 superseder，左→右过滤）
- `Compact` 机制：按阈值（500）+ 按 kind 保留（20）+ 按年龄（24h）自动压缩
- `sync.Map` 负载缓存（按 path+mtime，Append 时失效）
- 新增 `Source` 字段（溯源标记）
- 测试 +233 行

### ✅ 方向 5: Converge 交叉验证 — **已实现**

- `Signals` 新增 `FileDelta` 字段
- 同时新增：`RequirementConfidence`、`ReviewStatus`、`HumanApproved`、`Criteria`、`GateProof`、`CodeTestRatio`
- `Converge` 新增 +68 行结构 +51 行测试

### ✅ YAML Python shim 替换 — **已实现**

- `internal/yaml2json` — 756 行完整 Go YAML→JSON 转换
- `internal/yamlpath` — 196 行 YAML 路径查询
- 配套测试：+367 行

### 🟡 方向 3: Cross-vendor 路由 — **部分准备**

- `Rank`、`HaikuMax`、`SonnetMax`、`TaskTypeFloor`、`SafetyForceOpus`、`BandForScore` 已导出
- 注释已更新说明 `provider/tier` 复合键
- **缺失**：实际多厂商路由逻辑、LiteLLM 集成、策略激活

### ❌ 方向 4: WASM Gate — **未开始**

无任何 WASM/wazero 相关代码。

---

## 🚨 阻塞问题：forge accept 拒绝

当前工作树无法通过 `forge accept`：

```
forge-accept: REJECTED — test_pass failed; complexity_violations failed; architecture failed
```

### 1. 8 个超大文件（gate 体积违规 >500 行）

| 文件 | HEAD | 当前 | 状态 |
|------|------|------|------|
| `forge-core/cmd/forge/validate.go` | — | **994** | 🔴 新文件，需拆分 |
| `forge-core/internal/yaml2json/yaml2json.go` | — | **755** | 🔴 新文件，需拆分 |
| `forge-core/internal/memory/memory.go` | 199 | **568** | 🔴 增长 369 行超限 |
| `forge-core/cmd/forge/main.go` | 490 | **561** | 🟠 增长 71 行超限 |
| `forge-core/cmd/forge/main_test.go` | 465 | **525** | 🟠 增长 60 行超限 |
| `forge-core/cmd/forge/prompt_context.go` | 374 | **524** | 🟠 增长 150 行超限 |
| `forge-core/cmd/forge/evolve.go` | 440 | **504** | 🟡 增长 64 行刚超限 |
| `forge-core/internal/mode/mode.go` | 498 | **500** | 🟡 边缘 |

### 2. test_pass 失败

需要调查具体失败的测试。

---

## 建议下一步

按 **依赖顺序**：

### 第 1 步：拆分超大文件（立即）
用 `refactor-large-file` skill 处理 8 个文件，按职责拆分后再提交。

建议优先级：
1. `validate.go` (994→~2×500) — 新文件，最紧急
2. `yaml2json/yaml2json.go` (755→~2×380) — 新文件
3. `memory/memory.go` (568→~2×285) — 可抽离 cache/compact 到子文件
4. `prompt_context.go` (524→~2×262)
5. `main.go` (561→~2×280)
6. 其余视需要

### 第 2 步：修复 test_pass
运行 `node --test harness/` 定位具体失败的测试。

### 第 3 步：commit 当前完成的工作
方向 1、2、5、YAML 替换——形成一个大的原子 commit。

### 第 4 步：评估剩余方向
- **方向 3（Cross-vendor）**：按分析建议——等待触发条件，不提前做
- **方向 4（WASM）**：POC 可行，但非关键路径

---

要我立即开始拆分文件（第 1 步）来解阻塞？还是你想先讨论方向？
