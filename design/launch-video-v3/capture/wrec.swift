// wrec: record ONE window (by CGWindowID) to an H.264 .mov with ScreenCaptureKit.
// Only that window's pixels are captured (SCContentFilter(desktopIndependentWindow:)), never the
// screen, and the system pointer is not drawn. Stops when the window closes, on SIGINT/SIGTERM,
// or after MAXSEC seconds.
//
//   swiftc -O -o wrec wrec.swift
//   ./wrec <windowID> <out.mov> [maxSeconds=120] [fps=60]
import AppKit
import AVFoundation
import CoreMedia
import Foundation
import ScreenCaptureKit

let args = CommandLine.arguments
_ = NSApplication.shared  // initialises the window server connection (CGS)
let wid: UInt32 = args.count >= 3 ? UInt32(args[1]) ?? 0 : 0
if wid == 0 {
    FileHandle.standardError.write("usage: wrec <windowID> <out.mov> [maxSeconds] [fps]\n".data(using: .utf8)!)
    exit(2)
}
let outURL = URL(fileURLWithPath: args[2])
let maxSec = args.count > 3 ? Double(args[3]) ?? 120 : 120
let fps = args.count > 4 ? Int32(args[4]) ?? 60 : 60
try? FileManager.default.removeItem(at: outURL)

final class Rec: NSObject, SCStreamOutput, SCStreamDelegate {
    var writer: AVAssetWriter!
    var input: AVAssetWriterInput!
    var adaptor: AVAssetWriterInputPixelBufferAdaptor!
    var stream: SCStream?
    var started = false
    var frames = 0
    var finished = false
    let q = DispatchQueue(label: "wrec")

    func start() async throws {
        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: false)
        guard let win = content.windows.first(where: { $0.windowID == wid }) else {
            FileHandle.standardError.write("window \(wid) not found\n".data(using: .utf8)!)
            exit(3)
        }
        let filter = SCContentFilter(desktopIndependentWindow: win)
        let scale = CGFloat(filter.pointPixelScale)
        let w = Int(win.frame.width * scale) & ~1
        let h = Int(win.frame.height * scale) & ~1
        let cfg = SCStreamConfiguration()
        cfg.width = w
        cfg.height = h
        cfg.minimumFrameInterval = CMTime(value: 1, timescale: fps)
        cfg.showsCursor = false
        cfg.pixelFormat = kCVPixelFormatType_32BGRA
        cfg.queueDepth = 8
        cfg.capturesAudio = false
        cfg.ignoreShadowsSingleWindow = true
        writer = try AVAssetWriter(outputURL: outURL, fileType: .mov)
        input = AVAssetWriterInput(mediaType: .video, outputSettings: [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: w, AVVideoHeightKey: h,
            AVVideoCompressionPropertiesKey: [
                AVVideoAverageBitRateKey: 80_000_000,
                AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel,
                AVVideoExpectedSourceFrameRateKey: fps,
            ] as [String: Any],
        ])
        input.expectsMediaDataInRealTime = true
        adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: nil)
        writer.add(input)
        writer.startWriting()
        let s = SCStream(filter: filter, configuration: cfg, delegate: self)
        try s.addStreamOutput(self, type: .screen, sampleHandlerQueue: q)
        try await s.startCapture()
        stream = s
        print("recording window \(wid) \(w)x\(h) @\(fps) -> \(outURL.path)")
        fflush(stdout)
    }

    func stream(_ stream: SCStream, didOutputSampleBuffer sb: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, !finished, sb.isValid else { return }
        // Only complete frames carry an image.
        guard let att = CMSampleBufferGetSampleAttachmentsArray(sb, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
              let raw = att.first?[.status] as? Int, let st = SCFrameStatus(rawValue: raw), st == .complete,
              let pb = CMSampleBufferGetImageBuffer(sb) else { return }
        let t = CMSampleBufferGetPresentationTimeStamp(sb)
        if !started {
            writer.startSession(atSourceTime: t)
            started = true
        }
        if input.isReadyForMoreMediaData {
            adaptor.append(pb, withPresentationTime: t)
            frames += 1
        }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        print("stream stopped: \(error.localizedDescription)")
        finish()
    }

    func finish() {
        q.async {
            if self.finished { return }
            self.finished = true
            let done = { print("wrote \(self.frames) frames"); exit(0) }
            if self.started {
                self.input.markAsFinished()
                self.writer.finishWriting(completionHandler: done)
            } else {
                self.writer.cancelWriting()
                done()
            }
        }
    }
}

let rec = Rec()
signal(SIGINT, SIG_IGN)
signal(SIGTERM, SIG_IGN)
let sigs = [SIGINT, SIGTERM].map { s -> DispatchSourceSignal in
    let src = DispatchSource.makeSignalSource(signal: s, queue: .main)
    src.setEventHandler {
        Task { try? await rec.stream?.stopCapture(); rec.finish() }
    }
    src.resume()
    return src
}
Task {
    do { try await rec.start() } catch {
        FileHandle.standardError.write("start failed: \(error)\n".data(using: .utf8)!)
        exit(4)
    }
}
// Stop when the window is gone (the tour closes the app) or after maxSec.
let t0 = Date()
Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { _ in
    let list = CGWindowListCopyWindowInfo([.optionIncludingWindow], CGWindowID(wid)) as? [[String: Any]] ?? []
    if list.isEmpty || Date().timeIntervalSince(t0) > maxSec {
        Task { try? await rec.stream?.stopCapture(); rec.finish() }
    }
}
RunLoop.main.run()
