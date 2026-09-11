import SwiftUI

public struct ThemeTokens {
    public var keyboardBg: Color = Color(red: 0.91, green: 0.92, blue: 0.93)
    public var keyNormal: Color = .white
    public var keyUtility: Color = Color(red: 0.87, green: 0.88, blue: 0.89)
    public var keyAccent: Color = Color(red: 0.10, green: 0.45, blue: 0.91)
    public var keyPressed: Color = Color(red: 0.78, green: 0.80, blue: 0.82)
    public var composingText: Color = Color(red: 0.10, green: 0.45, blue: 0.91)
    public var candText: Color = Color(red: 0.13, green: 0.13, blue: 0.14)
    public var toolbarText: Color = Color(red: 0.37, green: 0.39, blue: 0.41)
    public var keyRadius: CGFloat = 12
}

public struct KeyboardSnapshot: Equatable {
    public var editorId: UInt64
    public var seq: UInt64
    public var composing: String
    public var candidates: [CandidateItem]
}

public struct CandidateItem: Identifiable, Equatable {
    public var id: UInt32
    public var text: String
}

public struct KeyDef: Identifiable, Equatable {
    public var id: String { label }
    public var label: String
    public var widthWeight: CGFloat = 1
    public var style: KeyStyle = .normal
    public var keyCode: UInt32?
}

public enum KeyStyle { case normal, utility, accent }

public enum Layout26Pinyin {
    public static let rows: [[KeyDef]] = [
        ["q","w","e","r","t","y","u","i","o","p"].map { KeyDef(label: $0, keyCode: UInt32($0.unicodeScalars.first!.value)) },
        ["a","s","d","f","g","h","j","k","l"].map { KeyDef(label: $0, keyCode: UInt32($0.unicodeScalars.first!.value)) } +
            [KeyDef(label: "⌫", widthWeight: 1.35, style: .utility, keyCode: 8)],
        ["z","x","c","v","b","n","m"].map { KeyDef(label: $0, keyCode: UInt32($0.unicodeScalars.first!.value)) },
        [
            KeyDef(label: "!#1", widthWeight: 1.1, style: .utility),
            KeyDef(label: "🌐", widthWeight: 1.1, style: .utility),
            KeyDef(label: ",", widthWeight: 0.85, keyCode: 44),
            KeyDef(label: "空格", widthWeight: 3.6, keyCode: 32),
            KeyDef(label: "。", widthWeight: 0.85, keyCode: 46),
            KeyDef(label: "搜索", widthWeight: 1.4, style: .accent),
        ],
    ]
}
