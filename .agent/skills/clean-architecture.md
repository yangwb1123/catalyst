# Skill: clean-architecture

> 依赖只向内。Dependencies point inward; domain knows nothing of the outside.

## 目标 (Goal)
强制单向依赖与分层边界,使核心领域稳定、可测、不被框架/IO 污染。

## 触发条件 (Triggers)
- 新建模块/目录,或迁移既有代码(见 skill: project-reorganization)。
- 出现反向依赖:domain 引用 framework / DB / HTTP;或 application 依赖 presentation。
- code-review 命中分层违规。

## 分层与依赖方向 (Layers & direction)
```
presentation → application → domain
infrastructure ──(实现 domain 定义的 port)──▶ 只被 application 装配/调用
```
- **domain**:实体 + 业务规则 + port(接口)。**零外部依赖**,不 import 其它层。
- **application**:用例编排;依赖 domain 抽象;调用 infrastructure(经 port)。
- **presentation**:UI / API / CLI;只调 application;不含业务规则。
- **infrastructure**:DB / 网络 / 文件等适配器;实现 domain 的 port;由 application 注入。

## 步骤 (Steps)
1. **定位层 (locate)**:判定每段代码归属层。
2. **查方向 (verify)**:依赖只能向内;**禁止任何反向/跨层依赖**。
3. **倒置违规 (invert)**:反向依赖 → 在内层定义 port,外层实现,运行期注入 (DI)。
4. **隔离 IO**:框架/DB/网络全部下沉到 infrastructure,domain 保持纯净。
5. **复检**:`node harness/gate.mjs` 循环依赖 = 0;依赖图无回边。

## 输出 (Output)
- 分层归属表(`文件/模块 → 层`)。
- 违规清单 + 修复方式(invert / 注入 / 下沉)。
- 依赖图(全部箭头向内)+ gate verdict。
