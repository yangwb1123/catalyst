# Skill: security-review

> 安全维度独立审查,输出风险等级。Output a risk rating, gate on critical.

## 目标 (Goal)
就安全维度审查变更,识别可利用风险,产出风险等级与处置建议;critical 必须挡下。

## 触发条件 (Triggers)
- 涉及认证/授权、输入处理、外部 IO、依赖变更、配置/密钥的改动。
- 新增对外接口或数据出入口。
- 由 fresh-context 独立 Reviewer 执行(与实现者分离);risk ≥ critical 触发模型升级 (≥ Opus)。

## 审查维度 (Checklist)
1. **注入 (injection)**:SQL/命令/路径/模板/XSS;须参数化、转义、白名单校验,绝不拼接不可信输入。
2. **密钥 (secrets)**:硬编码凭据/token/私钥;须用环境变量/密钥库;扫描 diff 防泄漏;勿入日志。
3. **依赖供应链 (supply-chain)**:新增/升级依赖的来源与已知 CVE;锁版本;警惕可疑/typosquat 包与构建脚本。
4. **权限 (authz/authorization)**:最小权限;每个敏感操作校验鉴权;防越权 (IDOR)、防权限提升;默认拒绝。
5. **数据 (data)**:敏感数据加密(传输/静态)、PII 处理、错误信息不外泄内部细节。

## 步骤 (Steps)
1. 取 diff,逐维度过清单;追踪不可信输入的数据流 (source → sink)。
2. 标注每个发现:`风险等级 (critical/high/medium/low) + 定位 + 影响 + 缓解措施`。
3. 综合定**总体风险等级**(取最高单项)。
4. critical/high → 阻断,要求修复后复审。

## 输出 (Output)
- **风险等级 (risk rating)**:critical / high / medium / low。
- 发现清单(按等级排序,含数据流、影响、缓解)。
- 放行建议:pass / fix-then-recheck / block。
