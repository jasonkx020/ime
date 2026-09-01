# 键盘界面设计规范

> 版本：1.3  
> 关联：[IME_ARCHITECTURE.md](IME_ARCHITECTURE.md) Presentation 层、ThemeTokens、LangPack、[AI_ASSIST_DESIGN.md](AI_ASSIST_DESIGN.md)、[HANDWRITING_DESIGN.md](HANDWRITING_DESIGN.md)  
> **Demo 视觉与交互基准**：三星输入法 Samsung Keyboard（One UI 6.x 中文键盘）

---

## 1. 设计目标

- **跟手**：按键按下反馈 ≤ 80ms；候选栏与键盘分区清晰。
- **可读**：候选、拼音串、按键标签在浅色/深色皮肤下均满足对比度。
- **可扩展**：布局与颜色来自 `ThemeTokens` / LangPack，支持 OTA 换肤与多语言键盘。
- **安全**：UI 不解析业务 JSON；仅渲染 Session 下发的 `ImmSnapshot`。

---

## 2. 整体结构（自上而下）

> 内置 Demo / Canvas 预览默认采用 **三星输入法** 分区与键位习惯（见第 11 节）。

```text
┌─────────────────────────────────────────────┐
│  预测候选栏 CandBar       高 44–52dp         │
│  [剪贴板建议芯片]  候选1 候选2 候选3 …      │
├─────────────────────────────────────────────┤
│  工具栏 Toolbar（图标行）  高 40dp            │
│  ⚙ 翻译 剪贴板 语音 表情 手写 AI写作              │
├─────────────────────────────────────────────┤
│  键盘区 KeyView / 扩展面板    高 ~220dp      │
│  Q W E R T Y U I O P                        │
│   A S D F G H J K L                         │
│  分词 Z X C V B N M 删除                    │
│  !#1  🌐   ,    [ 空格·语音 ]    .   搜索   │
├─────────────────────────────────────────────┤
│  底部拖拽条 / 单手模式偏移区                  │
└─────────────────────────────────────────────┘
```

**总高度（Android/iOS 常见）**：约 260–290dp + 系统安全区。

---

## 3. 分区规格

### 3.1 候选栏 CandBar

| 元素 | 规格 | 说明 |
|------|------|------|
| 高度 | 48–56dp | 含拼音行时可取上限 |
| 语言标签 | Pill 样式 | 显示当前 LangPack 名称 |
| 拼音串 | 12–13sp，accent 色 | `composing.text`，可配置隐藏 |
| 候选词 | 15sp，水平滚动 | 首屏 5–9 条；选中项加粗 + 描边 |
| 候选间距 | 6dp | 左右 padding 10dp |

**状态**

- 无候选：仅显示拼音串或空态占位。
- AI 候选：同排靠后，样式与本地词一致，score 低不抢首屏。

### 3.2 工具栏 Toolbar

| 元素 | 规格 |
|------|------|
| 高度 | 36dp |
| 图标/文字 | 12sp，secondary 色 |
| 点击区域 | 最小 44×36dp |

入口（三星对标顺序）：**设置**、**翻译**、**剪贴板**、**语音**、**表情**、**手写**、**AI写作**。

| 三星功能 | 本方案映射 |
|----------|-----------|
| 手写输入 | HandwritingPad（3.7 节） |
| 聊天助手 / 写作助手 | AiAssistPanel（3.6 节） |
| 剪贴板 | ExtensionHost Clipboard |
| 翻译 | AiAssist / 冷路径翻译 |
| 语音听写 | SpeechHost |
| 表情/贴纸 | ExtensionHost Emoji |
| 文本编辑 | 光标移动面板（P3，可选） |
| 键盘设置 | 语言包商店 + 皮肤 + 布局 |

**AI助手按钮状态**

| 状态 | 表现 |
|------|------|
| Normal | 图标 + 「AI」文字，`toolbar_text` 色 |
| Active | 面板打开时高亮 `accent.primary` |
| Disabled | `ForbiddenCloud`（密码框等）：`fill.tertiary` + 点击 Toast |
| Loading | 生成中图标旋转；仍可 `DismissAiAssist` 关闭 |

### 3.3 键盘区 KeyView

| 元素 | 规格 |
|------|------|
| 按键高度 | 44dp |
| 行间距 | 6dp |
| 圆角 | **12dp**（Demo 三星风格；皮肤可配置 8–12dp） |
| 外边距 | 12dp（紧凑模式 8dp） |
| 标准键宽 | 等分 10 列网格 |

**特殊键宽度（相对标准键）**

| 键 | 宽度倍率 |
|----|----------|
| 分词 / 删除 | 1.35× |
| !#1 / 语言 | 1.1× |
| 逗号 / 句号 | 0.85× |
| 空格（含语音） | 3.6× |
| 搜索/换行 | 1.4× |

---

## 4. 按键视觉状态

| 状态 | 背景 Token | 边框 | 说明 |
|------|-----------|------|------|
| Normal | `bg.elevated` | `stroke.secondary` | 字母/数字键 |
| Pressed | `fill.quaternary` | `accent.primary` | 按下 80–120ms |
| Utility | `fill.secondary` | `stroke.secondary` | 分词、删除、123、中英 |
| Accent | `accent.control` | `accent.primary` | 换行/搜索；文字 `text.onAccent` |
| Disabled | `fill.tertiary` | `stroke.tertiary` | 密码框禁云时 AI 助手等 |

**禁止**：过重阴影、渐变；Demo 三星浅色主题允许 **极轻** 按键分隔（1px `#D0D3D8`）。

---

## 11. Demo 基准：三星输入法（Samsung Keyboard）

内置交互预览与默认皮肤对标 **Samsung Keyboard One UI 6.x 中文全拼键盘**，仅借鉴交互与布局，不复制三星商标与专有资源。

### 11.1 视觉 Token（浅色默认）

| Token | 三星参考值 | 用途 |
|-------|-----------|------|
| `keyboard_bg` | `#E8EAED` | 键盘底板 |
| `key_normal` | `#FFFFFF` | 字母键 |
| `key_utility` | `#DDE0E4` | 功能键 !#1、语言 |
| `key_accent` | `#1A73E8` | 搜索/发送（三星蓝） |
| `key_pressed` | `#C8CCD2` | 按下态 |
| `cand_chip` | `#FFFFFF` + 细边框 | 预测词 Pill |
| `clipboard_chip` | `#E3F2FD` | 剪贴板建议芯片 |

### 11.2 功能对照

| 三星能力 | Demo 是否实现 | 说明 |
|----------|--------------|------|
| 预测候选横滑 | ✓ | CandBar 首屏 5–9 条 |
| 剪贴板建议芯片 | ✓ | 候选栏左侧一键粘贴 |
| 工具栏图标行 | ✓ | 设置/翻译/剪贴板/语音/表情/AI |
| 底部 `!#1` 符号页 | ✓ | 切换符号键盘子布局 |
| `,` `.` 独立键 | ✓ | 空格两侧 |
| 空格长按语音 | 示意 | 空格键标注麦克风 |
| 语言/布局切换 | ✓ | 地球键循环 LangPack |
| 表情面板 | ✓ | 替换 KeyView 区域 |
| 剪贴板历史面板 | ✓ | 替换 KeyView 区域 |
| AI 写作助手 | ✓ | AiAssistPanel |
| 手写板 | ✓ | HandwritingPad |
| 单手模式 | ✓ | 键盘整体水平偏移 |
| 浮动/分屏键盘 | 文档预留 | 架构支持，Demo 未做 |
| 滑行输入 Swipe | 文档预留 | 引擎层，Demo 未做 |

### 11.3 与架构映射

- 工具栏入口 → `UserAction.OpenExtension` / `OpenAiAssist`
- 剪贴板芯片 → `ExtensionHost.Clipboard.suggest()`
- 符号页 `!#1` → `SwitchLayout(layout_symbol)`
- AI 写作 → `OpenAiAssist`（3.6 节）
- 手写 → `OpenHandwriting` / `SwitchScheme(handwriting)`（3.7 节）

---

## 5. ThemeTokens 与换肤

UI 只消费已解析的 `ThemeTokens`（见架构 3.2），不解析皮肤 JSON。

```text
ThemeTokens {
  colors: {
    keyboard_bg,        // → bg.editor
    key_normal,           // → bg.elevated
    key_pressed,          // → fill.quaternary
    key_utility,          // → fill.secondary
    key_accent,           // → accent.control
    cand_text,            // → text.primary
    cand_selected_bg,     // → fill.tertiary
    cand_selected_border, // → accent.primary
    composing_text,       // → accent.primary
    toolbar_text,         // → text.secondary
  }
  key_radius: 8
  cand_font_size: 15
  key_font_size: 16
}
```

换肤 **不重置** composing；换 LangPack / layout 时 `ReloadKeyboard`。

---

## 6. 多语言布局（LangPack）

| LangPack | 布局 ID | 说明 |
|----------|---------|------|
| zh | layout_26_pinyin | 26 键全拼（默认） |
| en | layout_qwerty | 英文 QWERTY |
| vi | layout_telex | Telex 越南语 |
| th | layout_thai | 泰语辅音/元音分区 |

地球键 / 语言键：循环 `PluginHost.listEnabled()`；切换时 `Scheduler.switchLang` + 候选栏更新语言 Pill。

---

## 7. 动效与反馈

| 交互 | 动效 |
|------|------|
| 按键按下 | 背景色 80ms 过渡；可选轻触震动（平台） |
| 候选选中 | 150ms 背景/边框过渡 |
| 换语言 | 键盘 200ms 淡入重绘；composing 清空 |
| 语言包下载 | 工具栏外独立进度，不遮挡 KeyView |

---

## 8. 平台差异

| 项 | Android | iOS | 鸿蒙 | Windows | macOS | Linux |
|----|---------|-----|------|---------|-------|-------|
| 底部安全区 | 导航栏 inset | Home Indicator 12–20dp | 手势条 inset | 任务栏避让（可选） | Dock 避让（可选） | 面板内嵌，无系统 inset |
| 键盘高度 | 可调 `inputView` | Extension 高度受限 | Extension 高度受限 | 自绘 HWND / WinUI 浮层，建议 260–320px | IMK 面板 260–300pt | IBus/Fcitx 内嵌面板 |
| 深色模式 | 跟随系统 / 皮肤 | 跟随系统 / 皮肤 | 跟随系统 / 皮肤 | `UISettings` / 皮肤 | `NSAppearance` / 皮肤 | GTK 主题 / `prefers-color-scheme` |
| 物理键盘 | 部分场景 | 外接键盘 | 外接键盘 | **主路径** | **主路径** | **主路径** |
| 手写输入 | 触摸 InkCanvas | 触摸 | 触摸 | 鼠标 / 触控笔 / Surface Pen | 触控板 / 笔 | 鼠标 / 数位板 |
| 候选展示 | CandBar 内嵌 | CandBar 内嵌 | CandBar 内嵌 | CandBar 或 TSF 系统候选窗 | IMKCandidates 或自绘 | IBus lookup / Fcitx5 候选窗 |

---

## 9. 交互预览

可在 Cursor 中打开交互预览 Canvas（**三星输入法风格 Demo**）：

[ime-keyboard-ui.canvas.tsx](/Users/Administrator/.cursor/projects/c-Users-Administrator-Projects-ime-design/canvases/ime-keyboard-ui.canvas.tsx)

支持：三星浅色主题、预测候选、剪贴板芯片、工具栏六图标、符号键盘、`!#1` 底行、表情/剪贴板/AI 面板切换、单手模式、按键按下态。

---

## 10. AI 场景助手面板（AiAssistPanel）

> 架构见 [IME_ARCHITECTURE.md](IME_ARCHITECTURE.md) 3.6 节；场景与 Prompt 见 [AI_ASSIST_DESIGN.md](AI_ASSIST_DESIGN.md)。

### 10.1 呈现方式

- 点击工具栏 **「AI助手」** → `OpenAiAssist` → **替换 KeyView 区域**（CandBar + Toolbar 保留）
- 面板高度 **240–280dp**；总键盘高度临时增加，关闭后恢复
- 动画：面板 200ms 自底滑入；关闭 150ms 滑出

### 10.2 布局

```text
┌─────────────────────────────────────────┐
│ [回复][撰写][改写]    场景▼   语气▼      │  高 40dp
├─────────────────────────────────────────┤
│ 对方消息（粘贴）                         │
│ ┌─────────────────────────────────────┐ │
│ │ 多行输入，最小 2 行，最大 4 行       │ │  高 72–96dp
│ └─────────────────────────────────────┘ │
│ 补充背景（选填）                         │
│ ┌─────────────────────────────────────┐ │
│ │ 多行，最大 3 行                      │ │  高 56–72dp
│ └─────────────────────────────────────┘ │
│ 想达成（选填） [单行输入____________]    │  高 36dp
├─────────────────────────────────────────┤
│ [生成建议]              剩余 3/10 次    │  高 44dp
├─────────────────────────────────────────┤
│ ┌──────────┐ ┌──────────┐ ┌──────────┐ │
│ │ 候选 1    │ │ 候选 2    │ │ 候选 3    │ │  横滑卡片
│ │ 标签…    │ │          │ │          │ │  高 88–104dp
│ └──────────┘ └──────────┘ └──────────┘ │
│ AI 生成，请核对后发送                    │  11sp 免责声明
├─────────────────────────────────────────┤
│ [关闭]                                   │  高 36dp
└─────────────────────────────────────────┘
```

### 10.3 Tab 与模式映射

| Tab | AiMode | 必填字段 |
|-----|--------|----------|
| 回复 | SmartReply / HighEqReply | `peer_message` |
| 撰写 | Compose | `user_intent` 或 `background_note` 至少其一 |
| 改写 | Rewrite / Polish | `selection_text` 或面板内草稿 |

### 10.4 场景与语气选择器

- **场景**：下拉 / BottomSheet，列表来自内置 + AiPack OTA
- **语气**：水平 Pill，`Professional | Warm | Humorous | Concise | 高情商 | 坚定`
- 场景切换时自动更新默认语气（见 AI_ASSIST_DESIGN 3.2）

### 10.5 候选卡片

| 元素 | 规格 |
|------|------|
| 卡片宽 | 屏幕宽 78%，间距 8dp |
| 正文 | 14sp，`cand_text` 色，最多 4 行截断 |
| 标签 | 11sp Pill，`cand_selected_bg` 背景 |
| 选中态 | `cand_selected_border` 描边 2dp |

点击卡片 → 底部弹出 **预览条**：「插入」|「替换选区」|「复制」。

### 10.6 状态机

| 状态 | UI |
|------|-----|
| Empty | 仅输入区 + 生成按钮 |
| Generating | 骨架屏 3 卡；按钮变「取消」 |
| PreviewConfirm | 上云前脱敏预览 Dialog |
| Ready | 展示 3 候选 |
| Error | Toast + 保留输入内容 |
| Disabled | 整面板灰显 + 说明文案 |

### 10.7 隐私与禁用

- `PrivacyLevel = ForbiddenCloud`：工具栏按钮 Disabled，不打开面板
- `PrivacyLevel = Sensitive`：可打开面板，但「生成」仅走端侧模板；无模板时 Toast
- 关闭面板或 Session 切换：**立即清空** 面板内输入与候选（不保留在 UI 层）

### 10.8 ThemeTokens 扩展（可选）

```text
ai_panel_bg          // 面板背景，默认同 keyboard_bg
ai_card_bg           // 候选卡片，默认同 bg.elevated
ai_card_border       // 选中描边，默认同 accent.primary
ai_tag_bg            // 标签 Pill
ai_disclaimer_text   // 免责声明，11sp secondary
```

---

## 12. 手写板（HandwritingPad）

> 架构见 [IME_ARCHITECTURE.md](IME_ARCHITECTURE.md) 3.7 节；笔迹与识别见 [HANDWRITING_DESIGN.md](HANDWRITING_DESIGN.md)。

### 12.1 呈现方式

- 工具栏 **「手写」** 或 `SwitchScheme(handwriting)` → 替换 KeyView 为 HandwritingPad
- **CandBar 保持可见**：展示识别候选；隐藏拼音 composing 行
- 高度 **220–260dp**；底栏「返回键盘」恢复 `layout_26_pinyin`

### 12.2 布局

```text
┌─────────────────────────────────────────┐
│ [单字][连写]     识别中…    [撤销][清空] │  顶栏 36dp
├─────────────────────────────────────────┤
│                                         │
│         InkCanvas 手写书写区             │  ~180dp
│         浅底 #FAFAFA + 可选细网格        │
│                                         │
├─────────────────────────────────────────┤
│ [返回键盘]                               │  底栏 36dp
└─────────────────────────────────────────┘
```

### 12.3 交互

| 操作 | 行为 |
|------|------|
| 落笔移动 | UI 本地绘制笔迹，不调用 FFI |
| 抬笔 | `RecognizeHandwriting` → P1 识别 → 更新 CandBar |
| 单字/连写 | 切换 `WritingMode`；连写低置信可走云（Normal） |
| 撤销 | `UndoHandwriting` 删除上一笔 |
| 清空 | `ClearHandwriting` |
| 选候选 | `SelectCandidate` → Commit + 清空书写区 |

### 12.4 状态

| 状态 | UI |
|------|-----|
| Idle | 空白画布 + 顶栏 |
| Drawing | 实时墨迹 |
| Recognizing | 顶栏「识别中…」+ 禁用重复提交 |
| Ready | CandBar 显示候选 |
| Disabled | `ForbiddenCloud`：工具栏按钮 Disabled |

### 12.5 ThemeTokens 扩展

```text
hw_canvas_bg: #FAFAFA
hw_ink_color: #202124
hw_grid_line: #E8EAED
hw_toolbar_bg: 同 keyboard_bg
```

---

*文档结束*
