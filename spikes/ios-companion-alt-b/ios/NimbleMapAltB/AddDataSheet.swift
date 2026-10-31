import SwiftUI

/// "+" entry: library / Files / Paste link stubs. Paste link reuses URL open for the spike.
struct AddDataSheet: View {
    @Environment(\.dismiss) private var dismiss
    var onPasteLink: (String) -> Void
    @State private var showPasteLink = false

    var body: some View {
        NavigationStack {
            List {
                Section {
                    TextField("Search the data library", text: .constant(""))
                        .disabled(true)
                }

                Section("Data library") {
                    Text("World cities")
                    Text("Summer heat")
                }
                .disabled(true)

                Section {
                    Button("From Files") {}
                        .disabled(true)
                    Button("Paste link") {
                        showPasteLink = true
                    }
                }

                Section {
                    Text("Got a file in Mail or WeChat? Tap Share, then NimbleMap. (Share extension not in this spike.)")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
            }
            .navigationTitle("Add data")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
            }
            .sheet(isPresented: $showPasteLink) {
                OpenURLSheet { url in
                    onPasteLink(url)
                    dismiss()
                }
            }
        }
        .presentationDetents([.medium, .large])
    }
}
