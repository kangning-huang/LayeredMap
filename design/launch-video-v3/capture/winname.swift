// winname <owner> <title>: the CGWindowID (and bounds) of an on-screen window by owner and title.
import CoreGraphics
let owner = CommandLine.arguments[1], title = CommandLine.arguments[2]
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
for w in list {
  if (w[kCGWindowOwnerName as String] as? String) == owner, (w[kCGWindowName as String] as? String) == title,
     let id = w[kCGWindowNumber as String] as? Int {
    print(id, w[kCGWindowBounds as String] ?? ""); break
  }
}
