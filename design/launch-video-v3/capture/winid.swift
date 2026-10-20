// winid <pid>: the CGWindowID of that process's first normal (layer 0) on-screen window.
import CoreGraphics
let pid = Int(CommandLine.arguments[1]) ?? -1
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
for w in list {
  if let p = w[kCGWindowOwnerPID as String] as? Int, p == pid, let layer = w[kCGWindowLayer as String] as? Int, layer == 0,
     let id = w[kCGWindowNumber as String] as? Int { print(id); break }
}
