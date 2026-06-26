import Foundation

enum RustCoreBridge {
    private final class EventBox {
        var events: [String] = []
    }

    private final class EventHandlerBox {
        let handler: (String) -> Void

        init(handler: @escaping (String) -> Void) {
            self.handler = handler
        }
    }

    private static var retainedEventHandler: Unmanaged<EventHandlerBox>?

    static func protocolVersion() -> Int {
        Int(meron_core_protocol_version())
    }

    static func pingJson() -> String {
        ownedString(meron_core_ping_json())
    }

    static func initJson(dataDirectory: String, dbKey: String = "") -> String {
        if dbKey.isEmpty {
            return dataDirectory.withCString { pointer in
                ownedString(meron_core_init_json(pointer))
            }
        }
        return dataDirectory.withCString { dirPointer in
            dbKey.withCString { keyPointer in
                ownedString(meron_core_init_json_keyed(dirPointer, keyPointer))
            }
        }
    }

    static func invokeJson(_ request: String) -> String {
        request.withCString { pointer in
            ownedString(meron_core_invoke_json(pointer))
        }
    }

    static func setEventHandler(_ handler: ((String) -> Void)?) {
        if let retainedEventHandler {
            meron_core_register_event_callback(nil, nil)
            retainedEventHandler.release()
            self.retainedEventHandler = nil
        }
        guard let handler else {
            return
        }
        let box = EventHandlerBox(handler: handler)
        let retained = Unmanaged.passRetained(box)
        retainedEventHandler = retained
        meron_core_register_event_callback({ eventJson, userData in
            guard let eventJson, let userData else {
                return
            }
            let box = Unmanaged<EventHandlerBox>.fromOpaque(userData).takeUnretainedValue()
            let json = String(cString: eventJson)
            DispatchQueue.main.async {
                box.handler(json)
            }
        }, retained.toOpaque())
    }

    static func readyEvents() -> [String] {
        let box = EventBox()
        let opaque = Unmanaged.passUnretained(box).toOpaque()
        meron_core_register_event_callback({ eventJson, userData in
            guard let eventJson, let userData else {
                return
            }
            let box = Unmanaged<EventBox>.fromOpaque(userData).takeUnretainedValue()
            box.events.append(String(cString: eventJson))
        }, opaque)
        _ = meron_core_emit_ready_event()
        meron_core_register_event_callback(nil, nil)
        return box.events
    }

    private static func ownedString(_ pointer: UnsafeMutablePointer<CChar>?) -> String {
        guard let pointer else {
            return #"{"error":{"message":"meron-core returned null"}}"#
        }
        defer { meron_core_string_free(pointer) }
        return String(cString: pointer)
    }
}
