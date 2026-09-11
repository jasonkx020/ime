import Foundation

/// 候选栏占位协议。参见 docs/KEYBOARD_UI_DESIGN.md §3.1。
public protocol CandBar {
    func render(snapshot: Any)
    func applyTheme(tokens: Any)
}
