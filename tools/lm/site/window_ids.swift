// window_ids.swift PID: prints the on-screen, normal-level window ids owned by process PID, one per
// line (used by retake_screenshots.sh to capture only the NimbleMap window it launched).
import CoreGraphics

guard CommandLine.arguments.count == 2, let pid = Int(CommandLine.arguments[1]) else {
    print("usage: window_ids PID")
    exit(2)
}
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
for w in list {
    guard let owner = w[kCGWindowOwnerPID as String] as? Int, owner == pid,
          let layer = w[kCGWindowLayer as String] as? Int, layer == 0,
          let id = w[kCGWindowNumber as String] as? Int else { continue }
    if let b = w[kCGWindowBounds as String] as? [String: Any], let h = b["Height"] as? Double, h < 100 { continue }
    print(id)
}
