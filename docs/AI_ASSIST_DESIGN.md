# AI 场景助手设计规范

> 版本：1.0  
> 关联：[IME_ARCHITECTURE.md](IME_ARCHITECTURE.md) 3.6 节、[KEYBOARD_UI_DESIGN.md](KEYBOARD_UI_DESIGN.md) 第 10 节  
> 读者：产品、Prompt 工程师、Rust `ime-ai` 实现者

---

## 1. 产品定位

**AI 场景助手**帮助用户在沟通场景中更快产出得体、有吸引力的文字，覆盖：

| 场景 | 用户痛点 | 助手价值 |
|------|----------|----------|
| 商务谈判 | 措辞过硬易激化、过软失立场 | 立场清晰、留余地、促合作 |
| 客户跟进 | 不知道下一句说什么 | 专业有温度、推动下一步 |
| 恋爱聊天 | 怕尬聊、怕冒犯 | 真诚幽默、把握分寸 |
| 发朋友圈 | 想吸睛又怕太刻意 | 有记忆点、自然不尬 |
| 职场沟通 | 边界不清、语气不当 | 简洁礼貌、专业边界 |
| 道歉挽回 | 说不清、显敷衍 | 共情 + 承担 + 行动 |

**边界**：助手生成的是**建议草稿**，用户必须核对后发送；不替代用户决策。

---

## 2. 上下文策略（隐私优先）

### 2.1 仅显式输入

| 字段 | 来源 | 说明 |
|------|------|------|
| `selection_text` | 宿主选区或 composing | 可选 |
| `peer_message` | 用户粘贴 | 「对方说了什么」 |
| `background_note` | 用户填写 | 关系、禁忌、谈判背景 |
| `user_intent` | 用户填写 | 想达成什么 |
| `target_length` | UI 选择 | 短/中/长 |

### 2.2 明确禁止

- 自动读取剪贴板
- 读取聊天 App 历史消息
- 无障碍 / 截屏 OCR 抓上下文
- 上传 Session composing 明文到审计日志

### 2.3 上云前预览

`previewPayload()` 向用户展示脱敏后的字段，例如：

```text
peer_message: 你们报价太高了***
background_note: 首次合作，预算有限***
将使用云端模型生成
```

用户确认后才调用 `CloudLlmClient`。

---

## 3. 能力与场景矩阵

### 3.1 AiMode × AiScene 推荐组合

| AiMode | 推荐场景 | 说明 |
|--------|----------|------|
| `SmartReply` | customer_followup, work_chat, dating | 需要对方消息 |
| `HighEqReply` | apology, negotiation, dating | 强调语气与关系维护 |
| `Compose` | social_moment, negotiation | 从零撰写 |
| `Rewrite` | 任意 | 需 `selection_text` |
| `Polish` | 任意 | 兼容润色 |

### 3.2 内置场景默认语气

| scene_id | 默认 AiTone | 字数倾向 |
|----------|-------------|----------|
| negotiation | Assertive | Medium |
| customer_followup | Professional | Medium |
| dating | Warm | Short |
| social_moment | Humorous | Medium |
| work_chat | Concise | Short |
| apology | HighEq | Medium |
| custom | Warm | Medium |

---

## 4. 端云分工

```text
TaskReq
  → PrivacyScrubber          // 手机/身份证/银行卡/邮箱等脱敏
  → SceneRouter
      ├─ [本地] 模板 + 槽位填充     // 简单问候、节日祝福、短拒绝
      └─ [云端] LLM 多候选生成      // 谈判、恋爱、长文案
  → AiOutput { variants[3] }
```

| 路径 | 适用 | 延迟 | PrivacyLevel |
|------|------|------|--------------|
| 端侧模板 | 固定句式、槽位短填 | ≤800ms | Sensitive 可用 |
| 云端 LLM | 复杂推理、高情商、多风格 | ≤2.5s 首条 | 仅 Normal |

**路由规则**：

- `peer_message` + `background_note` 总字数 &lt; 80 且场景有本地模板 → 优先端侧
- `HighEqReply` / `negotiation` / `dating` → 默认云端（若允许）
- 无网 → 仅端侧；端侧无模板则 Toast「当前场景需联网」

---

## 5. Prompt 模板规范

### 5.1 结构

每个场景在 AiPack 或内置 `aipacks/default/scenes/{scene_id}.yaml`：

```yaml
scene_id: dating
display_name: 恋爱聊天
default_tone: Warm
max_output_chars: 120
allow_cloud: true

system_prompt: |
  你是恋爱沟通助手。帮助用户回复对方消息，语气真诚、自然、不过度热情。
  禁止：油腻、PUA、道德绑架、过度承诺。
  输出：恰好 3 条不同风格的回复，每条一行，不要编号。

user_template: |
  【对方消息】
  {{peer_message}}

  【关系背景】
  {{background_note}}

  【用户目的】
  {{user_intent}}

  【语气】{{tone}}
  【长度】{{target_length}}

examples:
  - peer_message: "在干嘛呢"
    good: "刚忙完一阵，正想你呢。你呢？"
    bad: "想我想得睡不着了吧"   # 油腻，禁止
```

### 5.2 占位符

| 占位符 | 来源 |
|--------|------|
| `{{peer_message}}` | AiContextBundle |
| `{{background_note}}` | AiContextBundle |
| `{{user_intent}}` | AiContextBundle |
| `{{selection_text}}` | AiContextBundle |
| `{{tone}}` | AiTone 显示名 |
| `{{target_length}}` | Short/Medium/Long 说明 |

### 5.3 输出解析

云端返回自由文本时，端侧按行拆分为 3 条 `AiSuggestionVariant`；解析失败则整段作为单条 + 重试一次。

标签 `tags` 由场景规则自动打标，例如：

- 高情商 → `["留余地","共情"]`
- 谈判 → `["立场清晰","可让步"]`
- 朋友圈 → `["吸睛","轻松"]`

---

## 6. 场景示例

### 6.1 商务谈判

**输入**

```text
peer_message: 你们这个价格我们接受不了，竞品低 15%。
background_note: 我方是供应商，首次合作，可小幅让利但需保毛利。
user_intent: 争取继续谈，不直接降价到竞品水平。
tone: Assertive
```

**期望输出风格（示意）**

1. 「理解您对成本的考量。我们愿意在首批订单上给出更有诚意的方案，同时保证交付与售后标准，方便明天对一下具体量级吗？」
2. 「竞品价格我们也有所了解。我们更希望在质量与响应速度上体现差异，看看是否能把合作先做小范围试点？」
3. 「15% 的差距确实不小。若您方便透露核心诉求，我们内部评估后给您一版可执行的替代方案。」

### 6.2 恋爱聊天

**输入**

```text
peer_message: 今天好累啊不想说话
background_note: 刚认识两周，见过两次面
user_intent: 表达关心但不施压
tone: Warm
```

### 6.3 发朋友圈

**输入**

```text
mode: Compose
scene: social_moment
background_note: 周末爬山，天气很好，想低调晒一下但不凡尔赛
user_intent: 让朋友觉得有趣、想点赞
tone: Humorous
```

---

## 7. AiPack 场景包格式

与 LangPack 解耦，轻量 OTA：

```text
.imeaipack/
  manifest.fb
  scenes/
    dating.yaml
    negotiation.yaml
    ...
  signature
```

**AiPackManifest（概念）**

```text
AiPackManifest {
  id: string
  version: uint32
  min_host_version: string
  display_name: string
  scenes: AiSceneEntry[]
}

AiSceneEntry {
  scene_id: string
  prompt_path: string
  default_tone: string
  allow_cloud: bool
}
```

- **不含模型权重**
- 主 App `install`；Extension 只读（`filesDir` / App Group）
- 验签策略同 LangPack

---

## 8. 合规与安全

| 项 | 要求 |
|----|------|
| 免责声明 | 每条 `AiOutput.disclaimer`：「AI 生成，请核对后发送」 |
| 敏感场景 | 密码框 `ForbiddenCloud`：面板 Disabled |
| 频控 | 云生成 10 次/分钟 |
| 内容安全 | 云端侧接入内容审核 API；违规返回空 + Toast |
| 未成年人 | 可配置禁用 dating 场景 |
| 数据留存 | 云端不持久化用户原文（请求级 ephemeral） |

---

## 9. 与 ime-ai crate 映射

```text
ime-ai/
  scrubber/       PrivacyScrubber
  router/         SceneRouter（本地 vs 云）
  local/          LocalTemplateEngine
  cloud/          CloudLlmClient
  assist/         AiAssistService::suggest
  polish/         AiAssistService::polish（兼容）
  pack/           AiPackLoader（M5）
```

---

## 10. 验收清单（M4.5）

- [ ] 谈判 / 恋爱 / 朋友圈 三场景可生成 3 条候选
- [ ] 用户仅通过面板提供上下文，无自动剪贴板
- [ ] 上云前展示 RedactedPreview，用户可取消
- [ ] 密码框拒绝云生成
- [ ] 切换输入框后旧任务结果丢弃，上下文 wipe
- [ ] 点选候选可 Commit 上屏
- [ ] 面板打开 ≤100ms，首条候选 P95 ≤2.5s（云）

---

*文档结束*
