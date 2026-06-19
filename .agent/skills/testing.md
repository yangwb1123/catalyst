# Skill: testing

> 先写测试,分层覆盖。Red → Green → Refactor; cover by layer.

## 目标 (Goal)
以 TDD 驱动实现,达成 ≥ 覆盖率阈值,并按 unit/integration/e2e 分层组织测试。

## 触发条件 (Triggers)
- 新建/修改行为代码(实现前)。
- 覆盖率 < 阈值(默认 `coverage_min` = 80%;无配置时取 80)。
- refactor 后需回归(见 skill: refactor-large-file)。
- bug 修复:先写复现该 bug 的失败测试。

## 步骤 (Steps)
1. **TDD 循环**:**Red**(先写失败测试,锚定一条行为)→ **Green**(最小实现使其通过)→ **Refactor**(清理,测试保持绿)。
2. **分层 (test pyramid)**:
   - **unit**:纯逻辑/单模块,mock 边界,快、多。
   - **integration**:模块间 + 真实适配器(DB/HTTP),中量。
   - **e2e**:经 presentation 的关键用户路径,少而精。
3. **覆盖关键面**:正常路径 + 边界 + 错误/异常 + 空值。
4. **测试即规格**:命名表达行为(`should ... when ...`);一个测试一个断言主题;确定性、可独立运行。
5. **测量**:跑覆盖率;低于阈值则补测关键分支(勿为凑数写无意义断言)。
6. **复检**:全套测试 + `node harness/gate.mjs` 全绿。

## 输出 (Output)
- 新增/更新测试清单(按 unit/integration/e2e 分组)。
- 测试结果(pass/fail)+ 覆盖率数值 vs 阈值。
- 未覆盖的关键缺口(若有)。
