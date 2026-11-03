import SafariServices
import SwiftUI
import WebKit

/// A's in-app browser on the NimbleMap site's data page, back in C under the Data library (the
/// owner: the site's Data tab is a good way to browse datasets). A tap on a link to a file NimbleMap
/// opens downloads it onto the map; anything else stays a web page.
struct DataWebPage: View {
    /// Which page: the site's data page by default, or a web search when the catalogues came back
    /// empty. A link to a file NimbleMap opens is intercepted either way.
    var url: URL? = DataWebPage.page
    var title = "NimbleMap data"
    var onDataLink: (URL) -> Void
    /// A file the web view downloaded itself (a `download` link, or a `blob:` export from a
    /// portal's own tooling), already on disk and ready to open.
    var onDataFile: (URL) -> Void = { _ in }

    @State private var safari = false
    static let page = URL(string: "https://nimblemap.com/data.html")

    var body: some View {
        Group {
            if let url {
                DataWebView(url: url, onDataLink: onDataLink, onDataFile: onDataFile)
                    .ignoresSafeArea(edges: .bottom)
            }
        }
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    safari = true
                } label: {
                    Image(systemName: "safari")
                }
                .accessibilityLabel("Open in Safari")
            }
        }
        .sheet(isPresented: $safari) {
            if let url { SafariView(url: url) }
        }
    }
}

struct DataWebView: UIViewRepresentable {
    var url: URL
    var onDataLink: (URL) -> Void
    /// A file the web view downloaded itself, already on disk.
    var onDataFile: (URL) -> Void = { _ in }

    func makeUIView(context: Context) -> WKWebView {
        let web = WKWebView(frame: .zero)
        web.navigationDelegate = context.coordinator
        web.load(URLRequest(url: url))
        return web
    }

    func updateUIView(_ web: WKWebView, context: Context) {
        context.coordinator.onDataLink = onDataLink
        context.coordinator.onDataFile = onDataFile
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(onDataLink: onDataLink, onDataFile: onDataFile)
    }

    /// Catches a data file however the page hands it over.
    ///
    /// Sniffing the tapped link was not enough, and the site's own data page is the proof: its
    /// download links carry an HTML `download` attribute, which WKWebView turns into a
    /// `WKDownload` instead of a navigation. With no download delegate those simply did nothing —
    /// no file, no error, no message. That is why the NetCDF links did not open.
    ///
    /// So there are three doors now: the tapped link (cheapest, and keeps the app's own progress
    /// bar), the response headers (a portal that answers `Content-Disposition: attachment` or a
    /// data media type), and the download itself (everything else, including `blob:` URLs that no
    /// amount of URL sniffing can catch).
    final class Coordinator: NSObject, WKNavigationDelegate, WKDownloadDelegate {
        var onDataLink: (URL) -> Void
        var onDataFile: (URL) -> Void

        init(onDataLink: @escaping (URL) -> Void, onDataFile: @escaping (URL) -> Void) {
            self.onDataLink = onDataLink
            self.onDataFile = onDataFile
        }

        static func isData(_ url: URL) -> Bool {
            let ext = url.pathExtension.lowercased()
            if Importer.extensions.contains(ext) && ext != "txt" && ext != "json" { return true }
            let s = url.absoluteString.lowercased()
            return s.contains("rows.csv") || s.hasSuffix(".geojson") || s.contains("format=geojson")
        }

        /// Media types that mean "this is the data", not a page about it.
        private static func isDataResponse(_ response: URLResponse) -> Bool {
            if let http = response as? HTTPURLResponse,
               let disposition = http.value(forHTTPHeaderField: "Content-Disposition")?.lowercased(),
               disposition.contains("attachment")
            {
                return true
            }
            let mime = (response.mimeType ?? "").lowercased()
            if mime.contains("html") || mime.contains("xhtml") { return false }
            let known = ["geo+json", "application/zip", "zipped-shapefile", "x-netcdf", "netcdf",
                         "text/csv", "tab-separated", "image/tiff", "geopackage", "google-earth",
                         "octet-stream", "gpx"]
            return known.contains { mime.contains($0) }
        }

        @MainActor
        func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction) async -> WKNavigationActionPolicy {
            if let url = navigationAction.request.url, navigationAction.navigationType == .linkActivated, Coordinator.isData(url) {
                onDataLink(url)
                return .cancel
            }
            return .allow
        }

        /// The server has answered and it is a file, not a page.
        @MainActor
        func webView(_ webView: WKWebView, decidePolicyFor navigationResponse: WKNavigationResponse) async -> WKNavigationResponsePolicy {
            guard navigationResponse.canShowMIMEType == false || Coordinator.isDataResponse(navigationResponse.response) else {
                return .allow
            }
            // An http(s) address can go through the app's own downloader, which shows the progress
            // bar and the Stop button. Anything else (blob:, data:) has to be downloaded here.
            if let url = navigationResponse.response.url, url.scheme?.hasPrefix("http") == true,
               Coordinator.isData(url) || Coordinator.isDataResponse(navigationResponse.response)
            {
                onDataLink(url)
                return .cancel
            }
            return .download
        }

        func webView(_ webView: WKWebView, navigationAction: WKNavigationAction, didBecome download: WKDownload) {
            download.delegate = self
        }

        func webView(_ webView: WKWebView, navigationResponse: WKNavigationResponse, didBecome download: WKDownload) {
            download.delegate = self
        }

        // MARK: WKDownloadDelegate

        func download(
            _ download: WKDownload,
            decideDestinationUsing response: URLResponse,
            suggestedFilename: String,
            completionHandler: @escaping (URL?) -> Void
        ) {
            let name = suggestedFilename.isEmpty ? "download" : suggestedFilename
            let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
            try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            completionHandler(dir.appendingPathComponent(name))
        }

        func downloadDidFinish(_ download: WKDownload) {
            guard let file = download.progress.fileURL else { return }
            let hand = onDataFile
            Task { @MainActor in hand(file) }
        }

        func download(_ download: WKDownload, didFailWithError error: Error, resumeData: Data?) {
            // Nothing to hand over; the sheet stays put rather than silently closing.
        }
    }
}

/// So a `URL?` can drive `.sheet(item:)` for the Safari search.
extension URL: @retroactive Identifiable {
    public var id: String { absoluteString }
}

struct SafariView: UIViewControllerRepresentable {
    var url: URL

    func makeUIViewController(context: Context) -> SFSafariViewController {
        SFSafariViewController(url: url)
    }

    func updateUIViewController(_ controller: SFSafariViewController, context: Context) {}
}
