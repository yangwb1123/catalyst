已保存至 `docs/requirements/2026-07-12-five-closure-gap-expansion-directions.md`（495 行，18.6 KB）。

概要：

| 方向 | 核心洞见 | 优先级 |
|------|----------|--------|
| **① 置信度标定** | `CONFIDENCE: N` 自报分数从未被验证/校准；引入 `KindCalibration` + 历史偏差调整 | P1 |
| **② 原型感知工作流** | `forge detect` 输出闲置；定义 `service`/`library`/`cli`/`monolith`/`config` 变体 | P1 |
| **③ 跨相位产物契约** | `emits` 只有文件名无 schema；`forge validate --emits` 离线校验 + prompt 注入 | P1 |
| **④ Tier 感知 Prompt** | Haiku/Sonnet/Opus 收到相同 prompt；`adrTopK`/`taskCap`/`memoryCap` 差异化 + 角色卡分区 | P2 |
| **⑤ 阶段交接协议** | `next_stage` 跳转无产物校验；`stage-passport.json` + `forge validate --stage-readiness` | P2 |

与 **~68+ 已有方向**的差异化证明：关键词 `confidence calibration`、`archetype-aware`、`cross-phase artifact`、`tier-specific prompt`、`stage handoff` 在全部 66 篇已有文档中零命中。
