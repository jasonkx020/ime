import Foundation

@main
struct YcInputServer {
    static func main() {
        let client = YcHotPathClient()
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        let dataDir = support.appendingPathComponent("YcInput").path
        _ = client.initCore(dataDir: dataDir)
        _ = client.beginSession(fieldId: 1)

        for ch in "nihao" {
            _ = client.submit(action: .keyPress, keyCode: UInt32(ch.asciiValue ?? 0))
            client.refreshIfNeeded { text in print("commit: \(text)") }
        }
        if let snap = client.readArena(), let first = snap.candidates.first {
            _ = client.submit(action: .selectCandidate, candidateId: first.id)
            client.refreshIfNeeded { text in print("M1 commit: \(text)") }
        }

        client.stopSession()
        client.shutdown()
        print("yc-shell-macos M1 smoke done")
    }
}
