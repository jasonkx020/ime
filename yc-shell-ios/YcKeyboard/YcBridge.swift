import Foundation

public enum YcError: Int32 {
    case ok = 0
    case session = -1
    case busy = -2
    case internalError = -3
}

public enum YcActionType: UInt32 {
    case initAction = 0
    case keyPress = 1
    case backspace = 2
    case selectCandidate = 3
}

public final class YcHotPathClient {
    public private(set) var editorId: UInt64 = 0
    private var clientSeq: UInt64 = 0
    public private(set) var lastSeq: UInt64 = 0

    public init() {}

    public func initCore(dataDir: String) -> YcError {
        YcError(rawValue: dataDir.withCString { yc_core_init($0) }) ?? .internalError
    }

    public func shutdown() { yc_core_shutdown() }

    @discardableResult
    public func beginSession(fieldId: UInt64, inputType: UInt32 = 0) -> UInt64 {
        editorId = yc_session_begin_with_input(fieldId, inputType)
        clientSeq = 0
        lastSeq = 0
        if editorId != 0 {
            _ = submit(action: .initAction)
        }
        return editorId
    }

    public func stopSession(reason: UInt32 = 0) {
        if editorId != 0 {
            yc_session_stop(editorId, reason)
            editorId = 0
        }
    }

    public func validate() -> Bool {
        editorId != 0 && yc_session_validate(editorId) != 0
    }

    @discardableResult
    public func submit(action: YcActionType, keyCode: UInt32 = 0, candidateId: UInt32 = 0) -> YcError {
        guard editorId != 0 else { return .session }
        clientSeq += 1
        var hot = YcHotAction()
        hot.editor_id = editorId
        hot.client_seq = clientSeq
        hot.action_type = action.rawValue
        hot.key_code = keyCode
        hot.candidate_id = candidateId
        return YcError(rawValue: yc_hot_submit(&hot)) ?? .internalError
    }

    public func readArena() -> ArenaSnapshot? {
        guard let ptr = yc_hot_arena_ptr() else { return nil }
        let size = yc_hot_arena_size()
        guard size > 0 else { return nil }
        let data = Data(bytes: ptr, count: size)
        return YcArena.parse(data)
    }

    public func refreshIfNeeded(onCommit: (String) -> Void) -> ArenaSnapshot? {
        guard let snap = readArena(), snap.editorId == editorId, snap.seq != lastSeq else { return nil }
        lastSeq = snap.seq
        for cmd in snap.commands {
            if case .commit(let text) = cmd { onCommit(text) }
            if case .reloadKeyboard(_, let layoutId) = cmd, !layoutId.isEmpty {
                let rows = LayoutLoader.load(dataDir: NSTemporaryDirectory(), layoutId: layoutId)
                NotificationCenter.default.post(name: .ycReloadKeyboard, object: rows)
            }
        }
        return snap
    }
}

public struct YcHotAction {
    public var editor_id: UInt64 = 0
    public var client_seq: UInt64 = 0
    public var action_type: UInt32 = 0
    public var key_code: UInt32 = 0
    public var candidate_id: UInt32 = 0
    public var flags: UInt32 = 0
    public var reserved: (UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8) = (0, 0, 0, 0, 0, 0, 0, 0)
}

public extension Notification.Name {
    static let ycReloadKeyboard = Notification.Name("yc.reloadKeyboard")
}
@_silgen_name("yc_core_init")
func yc_core_init(_ dataDir: UnsafePointer<CChar>?) -> Int32

@_silgen_name("yc_core_shutdown")
func yc_core_shutdown()

@_silgen_name("yc_session_begin_with_input")
func yc_session_begin_with_input(_ fieldId: UInt64, _ inputType: UInt32) -> UInt64

@_silgen_name("yc_session_validate")
func yc_session_validate(_ editorId: UInt64) -> Int32

@_silgen_name("yc_session_stop")
func yc_session_stop(_ editorId: UInt64, _ reason: UInt32)

@_silgen_name("yc_hot_submit")
func yc_hot_submit(_ action: UnsafePointer<YcHotAction>?) -> Int32

@_silgen_name("yc_hot_arena_ptr")
func yc_hot_arena_ptr() -> UnsafePointer<UInt8>?

@_silgen_name("yc_hot_arena_size")
func yc_hot_arena_size() -> Int
