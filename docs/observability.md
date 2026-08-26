# 可观测性

## 当前能力

| 面 | 工具 | 说明 |
|---|---|---|
| 运行跟踪 | `forge trace` | 最终 Claude 模型、overload 退避、typed error、stale 增量与已提交定向 loop-back |
| 阶段日志 | forge-core 的 `logf` 输出 | 每阶段命令/延迟/结果(渲染视图) |
| 成本观测 | cost sink | 每阶段 model/成本/延迟 → scorecard p95 |
| 评审产物 | `docs/reviews/reviews/*/stage-*.out.md` | 独立 Agent 评审留档 |
| 迁移 | `state/backups/` | 升级前自动快照 |
| 沙箱 | runner `Logf` | firecracker/docker 生命周期诊断 |

## 已知缺口(诚实)

- 无 SLI/指标/告警/仪表盘(Stage-06 评审 High,记录为后续)。
- 无 hub readiness probe CLI(内部探测原语存在,未暴露)。
- WAL checkpoint/保留策略未自动化。
- 跨边界(Go↔Rust)跟踪不连续。
- stale 连续计数仍为进程内状态；resume 会恢复上一轮信号，但不会恢复崩溃前的累计值。

## 后续

readiness probe CLI、open/migrate 结果日志、指标导出 —— 部署层设计,
见 `docs/reviews/reviews/production-context/stage-06.out.md`。
