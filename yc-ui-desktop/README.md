# yc-ui-desktop

共享 C++ 桌面 UI 契约与热路径客户端（M1）：

- `include/yc_arena_parser.hpp` — Arena 解析
- `include/yc_hot_path_client.hpp` — `HotPathClient`（submit + refreshUi）
- `include/yc_theme_tokens.hpp` — Samsung Token
- `include/yc_ui_panel.hpp` — `IKeyboardPanel` 接口

Win/Linux shell 通过 `yc_platform_adapter` 链接 `yc_ffi` 并运行 M1 smoke。
