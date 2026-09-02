import Foundation

/// 键盘区占位协议。参见 docs/KEYBOARD_UI_DESIGN.md §3.3。
public protocol KeyView {
    func render(snapshot: Any)
    func applyTheme(tokens: Any)
}
