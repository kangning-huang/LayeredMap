// lm-dictate: speech to text for NimbleMap Pro's Ask box (WP7.1, "speaking instead of typing").
//
// The app starts this helper (Contents/MacOS/lm-dictate) when the microphone is clicked and reads
// its standard output, one line per event:
//   ready                 listening
//   partial<TAB>text      the words so far (replaces the previous partial)
//   final<TAB>text        the finished text; the helper then exits
//   error<TAB>message     a reason in plain words; the helper then exits
// It stops when its standard input closes (the microphone clicked again), after 2 s of silence
// once something was heard, or after 60 s. Recognition runs on the Mac when the language
// supports it (Apple silicon: English and Chinese among others); nothing is sent to NimbleMap.
// The language is the system's, or the first argument ("zh-CN", "en-US").

import AVFoundation
import Foundation
import Speech

setvbuf(stdout, nil, _IOLBF, 0)

// The display names; keep in step with lm_core::brand (docs/lm/naming.md).
let appName = "NimbleMap"
let proName = "NimbleMap Pro"

func emit(_ kind: String, _ text: String = "") {
    let clean = text.replacingOccurrences(of: "\n", with: " ").replacingOccurrences(of: "\t", with: " ")
    print(text.isEmpty ? kind : "\(kind)\t\(clean)")
}

func fail(_ message: String) -> Never {
    emit("error", message)
    exit(1)
}

let locale = CommandLine.arguments.count > 1 ? Locale(identifier: CommandLine.arguments[1]) : Locale.current
guard let recognizer = SFSpeechRecognizer(locale: locale) ?? SFSpeechRecognizer() else {
    fail("Speech recognition isn't available for this language.")
}

let engine = AVAudioEngine()
let request = SFSpeechAudioBufferRecognitionRequest()
request.shouldReportPartialResults = true
if recognizer.supportsOnDeviceRecognition {
    request.requiresOnDeviceRecognition = true
} else if ProcessInfo.processInfo.environment["LM_OFFLINE"] == "1" {
    // Strict offline mode (WP6.6): Apple's server recognizer would go online.
    fail("Speaking needs on-device speech recognition while \(appName) is working offline.")
}

var latest = ""
var lastChange = Date()
var finished = false
var task: SFSpeechRecognitionTask?

func finish() {
    if finished { return }
    finished = true
    engine.stop()
    engine.inputNode.removeTap(onBus: 0)
    request.endAudio()
    // Give the recognizer a moment to settle the last words.
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) {
        emit("final", latest)
        exit(0)
    }
}

func start() {
    let input = engine.inputNode
    let format = input.outputFormat(forBus: 0)
    if format.channelCount == 0 {
        fail("No microphone was found.")
    }
    input.installTap(onBus: 0, bufferSize: 1024, format: format) { buffer, _ in
        request.append(buffer)
    }
    engine.prepare()
    do {
        try engine.start()
    } catch {
        fail("The microphone couldn't start: \(error.localizedDescription)")
    }
    task = recognizer.recognitionTask(with: request) { result, error in
        if let result = result {
            let text = result.bestTranscription.formattedString
            if text != latest {
                latest = text
                lastChange = Date()
                emit("partial", text)
            }
            if result.isFinal {
                DispatchQueue.main.async { finish() }
            }
        } else if error != nil && !finished {
            DispatchQueue.main.async { finish() }
        }
    }
    emit("ready")
    let began = Date()
    Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { _ in
        let quiet = Date().timeIntervalSince(lastChange)
        if (!latest.isEmpty && quiet > 2.0) || Date().timeIntervalSince(began) > 60 {
            finish()
        }
    }
}

// Standard input closing means "stop".
DispatchQueue.global().async {
    while readLine() != nil {}
    DispatchQueue.main.async { finish() }
}

func askMicrophone() {
    switch AVCaptureDevice.authorizationStatus(for: .audio) {
    case .authorized:
        DispatchQueue.main.async { start() }
    case .notDetermined:
        AVCaptureDevice.requestAccess(for: .audio) { ok in
            if ok { DispatchQueue.main.async { start() } } else { fail("\(proName) isn't allowed to use the microphone. Allow it in System Settings › Privacy & Security › Microphone.") }
        }
    default:
        fail("\(proName) isn't allowed to use the microphone. Allow it in System Settings › Privacy & Security › Microphone.")
    }
}

SFSpeechRecognizer.requestAuthorization { status in
    switch status {
    case .authorized:
        askMicrophone()
    case .denied, .restricted:
        fail("Speech recognition is turned off for \(proName). Allow it in System Settings › Privacy & Security › Speech Recognition.")
    default:
        fail("Speech recognition isn't available right now.")
    }
}

RunLoop.main.run()
