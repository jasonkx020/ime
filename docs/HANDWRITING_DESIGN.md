# 手写板输入设计规范

> 版本：1.1  
> 关联：[IME_ARCHITECTURE.md](IME_ARCHITECTURE.md) 3.7 节、[KEYBOARD_UI_DESIGN.md](KEYBOARD_UI_DESIGN.md) 第 12 节  
> 读者：UI 工程师、Rust `ime-handwriting` 实现者、模型集成工程师

---

## 1. 产品定位

手写板允许用户在 **InkCanvas** 上书写字形，经 **端侧识别为主、连写可选云端** 的流水线转为候选词并上屏。对标三星输入法手写模式：

- **工具栏入口** + **可切换 `InputScheme(handwriting)`**
- **单字模式**：一字一框，抬笔即识别
- **连写模式**：多字连续书写，低置信可走云（需隐私确认）

---

## 2. 笔迹采集规范

### 2.1 坐标与采样

| 项 | 规范 |
|----|------|
| 坐标 | 相对书写区归一化 `0.0..1.0`（含 canvas 宽高） |
| 采样率 | 建议 ≥ 60Hz；UI 侧可降采样至 30Hz 再提交 |
| 压感 | 可选；无压感设备填 `1.0` |
| 时间戳 | 单调递增 `t`（ms），用于笔迹分段 |

### 2.2 三端与桌面触摸 / 指针 API 映射

| 平台 | 采集 API |
|------|----------|
| Android | `MotionEvent` → `ACTION_DOWN/MOVE/UP` |
| iOS | `UITouch` in `UIInputViewController` |
| 鸿蒙 | ArkUI `TouchEvent` on InkCanvas |
| **Windows** | **Win32 `WM_POINTER*` / `WM_LBUTTON*`；Surface Pen `POINTER_PEN_INFO`** |
| **macOS** | **`NSEvent` 鼠标拖拽 / `NSTabletPoint` 压感笔** |
| **Linux** | **GTK `GdkEvent` / Qt `QTabletEvent`；Wayland `zwp_tablet_v2`** |

### 2.3 提交时机

- **抬笔（touch up）** 时组装 `StrokeBatch` 并 `PushStrokeBatch`
- **禁止** 每个 `MOVE` 点调用 `ime_hot_submit` 或 FFI
- 用户点击顶栏「识别」可手动触发 `RecognizeHandwriting`（连写模式）

### 2.4 StrokeBatch 结构

```text
StrokePoint { x: f32, y: f32, t: u64, pressure: f32 }
Stroke { points: StrokePoint[] }
StrokeBatch {
  editor_id: EditorId
  session_stroke_id: u64
  strokes: Stroke[]
  canvas_size: (w, h)
  writing_mode: SingleChar | Continuous
}
```

---

## 3. 识别流水线

```text
StrokeBatch
  → 预处理（平滑、重采样、归一化、去噪）
  → OnDeviceRecognizer.infer()
  → if confidence < 0.6 && Continuous && PrivacyLevel == Normal:
       previewPayload() → 用户确认
       CloudHwRecognizer.infer()
  → HandwritingResult { candidates[], confidence, used_cloud }
  → ImmSnapshot（CandBar）
```

| 阶段 | 延迟目标 |
|------|----------|
| 预处理 | ≤ 10ms |
| 端侧单字 | P95 ≤ 120ms |
| 端侧连写 | P95 ≤ 200ms |
| 云端连写 | P95 ≤ 800ms |

---

## 4. 端侧模型要求

| 项 | 要求 |
|----|------|
| 格式（M2.5 Android） | **PaddleOCR** 官方 [ppocr-sdk](https://github.com/PaddlePaddle/PaddleOCR/tree/main/deploy/ppocr-android)（ONNX Runtime **PP-OCRv6_tiny**） |
| 模型文件 | `det/inference.onnx` + `rec/inference.onnx` + `rec/inference.yml` |
| 输入 | 笔迹栅格化为白底黑笔 `Bitmap`（约 360px）后 OCR |
| 候选形态 | **仅汉字**；OCR 主结果 + CTC 备选字（池上限 200，CandBar 分页）；过滤拉丁/数字/其他语种 |
| 分发 | App `assets/models/ppocr/`；**不进 LangPack**；刷新：`python tools/fetch_ppocr_assets.py` |
| 体积 | tiny det+rec 合计约 ~6MB（可换 `PP-OCRv6_small`） |

模板匹配仅作 **OCR 失败时的 fallback / 单元测试**。

---

## 5. 云端连写识别

### 5.1 触发条件（须同时满足）

- `writing_mode == Continuous`
- 端侧 `confidence < threshold`（默认 0.6）
- `PrivacyLevel == Normal`
- 用户已确认 `RedactedPreview`

### 5.2 上云载荷

- **优先**：笔迹向量 `StrokeBatch`（非截图）
- **可选**：脱敏后的书写区位图（用户勾选）

### 5.3 隐私

- 笔迹 **不上传** 审计日志
- `Sensitive`：仅端侧，不触发云
- `ForbiddenCloud`：可禁用手写板或强制端侧

---

## 6. 与 InputEngine 边界

| 项 | 规则 |
|----|------|
| 组词 | 手写不走 `InputEngine.feed()` |
| 候选 | `source = Handwriting`，写入 `ImmSnapshot` |
| 上屏 | `SelectCandidate` → `UiCommand.Commit` |
| 清空 | 选词后 `HandwritingService.clear` 当前格 |
| 拼音 composing | 手写模式 **隐藏** 拼音行，CandBar 仅显示识别候选；**选词后离线词表联想**（优先下一字，后缀 ≤2 字，端侧字 bigram 重排，非云 LLM） |

---

## 7. UI 交互摘要

见 [KEYBOARD_UI_DESIGN.md](KEYBOARD_UI_DESIGN.md) 第 12 节：

- 顶栏：单字/连写、撤销、清空
- 主区：InkCanvas（浅底 + 可选细网格）
- 底栏：返回键盘
- CandBar：保持可见，展示识别候选

---

## 8. 布局 YAML 示例

见 [LANGPACK_AUTHORING.md](LANGPACK_AUTHORING.md) `layout_handwriting_pad.yaml`。

---

## 9. M2.5 验收清单

> 端侧识别 = **PP-OCRv6_tiny（ONNX / 官方 ppocr-sdk）**（Android）。候选 = **仅汉字**（OCR + CTC 备选，池 ≤200，分页）；独立形近字库后续另议。云端仍为 stub，但确认 UI 须闭环。

- [x] 工具栏「手写」打开 HandwritingPad ≤ 100ms（原生切换）
- [x] 单字模式抬笔 debounce（450ms）后识别
- [x] CandBar 展示 OCR 结果（整句/分框/分字），`source=Handwriting`
- [x] 选词 Commit 上屏并清空书写区
- [x] 选词后离线词表联想（CandBar 展示后缀，端侧字 bigram 重排，可继续点选）
- [x] 撤销/清空有效
- [x] `SwitchScheme(handwriting)` 与工具栏入口行为一致（core）
- [x] Session 切换 wipe 笔迹缓冲
- [x] 密码框 `ForbiddenCloud`：禁开手写 + 工具栏灰显
- [x] 连写切分/OCR + 低置信云确认对话框（云结果 stub）
- [x] 画布网格 + 压感线宽 +「识别中…」

### Android 手测建议

1. 普通文本框 → 手写 → 写「你/好/我」→ CandBar 合理 → 选词上屏 → 联想后缀（如「好」）可点选  
2. 多笔写「你」中间不停顿 → 不应只出「一」  
3. 连写两字 →「识别」→ 短语候选；低置信弹云确认  
4. 密码框 →「手写」灰显不可点  
5. 返回键盘后拼音正常；拼音选词后同样出现离线联想

---

*文档结束*
