# Skill: code-review

> Fresh-context 独立 Agent 审查,不让实现者审自己的代码。Output a verdict.

## 目标 (Goal)
对变更做结构化审查,产出明确 verdict(approve / request-changes / block),挡住腐化与缺陷。

## 触发条件 (Triggers)
- 任一实现/重构完成,合入前。
- gate 报警(体积/根目录/循环依赖)。
- 必须由 **fresh-context 的独立 Reviewer** 执行(AGENTS.md)。

## 审查清单 (Checklist)
1. **体积 (size)**:文件 ≤ 500 行、函数 ≤ 50 行;超 → 触发 refactor-large-file。
2. **复杂度 (complexity)**:嵌套/分支深度、可读性;有无更简单解。
3. **重复 (duplication)**:复制粘贴、可抽取的共享逻辑。
4. **职责 (SRP)**:单一职责;分层方向正确(见 skill: clean-architecture);无 God Object。
5. **边界 (boundaries)**:输入校验、错误处理、空值/异常路径、并发与资源释放。
6. **测试 (tests)**:覆盖新行为、含边界用例、覆盖率达标(见 skill: testing)。
7. **命名/契约**:命名达意;公共 API 稳定;无死代码。

## 步骤 (Steps)
1. 取 diff + 关联文件,逐项过清单。
2. 标注每个发现:`严重度 (blocker/major/minor) + 文件:行 + 修复建议`。
3. 跑(或核对)`node harness/gate.mjs` 与测试结果。
4. 综合得出 verdict;有 blocker → 不放行。

## 输出 (Output)
- **Verdict**:approve / request-changes / block。
- 发现清单(按严重度排序,含定位与修复建议)。
- gate + test 结果引用。
