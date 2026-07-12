Tech Lead 分析已完成并写入。

## 交付概要

**文件**: `docs/requirements/2026-07-12-five-verified-direction-tl-analysis.md`（693 行）

### 核心调整说明

原始文件基于**另一组 5 方向**（运行时上下文注入 / 增量调度 / 成本预测 / 失败升级 / 遥测总线）——与本输入文档的 5 信任缺口完全不同。已完全重写，聚焦输入文档的真实方向：

| 方向 | 优先级 | 任务数 | 核心决策 |
|---|---|---|---|
| **方向四：契约 Schema 化** | **P1** | 8 | 用 YAML Schema 注册表替换 `switch` 精确匹配，消除 `VERDICT:APPROVE`（缺空格静默 fail-open）的真实 bug |
| **方向一：Run Identity** | **P1** | 7 | UUIDv7 Run ID + 进程锁 + trace/checkpoint/memory 三方注入 |
| **方向三：策略变更预览** | **P2** | 6 | `forge policy plan` CLI + migrate dry-run 增强 + canary 框架（v2+） |
| **方向五：存储生命周期** | **P2** | 9 | retention 配置化（已有 rotation/compaction 保留）+ 健康自检 + 交叉引用 |
| **方向二：运行时依赖** | **P3** | 7 | Node/Python/Claude/Go 版本检查，配置化门槛 |

### 关键差异点

- **回避了原有文件的错误前提**：方向五的 trace rotation（`evolve.go:489`）和 memory compaction（`evolve.go:438`）已存在，分析聚焦在「不可配置 + 无健康告警」的真实缺口
- **37 个任务的依赖图明确标注了可并行 Group A~D**
- **11 天实施计划**（3 人团队），含甘特图
- **7.4 "不做"清单**防止镀金
