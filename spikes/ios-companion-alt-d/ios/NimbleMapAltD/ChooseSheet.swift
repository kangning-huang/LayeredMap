import SwiftUI

/// The picker behind a catalogue row that offers a family rather than a file.
///
/// Four rows described a family and handed over one member — "any country and any year from 2000
/// to 2020" gave China in 2020 to everyone. The fix is not better wording. A row that says "any
/// country" has to *ask which*, and this is where it asks.
///
/// The shape is deliberately the plainest thing iOS has: one row per axis, each pushing a list,
/// and one button at the bottom. Every axis starts on a sensible answer, so someone who does not
/// care can open this and tap Add to map without reading it — the old behaviour, now as a default
/// rather than as a ceiling.
struct ChooseSheet: View {
    var item: Catalog.Item
    /// Where the map is pointing, for the axes that can be answered from it.
    var view: GeoBounds?
    /// The size lookup is already running for this choice.
    var busy = false
    var onAdd: (URL, String) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var picks: [String: Catalog.Chooser.Option] = [:]

    private var chooser: Catalog.Chooser? { item.chooser }

    var body: some View {
        NavigationStack {
            Group {
                if let chooser {
                    list(chooser)
                } else {
                    ContentUnavailableView("Nothing to choose", systemImage: "square.stack.3d.up.slash")
                }
            }
            .navigationTitle(item.title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
        .onAppear { if picks.isEmpty, let chooser { picks = Self.defaults(for: chooser) } }
    }

    @ViewBuilder
    private func list(_ chooser: Catalog.Chooser) -> some View {
        List {
            if !item.blurb.isEmpty {
                Section { Text(item.blurb).font(.callout).foregroundStyle(.secondary) }
            }

            // Answering an axis from the map, when the map can answer it. A 10° tile is a thing
            // people have to look up; the phone already knows which one it is showing.
            if let from = chooser.fromView, let option = viewOption(chooser, from) {
                Section {
                    Button {
                        picks[from.key] = option
                    } label: {
                        HStack {
                            Label {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(from.label).foregroundStyle(Color.primary)
                                    Text(option.label).font(.caption).foregroundStyle(.secondary)
                                }
                            } icon: {
                                Image(systemName: "scope")
                            }
                            Spacer()
                            if picks[from.key] == option {
                                Image(systemName: "checkmark").foregroundStyle(Color.accentColor)
                            }
                        }
                    }
                }
            }

            ForEach(chooser.axes) { axis in
                let options = chooser.options(of: axis)
                Section {
                    NavigationLink {
                        OptionList(title: axis.label, options: options,
                                   chosen: picks[axis.key]) { picks[axis.key] = $0 }
                    } label: {
                        HStack(alignment: .firstTextBaseline) {
                            Text(axis.label)
                            Spacer(minLength: 12)
                            // Two lines, not one truncated one. "500 m — eastern Europe, the
                            // Middle East, Russia & India" on a single line became "500 m — …
                            // & India", which is not an answer to "which did I pick".
                            Text(picks[axis.key]?.label ?? "Choose")
                                .foregroundStyle(.secondary)
                                .multilineTextAlignment(.trailing)
                                .lineLimit(2)
                        }
                    }
                } footer: {
                    // 5,613 brands and 280 tiles are both "a list"; saying which kind of list it
                    // is up front stops it reading as a dropdown with four entries.
                    if options.count > 12 {
                        Text("\(options.count.formatted()) to choose from, searchable.")
                    }
                }
            }

            Section {
                Button {
                    guard let got = chooser.resolve(picks) else { return }
                    onAdd(got.url, got.name)
                    dismiss()
                } label: {
                    HStack {
                        Spacer()
                        if busy { ProgressView().controlSize(.small) } else { Text("Add to map") }
                        Spacer()
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(busy || chooser.resolve(picks) == nil)
                .listRowInsets(EdgeInsets())
                .listRowBackground(Color.clear)
            } footer: {
                // The sizes here run from 4 MB to half a gigabyte and the app will not know which
                // until it asks. Saying so beats a silent twenty-second wait.
                Text("The size is checked before anything downloads, and a big one asks first.")
            }
        }
    }

    /// The option matching where the map is pointing, if the publisher has one for there. An
    /// ocean tile simply is not offered rather than being offered and 404-ing.
    private func viewOption(_ chooser: Catalog.Chooser,
                            _ from: Catalog.Chooser.FromView) -> Catalog.Chooser.Option?
    {
        guard let view, let axis = chooser.axes.first(where: { $0.key == from.key }) else { return nil }
        let lat = (view.minLat + view.maxLat) / 2
        let lon = (view.minLon + view.maxLon) / 2
        let tile = from.tile(at: lat, lon)
        return chooser.options(of: axis).first { $0.value == tile }
    }

    private static func defaults(for chooser: Catalog.Chooser) -> [String: Catalog.Chooser.Option] {
        var out: [String: Catalog.Chooser.Option] = [:]
        for axis in chooser.axes {
            let options = chooser.options(of: axis)
            out[axis.key] = options.first { $0.value == axis.preset } ?? options.first
        }
        return out
    }
}

/// One axis, as a list. Searchable past a dozen entries, because the brand axis has 5,613 and
/// scrolling to Tesco is not a choice anyone would call one.
private struct OptionList: View {
    var title: String
    var options: [Catalog.Chooser.Option]
    var chosen: Catalog.Chooser.Option?
    var onPick: (Catalog.Chooser.Option) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var search = ""
    /// Each label lowercased once, because the brand axis has 5,613 of them.
    ///
    /// The earlier round of this study measured a 106 ms keystroke in the catalogue search, from
    /// lowercasing and splitting 5,705 entries inside a view body. `localizedCaseInsensitiveContains`
    /// over the same number of brands is the same mistake wearing a different method name: it goes
    /// through ICU, and a view body runs it on every character typed.
    @State private var folded: [(key: String, option: Catalog.Chooser.Option)] = []

    private var shown: [Catalog.Chooser.Option] {
        let text = search.trimmingCharacters(in: .whitespaces).lowercased()
        guard !text.isEmpty else { return options }
        // Names first, so "ikea" puts Ikea above "Ikea (Turkey)" and above anything that merely
        // mentions it.
        return folded
            .filter { $0.key.contains(text) }
            .sorted { a, b in
                if (a.key == text) != (b.key == text) { return a.key == text }
                let ap = a.key.hasPrefix(text), bp = b.key.hasPrefix(text)
                if ap != bp { return ap }
                if a.key.count != b.key.count { return a.key.count < b.key.count }
                return a.key < b.key
            }
            .map(\.option)
    }

    var body: some View {
        List(shown) { option in
            Button {
                onPick(option)
                dismiss()
            } label: {
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(option.label).foregroundStyle(Color.primary)
                        if !option.note.isEmpty {
                            Text(option.note).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                        }
                    }
                    Spacer()
                    if option == chosen {
                        Image(systemName: "checkmark").foregroundStyle(Color.accentColor)
                    }
                }
            }
        }
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
        .modifier(SearchIfLong(enabled: options.count > 12, text: $search))
        .task {
            guard folded.isEmpty else { return }
            folded = options.map { (key: $0.label.lowercased(), option: $0) }
        }
        .overlay {
            if shown.isEmpty {
                ContentUnavailableView.search(text: search)
            }
        }
    }
}

/// `.searchable` on a list of four looks like the app lost something.
private struct SearchIfLong: ViewModifier {
    var enabled: Bool
    @Binding var text: String

    func body(content: Content) -> some View {
        if enabled {
            content.searchable(text: $text, placement: .navigationBarDrawer(displayMode: .always))
        } else {
            content
        }
    }
}
