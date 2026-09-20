# 行业话术 LLM 与知识库

> 关联：[AI_ASSIST_DESIGN.md](AI_ASSIST_DESIGN.md)（通用 AI，入口独立）  
> FeatureBar「话术」≠「AI」

## 1. 边界

| | AI | 话术 |
|--|----|------|
| 目标 | 通用回复 / 润色 / 翻译 | 行业客服与销售话术 |
| 场景 | 关系/意图 chips | **内置行业或自定义 scene_profile** |
| 内容 | 无行业 pack | ContentPack deck + 可选知识库 |
| Router | `LlmAssistRouter` | `PhraseLlmRouter` |

共用 BYOK Key；Prompt 与入口分离。

## 2. scene_profile（必带）

每次 LLM 调用必须注入：

```text
【使用场景】{display_name}
【场景说明】{hint}
【禁止】与场景无关的话术；…{taboos}
```

- **内置**：`type=builtin`，来自 ContentPack `prompts/system.txt` + `active_industry`
- **自定义**：`type=custom`，用户填写名称 / 说明 / 禁语，覆盖生成时的场景锚点

无知识库时仍可生成：仅依赖 scene_profile + 场景卡模板。

## 3. 知识库（可选）

- 本地 `filesDir/phrase_kb/{bucket}/`，不上传
- 角色：`rule`（硬约束）/ `ref`（参考）/ `product`
- 生成时：rule 进 system；ref 检索 Top-K 进 user
- 未导入 KB：**不禁用**话术与 AI 优化

## 4. UX

1. 点场景卡 → 上屏原文  
2. AI 优化 → 2～3 变体点选上屏  
3. 面板/主 App 可切换内置行业或编辑自定义场景  

## 5. 验收

- [ ] 启用电商包后话术可一键上屏  
- [ ] 自定义「教培」后 AI 优化不出现电商「亲/退款」口癖（在合理模型行为下）  
- [ ] 无 KB、有 Key 可 AI 优化  
- [ ] 有 KB 时约束/参考进入 Prompt  
- [ ] 通用 AI 面板行为不变  
