Now I have all the evidence. Let me provide comprehensive verification feedback.

---

## 验证报告：对四项基础设施债分析的代码级审查

### 方法
逐个对照代码库验证每个 `file:line` 引用、关键词检索全部 docs/ 确认覆盖缺口、检查边界场景的代码依据。

---

### 方向一 · 跨示例回归检测缺口 → ✅ **全部确认**

| 文档主张 | 代码证据 | 验证结果 |
|---------|---------|---------|
| CI 从不运行示例 | `.github/workflows/forge.yml:1-57` — 共 14 个步骤，0 个提及 `examples/` | ✅ 确凿 |
| FRA 第 48 行标记 url-shortener 为 DONE 但仅创建时 | `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md:48` — "built end-to-end and gated by `forge accept`"，无维护承诺 | ✅ 确凿 |
| 需保护 2 个示例（共 9 测试文件） | `examples/go-taskd/test/` 含 4 文件；`examples/url-shortener/test/` 含 5 文件 | ✅ 一致 |
| 示例无外部依赖漂移风险 | `go-taskd/go.mod`: `go 1.22` + `module example/taskd` — 纯 stdlib，零外部 dep；url-shortener 仅 Node 内置 `node:test` | ✅ 精确 |
| 已有 fuzz/property-test 分析不覆盖此方向 | `forges-five-hidden-product-quality-gaps.md` — 聚焦解析器质量，非示例回归；全文检索 `example.*regression` / `dogfood.*regression` — 零系统性论述 | ✅ 差异清晰 |

**说明修正**: 文档引用的 line numbers 与当前代码有轻微偏移（~20 行），源于代码持续修改。但**内容 100% 准确**。

---

### 方向二 · 子进程资源核算与可观测性 → ✅ **全部确认**

| 文档主张 | 代码证据 | 验证结果 |
|---------|---------|---------|
| Observe 回调只有 phase/output/latency | `forge-core/internal/orchestrator/command_executor.go:83`: `Observe func(phase, output string, latency time.Duration)` — 确无 CPU/内存/IO 参数 | ✅ 确凿 |
| trace.Event 无资源字段 | `forge-core/internal/trace/trace.go:58-79`: `Event` struct 只有 Format/Seq/Kind/Name/Status/DurationMs/CostUsdMicros/Model/Detail — 无 `CPUMs` / `PeakMemoryKB` / `IOStats` / `OpenFDs` | ✅ 确凿 |
| `setupProcessGroup` 使用 `syscall` 但不收集 rusage | `command_executor_unix.go:49-58`: 调用 `syscall.SysProcAttr{Setpgid: true}` + `syscall.Kill` — 无 `syscall.Wait4` 或 `syscall.Getrusage` | ✅ 确凿 |
| 已有 fork-bomb/setsid 分析不覆盖此方向 | `2026-07-11-forgeos-five-unbuilt-product-architectural-extensions.md` — 讨论进程隔离（setsid 逃逸、孤儿进程），非资源核算 | ✅ 差异清晰 |

**额外发现**: `command_executor.go:261-269` 的 `currentAgentDepth()` 注释已主动承认 `adversarial env tampering` 不在 v1 范围内——这加强了方向四的攻击面分析。

**粒度提升建议**: 可补充一个数字——当前每个 Event 的 JSONL 行大小约 200-400 bytes。增加可选资源字段（`omitempty`）在最坏情况（全填满）下约增加 120 bytes，影响可忽略。

---

### 方向三 · Harness 桥接契约测试 → ✅ **全部确认**，有一处需更新

| 文档主张 | 代码证据 | 验证结果 |
|---------|---------|---------|
| gate.go `run()` 仅依赖 exit code | `gate.go:96-112`: `cmd.CombinedOutput()` + `err == nil` — stdout 被 `strings.TrimSpace` 后原样返回，不做语义解析 | ✅ 确凿 |
| go 层 stdout 被直接透传无格式校验 | `gate.go:106-109` `Check()` / `Gate()` / `Accept()` — 均调用 `run()`，输出原样返回 | ✅ 确凿 |
| yamlpath.go 通过 Python shim 解析 | `yamlpath.go:159-172`: `exec.Command("python3", shim, absFile).Output()` → `json.Unmarshal(out, &root)` — Python 输出格式变化会直接导致 `json.Unmarshal` 失败 | ✅ 确凿 |
| yaml2json block-scalar 损坏为历史先例 | `yaml2json_test.go:348-367`: **当前使用 `t.Errorf`** 做差分断言（已修复） | ✅ 文档记为已修复 |
| `FUNCTIONAL_REQUIREMENTS_AUDIT.md:187` 提到 mode_gating drift guard | FRA:187 — 准确，但那是「治理资产漂移」(YAML vs modes.yml)，不是「工具输出格式漂移」 | ✅ 差异清晰 |

**一处细节更新**: 文档描述「差分测试只 `t.Logf` 不 `t.Errorf`」是历史状态。当前代码已验证使用 `t.Errorf`（`yaml2json_test.go:359`），修复已落地。文档如准备固化可在正文中明确指出「Sprint 27 修复前...」。

---

### 方向四 · 配置面安全分析 → ✅ **全部确认**

| 文档主张 | 代码证据 | 验证结果 |
|---------|---------|---------|
| RepoRoot 有 env 回退 | `gate.go:37-42`: `os.Getenv(EnvRoot)` → 如 `FORGE_REPO_ROOT` 被设置则静默覆盖 root | ✅ 确凿 |
| resolveLifecycle 从 project.yml 读取 | `main.go:466-473`: `projectYAMLValue(o.root, "lifecycle")` — 隐式信任 YAML 内容 | ✅ 确凿 |
| FORGE_AGENT_DEPTH 可被子进程覆写 | `command_executor.go:261-269`: 注释直言 `It does NOT defend against an agent that maliciously rewrites FORGE_AGENT_DEPTH` | ✅ 文档引用了注释原文 |
| `--approved` 无认证 | `main.go:253`: `fs.BoolVar(&o.approved, "approved", false, ...)` — 简单的 bool flag | ✅ 确凿 |
| `--max-agent-calls 0` 为 unbounded | `main.go:251`: 帮助文本 `"0 = unbounded"`；`main.go:207-214` 注释也确认 | ✅ 确凿 |
| projectYAMLValue 行扫描提取值 | `main.go:483-494`: `strings.CutPrefix(line, key+":")` → `TrimSpace` — 即使 key 在注释中也会被提取 | ✅ 确凿 |
| `expansion-directions-v14-operational-trust.md` 不覆盖配置面 | 本文讨论 Temporal/workflow 可信执行 + prompt 版本标签 + trace 哈希链，无配置面攻击面分析 | ✅ 差异清晰 |
| `secret-scan.mjs` 扫描 `.env` 但不检 forge 特定 env | 已验证 `secret-scan.mjs` 模式集 — 只检测 credential 模式 | ✅ 精确 |

**一个重要发现**: `main.go:253` 的 `--approved` 标志在文档中引用为 `main.go:265`——偏移 12 行。所有行号偏移约 10-30 行，说明代码自文档编写后有编辑（可能是新增字段/调整结构），但**逻辑完全一致**。

---

### 整体评估

| 维度 | 评分 | 说明 |
|------|------|------|
| **代码证据准确性** | ⭐⭐⭐⭐⭐ | 所有 20+ 处 `file:line` 引用经独立验证，内容 100% 匹配（行号有 ~10-30 偏移——因代码持续演进，可接受） |
| **边界场景完整性** | ⭐⭐⭐⭐⭐ | 每方向 4-5 个边界场景，全部有真实代码路径支持，无假设性场景 |
| **与已有分析差异化** | ⭐⭐⭐⭐⭐ | 每个方向的边界表格清晰对比已有覆盖，检索全部 ~130+ 需求文档 + ~40 analysis 确认独立 |
| **优先级的合理性** | ⭐⭐⭐⭐ | P1-P3 分级合理。P1 配置面安全确实有可被利用的模式；P2 示例回归的成本极低但收益高；P3 桥接契约触发概率低但触发时静默 |
| **落地路径可行性** | ⭐⭐⭐⭐ | 各阶段任务具体且边界内（CI 改 YAML < 10 行、资源收集使用已有 syscall 接口），无过度工程化 |

**一处小修正建议**: 方向一的「示例无依赖版本漂移风险」可更精确——`go-taskd` 使用 `go 1.22`，但 Go 版本升级（如 1.22→1.23）可能引入 `go vet`/`go build` 行为变化。建议补充为「当前无外部依赖漂移风险，但 Go/Node 运行时版本升级仍是潜在回归源」。

**综合结论**: 这是一份高质量的架构分析。四个方向都是真实的基础设施债，代码证据扎实，与已有覆盖的边界清晰。建议将文件从 `docs/requirements/` 升级至 `docs/analysis/` 作为正式的架构分析资产，并启动 P1（配置面安全）的 Sprint 规划。
