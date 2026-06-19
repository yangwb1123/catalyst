# Skill: refactor-large-file

> 文件超阈值 → 按职责拆分,杀「上帝文件」。Split by responsibility before adding more.

## 目标 (Goal)
把一个臃肿文件拆成多个**单一职责**单元,使每个 ≤ `max_file_lines`(500),行为不变。

## 触发条件 (Triggers)
- 任一代码文件行数 > `harness/policies.yml: max_file_lines`(默认 500)。
- gate (`node harness/gate.mjs`) 报 `lines (max ...)`。
- 一个文件混杂多层(UI + 业务 + 数据访问)。
- 红线信号:**先拆分,再继续**(AGENTS.md)——命中即停新增。

## 步骤 (Steps)
1. **冻结**:停止往该文件加新代码;以当前测试为安全网(缺则先补 characterization test)。
2. **识别职责 (classify)**:逐块归类 → `UI / Service(用例) / Repository(数据) / DTO(数据形状) / Utils(纯函数)`。
3. **抽离 (extract)**:每类职责抽到独立文件;命名按领域,放到对应层目录(见 skill: clean-architecture)。
4. **删重复 (dedup)**:合并复制粘贴的逻辑为共享 util/service;消除重复定义。
5. **接线 (rewire)**:原文件改为瘦协调者或纯 re-export;修正所有 import,保持公共 API 不变。
6. **更新测试**:测试随职责迁移;为新抽出的单元补 unit test。
7. **复检**:`node harness/gate.mjs` + 全套测试,必须全绿。

## 输出 (Output)
- 新文件结构清单(`path → 职责`)。
- 每个新文件行数,均 ≤ 阈值。
- 测试结果(pass/fail)+ gate verdict。
- 一行变更摘要:拆出 N 个单元、删除 M 处重复。
