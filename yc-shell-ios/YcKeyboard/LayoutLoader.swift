import Foundation

public enum LayoutLoader {
    public static func load(dataDir: String, layoutId: String) -> [[KeyDef]] {
        let langpacks = URL(fileURLWithPath: dataDir).appendingPathComponent("langpacks")
        guard let packs = try? FileManager.default.contentsOfDirectory(at: langpacks, includingPropertiesForKeys: nil) else {
            return Layout26Pinyin.rows
        }
        for pack in packs {
            let bin = pack.appendingPathComponent("layouts/\(layoutId).bin")
            if FileManager.default.fileExists(atPath: bin.path),
               let data = try? Data(contentsOf: bin),
               let rows = parseBin(data) {
                return rows
            }
        }
        return Layout26Pinyin.rows
    }

    private static func parseBin(_ data: Data) -> [[KeyDef]]? {
        guard data.count >= 76 else { return nil }
        let magic = String(data: data.subdata(in: 0..<4), encoding: .ascii)
        guard magic == "YCLY" else { return nil }
        let keyCount = Int(data.withUnsafeBytes { $0.load(fromByteOffset: 72, as: UInt32.self) })
        var keys: [KeyDef] = []
        let slotSize = 16 + 16 + 1 + 4
        var off = 76
        for _ in 0..<keyCount {
            guard off + slotSize <= data.count else { break }
            let label = cstr(data, off, 16)
            let output = cstr(data, off + 16, 16)
            keys.append(KeyDef(label: label.isEmpty ? output : label, keyCode: output.first.map { Int($0.asciiValue ?? 0) }))
            off += slotSize
        }
        return keys.isEmpty ? nil : [keys]
    }

    private static func cstr(_ data: Data, _ off: Int, _ max: Int) -> String {
        let slice = data.subdata(in: off..<off + max)
        let end = slice.firstIndex(of: 0) ?? slice.count
        return String(data: slice.prefix(end), encoding: .utf8) ?? ""
    }
}

private enum Layout26Pinyin {
    static let rows: [[KeyDef]] = [
        ["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"].map { KeyDef(label: $0, keyCode: Int($0.first!.asciiValue!)) }
    ]
}

public struct KeyDef {
    public let label: String
    public let keyCode: Int?
    public init(label: String, keyCode: Int? = nil) {
        self.label = label
        self.keyCode = keyCode
    }
}
