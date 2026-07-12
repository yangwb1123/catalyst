已写入 `docs/requirements/global-scan-five-codegrounded-extension-directions.md`（~17KB）。

与已有 150+ 份分析文档交叉核对后，本文 5 个方向均从代码级未被深入覆盖的结构性缺口出发：

| 方向 | 核心缺口 | 代码证据来源 | 优先级 |
|------|---------|------------|--------|
| **一·错误灰色地带** | `KindTimeout` 的部分写污染 / `KindFailed` 的持久软错误 / 孙子进程逃逸 | `exec_error.go` + `command_executor_unix.go` | P1 |
| **二·project.yml schema** | 零值容忍使拼写错误静默绕过安全模型 | `asset.go:27-32` + `mode.go:50-70` | P1 |
| **三·Trace 格式进化** | 无 schema 注册表 / 无分段归档 / 无 mode_snapshot | `trace.go:106-110` + `scorecard_wind.go:246-270` | P2 |
| **四·红线自动门控** | `arch-check` 不在 CC hook 路径 / 无增量预警 / 预算手动调整 | `gate.mjs` hook 配置 + `.arch/rules.yaml` 历史 | P1 |
| **五·子进程最小权限** | `buildEnv()` 继承全部环境变量 / 无 argv allowlist / 写权限全开 | `command_executor.go:170-180` + `engine_build.go:90-125` | P2 |

每方向包含精确代码行号、边界场景表、建议方向和产品价值分析，末尾附分期收敛建议（只做一件→做前三件→全部）。
