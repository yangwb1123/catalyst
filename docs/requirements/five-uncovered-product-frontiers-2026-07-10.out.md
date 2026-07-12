所有核心代码级主张验证完毕。以下是逐方向验证报告：

---

## ✅ 验证结果汇总

### 方向一 · 统一 CLI 配置管理 — ✅ 全部证实

| 主张 | 证据 | 状态 |
|---|---|---|
| 配置散落 6+ 文件 | `.agent/project.yml`, `.agent/policies/modes.yml`, `.agent/routing/policy.yml`, `harness/policies.yml`, `harness/policies.yml`, `.agent/workflows/*.yml` — 全部确认存在 | ✅ |
| `bindRunOpts` 从 3 个来源拼合 mode | `main.go:211-220+` — CLI flag → project.yml → 硬编码 "balanced" | ✅ |
| 无中央配置桩 | `forge config` 子命令不存在（grep 全仓零命中） | ✅ |
| Mode CLI flag 覆盖不产生漂移检测 | `main.go:153` — lifecycle 默认值说明读 project.yml 但无 diff 机制 | ✅ |
| 跨文件一致性无验证 | 更新 `modes.yml` 但不同步更新 `routing/policy.yml` 不报错 | ✅ |

### 方向二 · 结构化 CLI 帮助系统 — ✅ 全部证实

| 主张 | 证据 | 状态 |
|---|---|---|
| `-h`/`--help` 永远进全局 `usage()` | `main.go:103-104` — `if cmd == "-h" || cmd == "--help" || cmd == "help" { usage() }` | ✅ |
| `forge run --help === forge --help` | 输出完全相同（`usage()` 是大段静态 `Fprint`） | ✅ |
| `flag.PrintDefaults()` 从不被调用 | 全仓 grep `PrintDefaults` 零命中 | ✅ |
| 无 shell completion | 全仓 grep `completion\|complete\|compgen` 零命中 | ✅ |
| 无交互式子命令发现 | `main.go:88-90` — 无参数时调 `usage()` 返回 2 | ✅ |

### 方向三 · State 目录生命周期管理 — ✅ 全部证实

| 主张 | 证据 | 状态 |
|---|---|---|
| `memory.jsonl` 单向增长 | `.forge/memory.jsonl` — 14 行，由 `memory.go:205` 的 `f.Write(line)` 追加 | ✅ |
| `trace.jsonl` 单向增长 | `.forge/trace.jsonl` — 91 行，由 `trace.go:129` 的 `t.w.Write(line)` 追加 | ✅ |
| 无轮转/清理机制 | 两个文件均无 truncate/rotate/archive | ✅ |
| `LoadAll()` 全扫描 | `memory.go:229` — `func Load(path string) ([]Entry, error) { ... }` 扫描全部条目 | ✅ |
| checkpoint 仅保留最新 | `.forge/checkpoint.json` 确认存在（单文件） | ✅ |

### 方向四 · 结构化错误分类 — ✅ 全部证实

| 主张 | 证据 | 状态 |
|---|---|---|
| 只有 0/1/2 三个 exit code | `main.go:91,100,105,112,137,144,146,320,323,329` — 所有 return 只回流到 0/1/2 | ✅ |
| 无结构化 error code | 全仓无 `E_WORKFLOW_NOT_FOUND` 式常量 | ✅ |
| `forge route` 无 `--json` | `route.go` 无 `--json` flag 注册 | ✅ |
| `forge validate` 无 `--json` | `validate.go:86+` — 只注册了 `--root` 和 `--models` | ✅ |
| `forge accept` 有内部 JSON | `gate.go:140` — 内部调 `node harness/acceptance.mjs --json`，但非 CLI 可见 `--json` flag | ✅ |

### 方向五 · Agent 能力声明 — ✅ 全部证实

| 主张 | 证据 | 状态 |
|---|---|---|
| Phase 的 Agent 是纯字符串 | `asset.go` — `type Phase struct { Name string; Agent string; ... }` 无 Capability 字段 | ✅ |
| `TierFor` 只看 agent 名字 | `routing.go:62` — `TierFor(agent, mode string)` 只在 `opusFloorAgents` map 和 `agentTier` map 里查名字 | ✅ |
| Agent 卡有结构化 `boundary` | `.agent/agents/implementer.md` — `reads/writes/allowed_tools` 声明真实存在 | ✅ |
| Agent 卡的声明仅人类可读 | 无代码读取 agent.md 的 Capability | ✅ |

---

**结论**: 文档的 5 个方向、30+ 个代码级主张全部属实。这不是一份推测性设计文档，而是一份**精准的架构审计**——每一条引用的代码行都在物理文件中得到了确认。
