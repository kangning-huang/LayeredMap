import Foundation

/// A small model, used only where the catalogues cannot help themselves.
///
/// Two jobs, both cheap and both optional:
///
/// 1. **Rewrite** what a person typed into the words a catalogue indexes. "where do people live in
///    china" is a sentence; Socrata wants "population density china". A catalogue search is a
///    keyword match, so a sentence returns nothing and the app looks broken.
/// 2. **Rank and explain** what came back, so the best three are at the top with a line saying why
///    this file answers that question.
///
/// What it deliberately does **not** do is search. Letting the model search costs about seventy
/// times what the tokens cost — OpenRouter's web plugin is $0.007 a request against roughly
/// $0.0001 of tokens — and returns worse results than the catalogues on everything the catalogues
/// cover. The catalogues search; the model only helps aim and read.
///
/// Everything here is skippable. With no key the app behaves exactly as it did before: the bundled
/// catalogue, then Socrata and ArcGIS Hub, then a web search. Nothing shows a spinner waiting on a
/// model, and nothing fails because one was unavailable.
enum Assist {
    /// OpenRouter, because the project already uses it (`tools/lm/llm_eval`). The key is read from
    /// the environment or the app's own defaults — never compiled in, never in the repository.
    private static let endpoint = URL(string: "https://openrouter.ai/api/v1/chat/completions")

    /// Measured on this project's own benchmark: glm-5.3-flash is $0.029/M in and $0.092/M out, so
    /// a rewrite plus a rank costs on the order of $0.0001. The eval harness already scored it on
    /// NimbleMap's own tasks, which is why it is the default rather than something untested.
    static let model = "z-ai/glm-5.3-flash"

    static var key: String? {
        if let k = ProcessInfo.processInfo.environment["LM_OPENROUTER_KEY"], !k.isEmpty { return k }
        let k = UserDefaults.standard.string(forKey: "lm.openrouter.key")
        return (k?.isEmpty == false) ? k : nil
    }

    static var available: Bool { key != nil }

    /// Catalogue words for a sentence. Returns nil when there is no key, the model is slow, or it
    /// answers with something unusable — every caller falls back to the person's own words.
    static func searchTerms(for text: String) async -> String? {
        let system = """
        You turn a map enthusiast's request into keywords for an open-data catalogue search \
        (Socrata and ArcGIS Hub). Reply with ONLY the keywords, 2 to 5 words, no punctuation, no \
        explanation, no quotes. Keep place names. Drop filler words. Use the words a government \
        data portal would use: prefer "bicycle network" over "bike lanes near me", "parcels" over \
        "who owns what". If the request is already good keywords, repeat it unchanged.
        """
        guard let reply = await chat(system: system, user: text, maxTokens: 32) else { return nil }
        let terms = reply.trimmingCharacters(in: .whitespacesAndNewlines)
            .replacingOccurrences(of: "\"", with: "")
            .replacingOccurrences(of: "\n", with: " ")
        // A model that starts explaining itself is not answering the question.
        guard !terms.isEmpty, terms.count < 80, terms.split(separator: " ").count <= 8 else { return nil }
        return terms
    }

    /// The best few of what came back, in order, with one line each on why. Returns nil rather
    /// than guessing, so the caller keeps the catalogue's own ranking.
    static func rank(_ items: [Item], for text: String) async -> [Ranked]? {
        guard items.count > 1 else { return nil }
        let listed = items.prefix(20).enumerated()
            .map { "\($0.offset): \($0.element.title) — \($0.element.source) — \($0.element.blurb.prefix(120))" }
            .joined(separator: "\n")
        let system = """
        You help someone choose a map dataset. Given their request and a numbered list, reply with \
        ONLY lines of the form `index|one short sentence saying what this would show them`. At \
        most 5 lines, best first. Drop anything irrelevant. No preamble, no numbering of your own, \
        no markdown. Write for someone who knows nothing about GIS.
        """
        let user = "Request: \(text)\n\nDatasets:\n\(listed)"
        guard let reply = await chat(system: system, user: user, maxTokens: 300) else { return nil }
        var out: [Ranked] = []
        for line in reply.split(whereSeparator: \.isNewline) {
            let parts = line.split(separator: "|", maxSplits: 1).map(String.init)
            guard parts.count == 2, let i = Int(parts[0].trimmingCharacters(in: .whitespaces)),
                  items.indices.contains(i)
            else { continue }
            out.append(Ranked(index: i, why: parts[1].trimmingCharacters(in: .whitespaces)))
        }
        return out.isEmpty ? nil : out
    }

    struct Ranked {
        let index: Int
        let why: String
    }

    /// What `rank` needs to know about a result, so this file does not depend on Catalog's shape.
    struct Item {
        let title: String
        let source: String
        let blurb: String
    }

    // MARK: The call

    private static func chat(system: String, user: String, maxTokens: Int) async -> String? {
        guard let key, let endpoint else { return nil }
        var r = URLRequest(url: endpoint)
        r.httpMethod = "POST"
        r.timeoutInterval = 12
        r.setValue("Bearer \(key)", forHTTPHeaderField: "Authorization")
        r.setValue("application/json", forHTTPHeaderField: "Content-Type")
        // OpenRouter asks callers to identify themselves.
        r.setValue("https://nimblemap.com", forHTTPHeaderField: "HTTP-Referer")
        r.setValue("NimbleMap D (spike)", forHTTPHeaderField: "X-Title")
        let body: [String: Any] = [
            "model": model,
            "max_tokens": maxTokens,
            "temperature": 0,
            "messages": [
                ["role": "system", "content": system],
                ["role": "user", "content": user],
            ],
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: body) else { return nil }
        r.httpBody = data
        guard let (out, response) = try? await URLSession.shared.data(for: r),
              let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode),
              let root = try? JSONSerialization.jsonObject(with: out) as? [String: Any],
              let choices = root["choices"] as? [[String: Any]],
              let message = choices.first?["message"] as? [String: Any],
              let content = message["content"] as? String
        else { return nil }
        return content
    }
}
