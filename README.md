# ime-design



跨平台输入法（IME）架构与能力设计文档仓库。



## 文档



| 文档 | 说明 |

|------|------|

| [docs/KEYBOARD_UI_DESIGN.md](docs/KEYBOARD_UI_DESIGN.md) | **键盘界面设计规范**：三星输入法 Demo 基准、HandwritingPad、AiAssistPanel、桌面差异 |

| [docs/IME_ARCHITECTURE.md](docs/IME_ARCHITECTURE.md) | **完整设计方案**（v1.9）：六端架构、Rust 核心、语言包 OTA、AI 场景助手、手写板 |

| [docs/HANDWRITING_DESIGN.md](docs/HANDWRITING_DESIGN.md) | **手写板设计规范**：笔迹格式、端云识别、隐私门禁、性能指标、M2.5 验收 |

| [docs/AI_ASSIST_DESIGN.md](docs/AI_ASSIST_DESIGN.md) | **AI 场景助手规范**：智能回复、高情商话术、场景 Prompt、AiPack |

| [docs/LANGPACK_AUTHORING.md](docs/LANGPACK_AUTHORING.md) | **语言包创作规范**：源格式（YAML/TOML/TSV）、词库 MMAP、布局、工具链 |

| [docs/RUST_PLATFORM_INTEGRATION.md](docs/RUST_PLATFORM_INTEGRATION.md) | **Rust 六端对接**：C ABI、移动三端（JNI / xcframework / NAPI）+ 桌面三端（TSF / IMK / IBus·Fcitx5） |

| [docs/SOURCE_NAMING_CONVENTIONS.md](docs/SOURCE_NAMING_CONVENTIONS.md) | **源代码命名规范**：`yc` 前缀、六端命名、FFI、迁移对照 |



## 技术约定



- **核心语言**：**Rust**（[`yc-core/`](yc-core/) workspace，经 `yc-ffi` + C ABI 供各平台胶水调用）

- **移动平台**：Android `InputMethodService` + JNI；iOS `UIInputViewController` + Swift；鸿蒙 `InputMethodExtensionAbility` + NAPI

- **桌面平台**：Windows **TSF** + C++；macOS **InputMethodKit** + Swift；Linux **IBus / Fcitx5** + C++

- **热路径**：C ABI / 定长结构（`cbindgen` 生成头文件，禁止 Protobuf）

- **冷路径**：FlatBuffers + Tokio 异步 IO

- **词库**：MMAP(`memmap2`) 热查询；SQLite(`rusqlite`) 仅元数据与异步落盘

- **语言包 OTA**：`.imepack` 远程下载、验签、enable/disable，无需更新 App（见文档 3.5 节）

- **AI 场景助手**：端云混合、用户显式上下文、上云前脱敏预览（见文档 3.6 节）

- **手写板输入**：端侧识别为主、连写可选云端；抬笔提交 `StrokeBatch`（见文档 3.7 节）

- **安全基线**：**一输入框一个 Session**（见文档 1.5 节）；敏感数据 `zeroize` 擦除



## 阅读建议



1. 先读 **第 2 章 总体架构** 了解分层与 **2.4 Rust 技术栈**；落地开发读 **[RUST_PLATFORM_INTEGRATION.md](docs/RUST_PLATFORM_INTEGRATION.md)**（第 4–9 节按平台选读）。

2. 读 **第 1.5 节 Session 安全模型** 理解输入框隔离与隐私边界。

3. 读 **第 3.5 节 语言包 OTA** 与 **[LANGPACK_AUTHORING.md](docs/LANGPACK_AUTHORING.md)**；语言包作者必读创作规范。

4. 读 **第 3.6 节 AI 场景助手** 与 **[AI_ASSIST_DESIGN.md](docs/AI_ASSIST_DESIGN.md)** 理解智能回复与高情商话术设计。

5. 读 **第 3.7 节 手写输入** 与 **[HANDWRITING_DESIGN.md](docs/HANDWRITING_DESIGN.md)** 理解笔迹采集、端云识别与隐私边界。

6. 读 **[KEYBOARD_UI_DESIGN.md](docs/KEYBOARD_UI_DESIGN.md)** 第 8 节（六端 UI 差异）、第 11–12 节（三星 Demo / HandwritingPad）与第 10 节 AiAssistPanel。

7. **第 5 章 模块接口** 与 **第 6 章 时序图**（含 6.10 手写识别）供开发与评审对照。

8. **第 9 章 仓库模块边界** 与 **第 10 章 里程碑**（含 M2.5 手写、**M5.5 桌面 MVP**）为迭代计划。

9. 落地实现前读 **[SOURCE_NAMING_CONVENTIONS.md](docs/SOURCE_NAMING_CONVENTIONS.md)**：`yc` 前缀替换遗留 `ime` 命名。



## 版本历史



| 版本 | 变更 |

|------|------|

| 1.0 | 初始完整设计方案 |

| 1.1 | Session 安全隔离：SessionManager、EditorFingerprint、一框一会话 |

| 1.2 | 核心模块确定为 Rust：ime-ffi、Cargo workspace、cbindgen、Tokio 冷路径 |

| 1.3 | 语言包 OTA：PluginHost、LangPack、enable/disable、Catalog CDN、附录 C |

| 1.4 | 语言包创作规范：源格式、MMAP 词库、布局 YAML、ime-tools 工具链 |

| 1.5 | Rust 三端对接：C ABI、Android JNI、iOS xcframework、鸿蒙 NAPI、M0 教程 |

| 1.6 | 鸿蒙 HarmonyOS NEXT：InputMethodExtensionAbility、NAPI、filesDir 语言包 |

| 1.7 | AI 场景助手：智能回复、高情商话术、AiAssistPanel、AiPack、附录 D |

| 1.8 | 手写板输入：HandwritingService、HandwritingPad UI、端云识别、附录 E、M2.5；Demo 对齐三星 |

| 1.9 | **桌面三端**：Windows TSF、macOS InputMethodKit、Linux IBus/Fcitx5；六端复用 ime-ffi；M5.5 里程碑 |

| 1.10 | **源代码命名规范**：`yc` 前缀替换 `ime`；[SOURCE_NAMING_CONVENTIONS.md](docs/SOURCE_NAMING_CONVENTIONS.md) |



## 构建 yc-core（Rust 核心）

需安装 [Rust](https://rustup.rs/) 工具链。

```bash
cd yc-core
cargo test --workspace
cargo build -p yc-ffi --release
cargo run -p yc-cli
```

生成 C 头文件（可选，需 `cargo install cbindgen`）：

```powershell
cd yc-core
.\scripts\gen-header.ps1
```

产物：`libyc_ffi`（`.dll` / `.so` / `.dylib`）、[`include/yc_hot.h`](yc-core/include/yc_hot.h)。

当前实现范围（M0–M2）：`yc-types`、`yc-session`、`yc-engine`（最小拼音 + ASCII/数字直出）、`yc-lexicon`（内存词表）、`yc-ffi`（热路径 C ABI + Arena）、`yc-cli`（桌面 REPL Demo 壳）。



## 交互 Demo



在 Cursor 中打开 Canvas 预览 **[ime-keyboard-ui.canvas.tsx](canvases/ime-keyboard-ui.canvas.tsx)**（路径因环境而异，见 KEYBOARD_UI_DESIGN 第 9 节链接）。默认采用 **Samsung Keyboard One UI 6.x** 浅色风格；支持工具栏「手写」入口、InkCanvas 绘制、抬笔模拟候选上屏。（Demo 为 UI 概念验证，桌面壳工程实现见对接教程第 7–9 节。）



## 范围说明



本仓库包含**设计文档**与 **`yc-core/` Rust 核心实现（M0/M1）**，不含：



- 六端 `yc-shell-*` 工程脚手架

- 词库/模型训练与后端 API

- 语言包 / AiPack CDN 与签名密钥运维



## 许可



内部设计文档，供团队评审与落地参考。

