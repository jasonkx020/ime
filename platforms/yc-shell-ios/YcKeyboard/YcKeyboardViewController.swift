import UIKit
import SwiftUI

public final class YcKeyboardViewController: UIInputViewController {
    private let client = YcHotPathClient()
    private var hosting: UIHostingController<SamsungKeyboardRoot>?
    private var snapshot = KeyboardSnapshot(editorId: 0, seq: 0, composing: "", candidates: [])

    public override func viewDidLoad() {
        super.viewDidLoad()
        _ = client.initCore(dataDir: NSTemporaryDirectory())
        mountKeyboard()
    }

    public override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        _ = client.beginSession(fieldId: 1)
        refresh()
    }

    public override func viewWillDisappear(_ animated: Bool) {
        client.stopSession()
        super.viewWillDisappear(animated)
    }

    private func mountKeyboard() {
        let root = SamsungKeyboardRoot(
            snapshot: Binding(
                get: { self.snapshot },
                set: { self.snapshot = $0 }
            ),
            onKey: { [weak self] key in self?.onKey(key) },
            onCandidate: { [weak self] cand in self?.onCandidate(cand) }
        )
        let host = UIHostingController(rootView: root)
        hosting = host
        addChild(host)
        view.addSubview(host.view)
        host.view.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            host.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            host.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            host.view.topAnchor.constraint(equalTo: view.topAnchor),
            host.view.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        host.didMove(toParent: self)
    }

    private func onKey(_ key: KeyDef) {
        if key.label == "⌫" {
            _ = client.submit(action: .backspace)
        } else if let code = key.keyCode {
            _ = client.submit(action: .keyPress, keyCode: code)
        }
        refresh()
    }

    private func onCandidate(_ cand: CandidateItem) {
        _ = client.submit(action: .selectCandidate, candidateId: cand.id)
        refresh()
    }

    private func refresh() {
        guard let snap = client.refreshIfNeeded(onCommit: { [weak self] text in
            self?.textDocumentProxy.insertText(text)
        }) else { return }
        snapshot = KeyboardSnapshot(
            editorId: snap.editorId,
            seq: snap.seq,
            composing: snap.composing,
            candidates: snap.candidates.map { CandidateItem(id: $0.id, text: $0.text) }
        )
    }
}

private struct SamsungKeyboardRoot: View {
    @Binding var snapshot: KeyboardSnapshot
    var onKey: (KeyDef) -> Void
    var onCandidate: (CandidateItem) -> Void

    var body: some View {
        SamsungKeyboardView(
            snapshot: $snapshot,
            onKey: onKey,
            onCandidate: onCandidate
        )
    }
}
