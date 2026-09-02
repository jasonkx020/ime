import Foundation

public struct ArenaCandidate: Equatable {
    public let id: UInt32
    public let text: String
}

public enum ArenaCommand: Equatable {
    case commit(text: String)
    case setComposing(text: String)
    case finishComposing
    case deleteSurrounding(before: UInt32, after: UInt32)
    case reloadKeyboard(layout: UInt32, layoutId: String)
}

public struct ArenaSnapshot: Equatable {
    public let editorId: UInt64
    public let seq: UInt64
    public let statusFlags: UInt32
    public let composing: String
    public let candidates: [ArenaCandidate]
    public let commands: [ArenaCommand]
}

public enum YcArena {
    private static let headerSize = 32
    private static let composingLen = 64
    private static let candSlotSize = 80
    private static let cmdSlotSize = 80
    private static let maxCandidates = 9
    private static let maxCommands = 4
    private static let maxTextLen = 64

    public static func parse(_ data: Data) -> ArenaSnapshot? {
        guard data.count >= headerSize else { return nil }
        let editorId = data.withUnsafeBytes { $0.load(fromByteOffset: 0, as: UInt64.self) }
        let seq = data.withUnsafeBytes { $0.load(fromByteOffset: 8, as: UInt64.self) }
        let statusFlags = data.withUnsafeBytes { $0.load(fromByteOffset: 16, as: UInt32.self) }
        let composingCount = Int(data.withUnsafeBytes { $0.load(fromByteOffset: 20, as: UInt32.self) })
        let candCount = min(Int(data.withUnsafeBytes { $0.load(fromByteOffset: 24, as: UInt32.self) }), maxCandidates)
        let cmdCount = min(Int(data.withUnsafeBytes { $0.load(fromByteOffset: 28, as: UInt32.self) }), maxCommands)

        let composing = String(data: data.subdata(in: headerSize..<(headerSize + min(composingCount, composingLen))), encoding: .utf8) ?? ""

        var candidates: [ArenaCandidate] = []
        let slotsOff = headerSize + composingLen
        for i in 0..<candCount {
            let off = slotsOff + i * candSlotSize
            guard off + candSlotSize <= data.count else { break }
            let id = data.withUnsafeBytes { $0.load(fromByteOffset: off, as: UInt32.self) }
            let textLen = min(Int(data.withUnsafeBytes { $0.load(fromByteOffset: off + 8, as: UInt32.self) }), maxTextLen)
            let text = String(data: data.subdata(in: (off + 16)..<(off + 16 + textLen)), encoding: .utf8) ?? ""
            candidates.append(ArenaCandidate(id: id, text: text))
        }

        var commands: [ArenaCommand] = []
        let cmdsOff = slotsOff + maxCandidates * candSlotSize
        for i in 0..<cmdCount {
            let off = cmdsOff + i * cmdSlotSize
            guard off + cmdSlotSize <= data.count else { break }
            let cmdType = data.withUnsafeBytes { $0.load(fromByteOffset: off, as: UInt32.self) }
            let param0 = data.withUnsafeBytes { $0.load(fromByteOffset: off + 4, as: UInt32.self) }
            let param1 = data.withUnsafeBytes { $0.load(fromByteOffset: off + 8, as: UInt32.self) }
            let textLen = min(Int(data.withUnsafeBytes { $0.load(fromByteOffset: off + 12, as: UInt32.self) }), maxTextLen)
            let text = String(data: data.subdata(in: (off + 16)..<(off + 16 + textLen)), encoding: .utf8) ?? ""
            switch cmdType {
            case 0: commands.append(.commit(text: text))
            case 1: commands.append(.setComposing(text: text))
            case 2: commands.append(.finishComposing)
            case 3: commands.append(.deleteSurrounding(before: param0, after: param1))
            case 4: commands.append(.reloadKeyboard(layout: param0, layoutId: text))
            default: break
            }
        }

        return ArenaSnapshot(editorId: editorId, seq: seq, statusFlags: statusFlags, composing: composing, candidates: candidates, commands: commands)
    }
}
