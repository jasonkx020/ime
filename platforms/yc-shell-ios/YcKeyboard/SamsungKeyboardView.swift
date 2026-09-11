import SwiftUI

public struct SamsungKeyboardView: View {
    @Binding public var snapshot: KeyboardSnapshot
    public var tokens: ThemeTokens
    public var onKey: (KeyDef) -> Void
    public var onCandidate: (CandidateItem) -> Void
    public var onToolbar: (String) -> Void

    private let toolbarItems = ["设置", "翻译", "剪贴板", "语音", "表情", "手写"]

    public init(
        snapshot: Binding<KeyboardSnapshot>,
        tokens: ThemeTokens = ThemeTokens(),
        onKey: @escaping (KeyDef) -> Void,
        onCandidate: @escaping (CandidateItem) -> Void,
        onToolbar: @escaping (String) -> Void = { _ in }
    ) {
        self._snapshot = snapshot
        self.tokens = tokens
        self.onKey = onKey
        self.onCandidate = onCandidate
        self.onToolbar = onToolbar
    }

    public var body: some View {
        VStack(spacing: 0) {
            candBar
            toolbar
            keyView
        }
        .background(tokens.keyboardBg)
    }

    private var candBar: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                if !snapshot.composing.isEmpty {
                    Text(snapshot.composing)
                        .font(.system(size: 13))
                        .foregroundStyle(tokens.composingText)
                }
                ForEach(snapshot.candidates) { cand in
                    Button(cand.text) { onCandidate(cand) }
                        .font(.system(size: 15))
                        .padding(.horizontal, 10)
                        .padding(.vertical, 6)
                        .background(RoundedRectangle(cornerRadius: 16).fill(Color.white))
                        .overlay(RoundedRectangle(cornerRadius: 16).stroke(tokens.keyAccent, lineWidth: 1))
                        .foregroundStyle(tokens.candText)
                }
            }
            .padding(.horizontal, 10)
        }
        .frame(height: 52)
    }

    private var toolbar: some View {
        HStack {
            ForEach(toolbarItems, id: \.self) { item in
                Button(item) { onToolbar(item) }
                    .font(.system(size: 12))
                    .foregroundStyle(tokens.toolbarText)
                    .frame(maxWidth: .infinity, minHeight: 36)
            }
        }
        .frame(height: 36)
    }

    private var keyView: some View {
        VStack(spacing: 6) {
            ForEach(Array(Layout26Pinyin.rows.enumerated()), id: \.offset) { _, row in
                HStack(spacing: 3) {
                    ForEach(row) { key in
                        keyButton(key)
                    }
                }
            }
        }
        .padding(12)
    }

    private func keyButton(_ key: KeyDef) -> some View {
        Button(key.label) { onKey(key) }
            .font(.system(size: 16))
            .frame(maxWidth: .infinity * key.widthWeight)
            .frame(height: 44)
            .background(
                RoundedRectangle(cornerRadius: tokens.keyRadius)
                    .fill(keyFill(key.style))
            )
            .foregroundStyle(key.style == .accent ? Color.white : tokens.candText)
    }

    private func keyFill(_ style: KeyStyle) -> Color {
        switch style {
        case .normal: return tokens.keyNormal
        case .utility: return tokens.keyUtility
        case .accent: return tokens.keyAccent
        }
    }
}
