我已完整阅读了这份 693 行的 Tech Lead 分析文档。以下是我的**独立评审意见**——作为另一位 Tech Lead 对已有分析的质量验证、盲区补漏和实施方案的可行性评估。

---

# Tech Lead 独立评审：五方向实施分析

> **评审者角色**: 独立 Tech Lead（fresh-context，不参与原始分析撰写）  
> **评审对象**: `2026-07-12-five-verified-direction-tl-analysis.md`  
> **评审范围**: 任务分解质量、依赖正确性、工时估算、风险覆盖、资源可行性  
> **评审方法**: 逐节批判性审阅 + 代码级交叉检查

---

## Executive Summary

**总体评分：8.5/10**。这是一份高质量的实施分析，37 个任务覆盖完整、依赖图准确、风险识别到位。存在三个结构性问题需要修正：

| 问题 | 严重度 | 简要 |
|------|--------|------|
| **工时估算整体偏低约 20-30%** | 🔴 高 | 任务分解缺测试数据准备、review 迭代、集成调试的 buffer |
| **方向间的隐性依赖未标注** | 🟡 中 | T023 (通用解析引擎) 的接口设计依赖于方向一的 Event 结构定义，方向三的 diff 引擎依赖于方向五的策略配置路径 |
| **缺少「安全回滚」机制** | 🟡 中 | 五个方向都有破坏性变更风险，但无一任务描述「如何回滚」 |

以下逐节展开。

---

## 1. 任务分解评审

### 1.1 粒度评估

37 个任务中 35 个在 1-3h 范围内，粒度控制良好。两个异常：

| 任务 | 预估 | 问题 |
|------|:----:|------|
| TASK-019 (canary 模式) | 3h | **严重低估**。canary 涉及 workflow 子集选择、策略隔离、结果对比、`--canary` flag 的全路径——至少 6-8h。如果标注 `v2+ deferred`，应标记 `[DEFERRED]` 并从工时总计中剔除 |
| TASK-005 (进程锁) | 3h | 偏低。需要处理 SIGINT/SIGTERM 信号处理 cleanup（已有 `signal.Notify`）、锁过期检测、跨平台（Windows: `TODO` 应计 1h 占位）→ 4-5h |

### 1.2 缺失任务

| 缺失任务 | 涉及方向 | 理由 | 建议工时 |
|----------|---------|------|:--------:|
| **旧格式兼容性测试数据** | 全部方向 | 每个方向依赖旧 trace/checkpoint/memory 格式的 fixture 数据。现有 repo 中无这些 fixture，需要创建。评审文档提及「向后兼容」但未分配测试数据工时 | 2h |
| **`forge accept` gate 更新** | 全部方向 | 新增文件后 `gate.mjs`（体积/数量）、`arch/arch-check.mjs`（layering）、`check.py`（目录结构检查）都需要更新。每方向 0.5h → 2.5h | 2.5h |
| **ADR-0005 撰写** | 全部方向 | 分析文档提到「建议 ADR-0005」但未分配工时。ADR 撰写 + 评审会议 + 修改 ≈ 4h | 4h |
| **E2E 冒烟测试** | 全部方向 | 分析文档仅提单元/集成测试，缺少端到端冒烟测试（完整的 `forge run` 流程验证 run_id → trace → checkpoint → memory 全链路） | 3h |

**修正后任务总计：41 个任务（+4），总工时约 86.5h（原 75h + 11.5h 新增 + 修正）**。

### 1.3 依赖关系验证

我逐条校验了文档的依赖图代码段，发现一个**缺失边**和一处**不合理依赖**：

```
# 缺失边 (文档未标注):
T023 (通用解析引擎) ──> T024 (替换 parser) 已标注 ✓
但: T023 ──> T001 (Run ID) ❌ 缺失
    → 通用解析引擎的 ParseVerdict 签名需返回 trace 事件字段。
      如果 direction 一的 Trace Event.RunID 用 string，ContractToken.Value 也用 string，
      但若 RunID 用自定义类型则需协调。这是接口依赖，非代码依赖但设计上必须顺序。
```

```
# 不合理依赖:
T035 (checkpoint↔trace 交叉引用) ──> T029 (Retention 配置) 文档标注了
    但 T035 核心是 checkpoint 记录 trace seq 范围，不依赖 retention 配置。
    T035 的正确前置是 T002 (trace run_id) + T001 (Run ID)。
    T029 是 optional 的「retention 配置里加交叉引用参数」，不是 T035 的阻塞依赖。
```

**修正后依赖**：
```
T001 ──> T023 (设计时参考, 非编译依赖)
T001 ──> T035 ✓
T029 ──> T035 ❌ 移除, 改为 T035 ──> T029 (交叉引用可用后, retention 可引用它做 seq 范围保留)
```

---

## 2. 执行顺序评审

### 2.1 Mermaid 依赖图质量

方向内的 DAG 全部正确。方向间的虚线（跨方向依赖）有三处需要修正：

| 虚线边 | 文档标注 | 验证结果 |
|--------|---------|---------|
| T001 → T035 | ✅ RunID 被消费 | **正确** |
| T023 → T029 | ❌ 解析引擎设计参考注册表模式 | **误导**。T023 (契约) 和 T029 (策略) 是正交的。不应画虚线关联。删除或改为注释说明两者设计模式相似但不依赖 |
| T015 → T012 | ⚠️ 策略 diff 可复用方向二配置格式 | **可接受但弱**。policies.yml 格式不同（版本约束 vs 策略定义），实际复用度低。保留但标注 `weak ref` |

### 2.2 并行化建议

文档的 Group A-D 分组合理。但 Group D（测试收尾）可以**提前并行启动**以压缩关键路径：

```
优化方案:
Day 2 起: 每完成一个核心任务立即写对应的测试（非等全部核心完成才写测试）
  → T001 完成 → 立即写 runid_test.go
  → T021 完成 → 立即写 contract_test.go
  → T015 完成 → 立即写 policydiff_test.go
```

可将关键路径从 11 天压缩至 **9-10 天**。

---

## 3. 技术风险 补充

### 3.1 新增高风险项

| 风险 ID | 风险 | 方向 | 概率 | 影响 | 缓解 |
|---------|------|------|:----:|:----:|------|
| **R6: preflight 版本检查误判** | 方向二 | 中 | 高 | `node --version` 输出格式因安装方式而异（nvm/brew/source）。版本字符串解析用 `strings.Fields` 比 semver 库更稳健。**建议：单元测试覆盖所有已知 node --version 输出格式** |
| **R7: 契约注册表与 agent card prose 漂移** | 方向四 | 高 | 中 | agent card 修改者忘记更新 `.agent/contracts/*.yml`。缓解：`forge validate contracts` 必须进入 CI gate + pre-commit hook |
| **R8: 150+ 现有 trace 文件兼容性** | 方向一 | 高 | 中 | 现有 `.forge/trace.jsonl` 文件无 `run_id`。`ReadEvent` 反序列化时旧行 `run_id` 为空字符串，不能报错。**需特意测试** |
| **R9: 进程锁对 CI/CD 环境的干扰** | 方向一 | 中 | 中 | CI（GitHub Actions）可能并行跑多个 `forge run` 在不同工作目录。锁路径是 `.forge/lock`（工作目录级别），理论上隔离，但需要确认 CI 工作流 |

### 3.2 风险严重度重评

| 文档中的风险 | 文档评级 | 我的评级 | 理由 |
|-------------|:--------:|:--------:|------|
| R2 (通用解析引擎破坏匹配) | 🟡 中→高 | 🔴 高→极高 | 这是**五个方向中风险最高的单个代码变更**。现有 `cost.go` 的 3 个 parser (`parseReviewerVerdict`, `parseExecutiveVerdict`, `parseConfidenceScore`) 被 19 个 `.out.md` 文件间接依赖。任何一个的匹配行为变化都会导致既有的审计判断变为「解析不正确」。**建议将此风险提升至最高优先级**，并在 TASK-024 的验收标准中增加「对所有 19 个 `.out.md` 文件中出现的所有 verdict 做解析测试」 |
| R4 (策略 diff 与真实 effect 不一致) | 🟡 中 | 🟢 低 | ACTUAL EFFECT MAY VARY 标注已控制风险。但**建议 diff 引擎直接复用 `migrate.go` 的 effect calculation 逻辑**，从源头避免逻辑分歧 |

### 3.3 技术债务评估

几个方向会引入新的技术债务：

| 债务来源 | 方向 | 说明 | 管理策略 |
|----------|------|------|---------|
| 进程锁 `TODO(windows)` | 方向一 | 跨平台锁定证暂时只做 Unix。但 **TODO 必须在代码中有显式占位 + Issue 追踪**，否则变成永久 TODO | `lock.go` 首行注释 `// +build !windows`；`lock_windows.go` 存根 panic("not implemented") |
| canary `v2+ deferred` | 方向三 | TASK-019 标注 defer 但保留在估算中（3h）。**要么做要么砍，不能悬空** | 从 v1 范围移除，工时归零；在 ROADMAP.md 建 `[future] canary mode` 条目 |
| `forge doctor --repair` | 方向五 | 标注「可选」但估了 3h。可选功能 + 全量估算 → 范围蔓延 | 明确标记 `OPTIONAL`，已分配工时从基线移除，放 buffer pool |

---

## 4. 资源评估 可行性验证

### 4.1 人员需求验证

文档建议 3 人最少 + 1 位兼 Tech Lead。我根据具体任务量验证了可行性：

| 人员 | 文档分配 | 工作量(校正后) | 可行性 |
|------|---------|:-------------:|--------|
| Engineer A (Go, 并发) | 方向一 + 方向五交叉引用 | 18.5h | ✅ **可行**。8 天 18.5h 非常轻松 |
| Engineer B (Go, YAML, 解析器) | 方向四 + 方向三 | 22h | ⚠️ **可行但有风险**。方向四和方向三都是设计密集型，8 天 22h 合理但不留富余 |
| Engineer C (全栈) | 方向二 + 方向五非核心 | 16h | ✅ **轻松**。8 天 16h，可分担其他方向的临时任务 |
| QA | 全部方向测试 | 17.5h | ⚠️ **偏紧**。测试 + 集成 + E2E + regression 实际需要 25h+。建议：QA 从 Day 2 开始（非 Day 9），利用并行写测试 |

**结论**：3 人可完成，但 QA 是瓶颈。**建议增加 1 名兼职 QA 或 Engineer C 分担 10h 测试任务**。

### 4.2 关键路径验证

文档甘特图关键路径：
```
Day 1 (T001) → Day 2 (T002) → Day 6 (T005) → Day 7 (T006) → Day 8 (T007) → Day 10 (测试)
                                                                   ↑ 含 T007 的方向一测试
```

这是 75h 估算下的关键路径（约 10 天）。但考虑到：
- 新增缺失任务 +11.5h → 约 **86.5h**
- QA 单线程瓶颈 → 可能需要 **+2 天**
- ADR 评审（非编码但必须的步骤）→ **+1 天**

**更现实的时间表：12-14 天**（而非文档的 11 天）。

### 4.3 阻塞点重新评估

| 阻塞点 | 文档评级 | 重新评估 |
|--------|:--------:|---------|
| B1 (文件锁跨平台) | 文档已识别 | ✅ 缓解策略合理（先 Unix, Windows TODO） |
| B2 (contract schema 同步) | 文档已识别 | ⚠️ **缓解策略不够**——仅 CI 检查不够，需要 pre-commit hook + agent card 模板包含 contract 引用 |
| B3 (retention 配置扩散) | 文档已识别 | ✅ 缓解策略合理（优先级链） |
| B4 (diff 引擎 vs migrate 双重维护) | 文档已识别 | ⚠️ **需更具体**——「diff 引擎直接复用 migrate 的配置解析」应该成为架构评审的强制要求，而非建议 |

### 4.4 新增阻塞点

| 阻塞点 | 描述 | 严重度 | 缓解 |
|--------|------|:------:|------|
| **B5 — Run ID 格式决策** | UUIDv7 vs ULID vs Snowflake vs 递增 seq。文档默认用 UUIDv7（36 字符 hex）。影响方向一+五的序列化长度和 grep 便利性。这个决策必须在 ADR-0005 中锁定 | 🔴 高 | ADR-0005 中评估 tradeoff：UUIDv7 可排序+唯一性最高，但 36 字符比 ULID 的 26 字符长。建议用 ULID（更短、可排序、grep 友好） |
| **B6 — 契约 schema vs agent card 的双源真相** | `reviewer.md` prose 定义契约内容，`.agent/contracts/reviewer.yml` 定义机器可读版本。prose 修改但 yml 未更新→静默错位 | 🟡 中 | 方案 A（推荐）：agent card 头部嵌入 YAML front matter，单源真相。方案 B：CI 强制验证一致性 |

---

## 5. 质量保证 深度评估

### 5.1 覆盖率目标合理性

| 方向 | 文档目标 | 评估 |
|------|:-------:|------|
| 方向一 | 85% | ✅ **合理**。核心逻辑在独立包，mock 友好 |
| 方向二 | 80% | ⚠️ **建议提升至 85%**。版本解析是纯函数，边界条件多（`v20.11.0`, `Python 3.10.12`, `claude 1.2.3`），测试成本低收益高 |
| 方向三 | 80% | ✅ **合理**。diff 引擎输出格式测试 + migrate dry-run 向后兼容测试 |
| 方向四 | 85% | ✅ **合理**。但需特别关注 A/B regression suite |
| 方向五 | 80% | ✅ **合适**。配置加载测试 + 轮转参数化 + 边界条件 |

### 5.2 A/B 回归测试设计

这是方向四最关键的质量环节。需要对 **替换后 vs 替换前** 逐行断言。我建议设计如下 suite：

```go
// cost_test.go — regression suite

type verdictFixture struct {
    input    string      // agent output snippet
    agent    string      // "reviewer" | "executive" | "confidence"
    expected interface{} // expected parsed value
    fuzzy    bool        // whether it hits fuzzy path
}

// Test data: 30-50 fixtures covering:
// - exact matches (happy path)
// - case variations
// - whitespace variations (missing space, extra space, tab)
// - common typos (APPROVE→APROVE)
// - complete garbage (should return zero value)
// - empty string
// - multiline output with verdict in middle/last line
// - all 19 .out.md files' actual verdict strings
```

**文档缺失此项设计**——「A/B 比对」概念正确但未给出 fixture 构造方法和覆盖率标准。

### 5.3 文档建议补充的测试类型

| 测试类型 | 方向 | 缺失？ | 建议 |
|---------|------|:------:|------|
| 信号处理测试（SIGINT → lock release） | 方向一 | ✅ 未覆盖 | `signal.Notify` 的 channel 行为需要 goroutine leak 测试 |
| 文件锁 TTL 过期 | 方向一 | ✅ 未覆盖 | 模拟锁文件创建时间 > TTL 的场景 |
| `forge doctor storage` 损坏文件 | 方向五 | ✅ 未覆盖 | `trace.jsonl` 中间行损坏 → doctor 输出 WARN 而非 crash |
| concurrent memory append | 方向一 | ✅ 未覆盖 | `memory.Append` 在并发下是否安全（文件名含 run_id 隔离） |

---

## 6. 实施计划 可行性评审

### 6.1 甘特图逻辑验证

文档甘特图有 4 个阶段 + 关键依赖。我用 work breakdown 做了 Day-by-day 负载模拟：

```
Day-by-day 负载（按文档分配 75h / 3 人 / 11 天）:

         Engineer A    Engineer B    Engineer C/QA
Day 1     4h (T001)     6h (T021+22)  3h (T029)
Day 2     4h (T002)     6h (T015)     3h (T008+09)
Day 3     3h (T003+04)  6h (T023)     3h (T010+11+30)
Day 4     3h (T006)     6h (T027)     6h (T031+32)
Day 5     6h (T005)     6h (T016+17)  4h (T012+05部分)
Day 6     0h             6h (T018)     6h (T033+T035)
Day 7     0h             6h (T024+25)  6h (T035+T034)
Day 8     3h (T007)     6h (T026)     6h (T013+T019)
Day 9     3h (T007续)   4h (T020)     6h (T036+T037)
Day 10    0h             4h (T020续)   4h (T037续+集成)
Day 11    3h(审查)       3h(审查)      3h(审查+文档)
```

**问题**：Engineer A 在 Day 5 后闲置（除非承担其他方向任务）。Engineer C 从 Day 4 开始持续过载（6h/天持续 5 天）。**实际需要更均衡的负载分配**。

### 6.2 修正后甘特图建议

基于 3 人 + QA 兼职（共 3.5 FTE）的优化排期：

| 阶段 | 时间 | Engineer A | Engineer B | Engineer C | 
|------|------|------------|------------|------------|
| **I: 基础设施** (Day 1-3) | 7/14-7/16 | T001(4h) → T002(4h) → T003+T004(3h) | T021+T022(4h) → T015(6h) → T023(6h) | T029(3h) → T008+T009+T010(4h) → T011+T030(3h) |
| **II: 核心逻辑** (Day 3-7) | 7/16-7/20 | T005(5h) → T006(2h) → **助方向五** T031+T032(4h) | T027(4h) → T016+T017(6h) → T018(4h) → T024(4h) | T012(2h) → T033(4h) → T035(4h) → **助测试** T007(4h) |
| **III: 整合增强** (Day 7-9) | 7/20-7/22 | T034(2h) → **测试 T007续**(4h) → ADR文档(4h) | T025+T026(3h) → T020(6h) → **集成测试**(4h) | T013(2h) → **测试 T014**(4h) → **测试 T037**(4h) |
| **IV: 验收发布** (Day 9-11) | 7/22-7/24 | T028(6h) → E2E冒烟(3h) | T037续(2h) → gate更新(3h) → **forge accept**(2h) | 文档更新(3h) → 独立审查(4h) |

**修正后总工期：11 天（与文档一致但负载更均衡）**。

### 6.3 发布 checklist

文档缺少发布 checklist。建议增加：

```
☐ 所有新增字段的向后兼容测试 PASS（读旧格式不崩溃）
☐ `go test -race -count=5 ./...` zero race zero flaky
☐ `forge accept` ACCEPTED
☐ 旧 `.forge/trace.jsonl` 不因新增 run_id 字段而报错
☐ 无 `.agent/contracts/` 目录时 forge 正常启动（方向四退化）
☐ 无 retention 配置时方向五使用硬编码默认值
☐ 进程锁不阻止 `forge doctor` 运行
☐ BOOTSTRAP.md / ROADMAP.md / .agent/CURRENT_SPRINT.md 更新
☐ 方向一~五的新增 CLI flag 全部在 `forge --help` 中可见
☐ canary 模式明确标注 `[v2+]` 从当前版本隐藏
☐ `.github/workflows/forge.yml` CI 包含新的测试 suite
```

---

## 7. 架构评审焦点（ADR-0005 准备）

如果安排 ADR-0005 评审，以下是我认为需要**重点讨论**的 5 个接口决策：

### 7.1 Run ID 数据类型

| 选项 | 长度 | 可排序 | grep 友好 | 建议 |
|------|:----:|:------:|:---------:|:----:|
| UUID v7 | 36 字符 | ✅ | ⚠️ 太长 | ❌ |
| **ULID** | **26 字符** | **✅** | **✅** | **✅ 推荐** |
| Snowflake | 19 字符 | ✅ | ✅ | ❌ 需 ID 生成器依赖 |
| `timestamp-PID-random` | ~30 字符 | ✅ | ⚠️ | ❌ 不可伪造性不足 |

**建议：ULID**。比 UUIDv7 短 10 字符、时间有序、不依赖第三方库（Go 实现 200 行纯标准库）。

### 7.2 Contract Registry 接口设计

```go
// 文档中的设计 (接受)
type ContractRegistry struct {
    byAgent map[string]Contract
}
func (r *ContractRegistry) ParseVerdict(agentType, output string) (value string, ok bool, fuzzy bool)

// 我的改进建议:
func (r *ContractRegistry) ParseVerdict(agentType, output string) VerdictResult

type VerdictResult struct {
    Value string       `json:"value"`     // parsed value
    OK    bool         `json:"ok"`        // whether parsing succeeded
    Fuzzy bool         `json:"fuzzy"`     // fuzzy match used
    Warns []string     `json:"warns"`     // warnings (e.g., "expected space after VERDICT:")
}
```

返回 struct 而非 triple string 的收益：trace 事件序列化更友好、可扩展（以后加 `Confidence float64` 无需改签名）。

### 7.3 配置层级优先级

文档提出 `CLI flag > .forge/policy.yml > project.yml > 硬编码默认`。需要确认：

- `.forge/policy.yml` 是 per-project 还是 per-workspace 的配置？
- 如果 `project.yml` 已有 `retention` 字段（未来），与 `.forge/policy.yml` 的冲突如何解决？
- 我建议：**统一到单一源**——`project.yml` 作为唯一配置入口，`.forge/policy.yml` 废弃（或作为运行时的临时覆盖，不承诺持久化）。

### 7.4 进程锁的跨平台策略

- Unix: `syscall.Flock` + `signal.Notify` → cleanup
- Windows: `LockFileEx` via `syscall` 或 lowest-effort 用 `os.Create` + O_EXCL（无强制性但足够 advisory）
- **必须在 ADR 中记录跨平台差异**，并在 `lock.go` 顶部注释清晰的平台选择矩阵

### 7.5 方向四的退化模式

方向四的「没有 `.agent/contracts/` 时退化到 switch」必须是可逆的。但更干净的方案是：

```go
// 初始化优先级:
// 1. 加载 .agent/contracts/*.yml → 成功则用新引擎
// 2. 加载失败或目录不存在 → 加载内置的 defaults（hardcoded switch 等价物）→ 作为 ContractRegistry 初始化
// 3. 无法初始化 → panic（不该发生）

func NewContractRegistry() (*ContractRegistry, error) {
    if contracts, err := loadContractDir(".agent/contracts"); err == nil {
        return contracts, nil
    }
    return builtinDefaults(), nil // 向后兼容的 switch 等价物
}
```

不搞「运行时 fallback」——在 init 时确定引擎，避免运行时分支。

---

## 8. 最终建议

### 8.1 必须做的 3 件事（不做到不开工）

1. **ADR-0005 评审**（4h，Day 0.5）：锁定 Run ID 类型、Contract Registry 接口、配置层级优先级。所有达成一致的决策写入 ADR 后签入。
2. **A/B regression suite fixture 收集**（2h，Day 0）：从 100+ `.out.md` 中提取所有 verdict 字符串，作为方向四的测试基线。**这件事不做，方向四不能合并**。
3. **跨平台锁策略决策**（Orientation 会议 30min）：Unix 先做，Windows 明确标记 TODO 并建 GitHub Issue。

### 8.2 推荐的优先级微调

| 优先级 | 文档推荐 | 我的推荐 | 理由 |
|:------:|----------|----------|------|
| P0 | 方向四 | **方向四** | 一致。修复解析 bug 的 ROI 最高 |
| P1 | 方向一 | **方向一** | 一致。Run ID 是其他方向的基础设施 |
| P2 | 方向三 | **方向五** | **与文档分歧**。方向三的 diff 引擎虽然差异化价值高但用户故事更弱（谁在问这个功能？）。方向五的 retention 配置化和存储健康是管理员实际痛点（trace 爆盘是无提示的静默失败） |
| P3 | 方向五 | **方向三** | 互换 |
| P4 | 方向二 | **方向二** | 一致。最小工作放最后 |

### 8.3 风险登记册最终状态

| 风险 | 处置 | 责任人 | 截止日 |
|------|------|--------|:------:|
| R1 进程锁假死 | 缓解（TTL + `--force`） | Eng A | Day 2 |
| R2 解析引擎破坏匹配 | **提升为极高级** | Eng B + QA | Day 6 |
| R3 配置错误删数据 | 缓解（单位校验 + fail-safe 默认值） | Eng C | Day 2 |
| R4 diff vs effect 不一致 | 接受（标注） | Eng B | Day 4 |
| R5 版本检查过严 | 缓解（两级检查 + `--skip-version-check`） | Eng C | Day 3 |
| R6 版本解析误判 | 缓解（strings.Fields + 多格式测试） | Eng C | Day 3 |
| R7 契约漂移 | 缓解（CI + pre-commit hook） | Eng B | Day 5 |
| R8 旧 trace 兼容 | 缓解（ReadEvent 空 run_id 不报错测试） | Eng A | Day 5 |
| R9 CI 锁干扰 | 接受（工作目录级别隔离） | Eng A + DevOps | Day 4 |
| **R10 工时不足导致仓促合入** | **缓解（基线改为 86.5h，增加 buffer 2 天）** | **TL** | **全周期** |

---

## 总结

这是一份扎实的分析，在我预期之上。最大的价值在于：
1. **方向四**从「parser 脆弱性」提升到 Schema 化注册表的完整架构——这是此前所有分析未触及的角度
2. **TASK-001~037 编号体系**让 5 个方向可以同时追踪、依赖可表达、进度可度量

需要改进的核心 3 点：
1. **工时+14%**（75→86h）、增加 4 个缺失任务、修正 2 条依赖边
2. **R2（解析引擎破坏匹配）风险评级提升至极高级**，需要专门的 A/B regression framework
3. **明确「不做」清单的强制执行机制**——尤其是 canary (T019) 和 repair (T036) 的 `OPTIONAL` 标记需要在甘特图中可见
