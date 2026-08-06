# ForgeOS 高维分析工具(移植自 ai-batch-runner)

`docs/ai-batch/` 是 ai-batch-runner 高维特性的**零依赖移植**:
- `pi-batch.py`:薄壳入口(classify/rules/assess/eval 子命令)
- `pbatch/`:依赖闭包(config/text_io/relevance/classifier/rule_matcher/
  product/assessor/eval + role_keywords/task_keywords.yaml),全部 ≤500 行
- `pi-batch.yaml`:声明式配置(可缺省;PyYAML 缺失回退内建默认)

用法:
```bash
python docs/ai-batch/pi-batch.py assess "..."            # 需求评估(8维/规模/分级)
python docs/ai-batch/pi-batch.py assess --file req.md   # 从文件评估
python docs/ai-batch/pi-batch.py rules "..." --json     # 规范匹配
python docs/ai-batch/pi-batch.py classify "..."         # 任务类型判断
python docs/ai-batch/pi-batch.py eval                   # 规则回归套件
```

未移植:runner/pipeline/meta 编排(与 ForgeOS Graph 编排协议重叠)、
campaign/memory/learn(ForgeOS 有等价物)。assess 的 frontend_ui 画像
关键词为上游设计(Flutter/UI 场景),后端任务会误判 —— 以工作流分级与
完整性检查为准。
