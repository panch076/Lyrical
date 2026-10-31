import Foundation

/// Fetches time-synced lyrics from LRCLIB (https://lrclib.net), falling back to
/// NetEase Cloud Music when it is down or has no synced lyrics for a song.
actor LyricsService {
    private struct Record: Decodable {
        let duration: Double?
        let instrumental: Bool?
        let plainLyrics: String?
        let syncedLyrics: String?

        var hasSynced: Bool { !(syncedLyrics ?? "").isEmpty }
        var hasPlain: Bool { !(plainLyrics ?? "").isEmpty }
    }

    private var cache: [String: LyricsState] = [:]
    private let session: URLSession

    init() {
        let config = URLSessionConfiguration.default
        config.httpAdditionalHeaders = ["User-Agent": "Lyrical/1.0 (macOS lyrics overlay)"]
        config.timeoutIntervalForRequest = 12
        // Once NetEase has set a cookie, its search returns unrelated songs.
        config.httpCookieAcceptPolicy = .never
        config.httpShouldSetCookies = false
        config.httpCookieStorage = nil
        session = URLSession(configuration: config)
    }

    func lyrics(for track: Track) async -> LyricsState {
        let key = "\(track.artist)|\(track.title)".lowercased()
        if let cached = cache[key] { return cached }

        var networkFailed = false
        var records: [Record] = []

        if let duration = track.duration, !track.album.isEmpty {
            let (record, failed) = await get(track: track, duration: duration)
            networkFailed = networkFailed || failed
            if let record { records.append(record) }
        }
        if !networkFailed, !records.contains(where: \.hasSynced) {
            let (found, failed) = await search(title: track.title, artist: track.artist)
            networkFailed = networkFailed || failed
            records += found
        }
        if !networkFailed, !records.contains(where: \.hasSynced) {
            let title = Self.cleanTitle(track.title)
            let artist = Self.primaryArtist(track.artist)
            if title != track.title || artist != track.artist {
                let (found, failed) = await search(title: title, artist: artist)
                networkFailed = networkFailed || failed
                records += found
            }
        }

        var result = Self.choose(from: records, duration: track.duration)
        var fallbackFailed = false
        if result.syncedLines == nil, result != .instrumental {
            let (record, failed) = await fallback(for: track)
            fallbackFailed = failed
            if let record {
                records.append(record)
                result = Self.choose(from: records, duration: track.duration)
            }
        }

        if networkFailed && result == .unavailable { return .serviceDown }
        if !fallbackFailed { cache[key] = result }
        return result
    }

    // MARK: - Requests

    private func get(track: Track, duration: TimeInterval) async -> (Record?, Bool) {
        var components = URLComponents(string: "https://lrclib.net/api/get")!
        components.queryItems = [
            .init(name: "track_name", value: track.title),
            .init(name: "artist_name", value: track.artist),
            .init(name: "album_name", value: track.album),
            .init(name: "duration", value: String(Int(duration.rounded()))),
        ]
        do {
            let (data, response) = try await session.data(from: components.url!)
            guard (response as? HTTPURLResponse)?.statusCode == 200 else { return (nil, Self.isOutage(response)) }
            return (try? JSONDecoder().decode(Record.self, from: data), false)
        } catch {
            return (nil, true)
        }
    }

    private func search(title: String, artist: String) async -> ([Record], Bool) {
        var components = URLComponents(string: "https://lrclib.net/api/search")!
        components.queryItems = [
            .init(name: "track_name", value: title),
            .init(name: "artist_name", value: artist),
        ]
        do {
            let (data, response) = try await session.data(from: components.url!)
            guard (response as? HTTPURLResponse)?.statusCode == 200 else { return ([], Self.isOutage(response)) }
            return ((try? JSONDecoder().decode([Record].self, from: data)) ?? [], false)
        } catch {
            return ([], true)
        }
    }

    /// Server errors and rate limiting say nothing about whether a song has lyrics.
    private static func isOutage(_ response: URLResponse) -> Bool {
        guard let status = (response as? HTTPURLResponse)?.statusCode else { return true }
        return status >= 500 || status == 429
    }

    // MARK: - Fallback (NetEase Cloud Music)

    private struct NetEaseSearch: Decodable {
        struct Results: Decodable { let songs: [NetEaseSong]? }
        let code: Int
        let result: Results?
    }

    private struct NetEaseSong: Decodable {
        struct Artist: Decodable { let name: String }
        let id: Int
        let name: String
        let duration: Double
        let artists: [Artist]

        var seconds: TimeInterval { duration / 1000 }
    }

    private struct NetEaseLyric: Decodable {
        struct Body: Decodable { let lyric: String? }
        let lrc: Body?
    }

    /// Used when LRCLIB is down or has no synced lyrics. The API is undocumented,
    /// so anything unexpected is reported as a failure rather than "no lyrics".
    private func fallback(for track: Track) async -> (Record?, Bool) {
        var components = URLComponents(string: "https://music.163.com/api/search/get")!
        components.queryItems = [
            .init(name: "s", value: "\(Self.cleanTitle(track.title)) \(Self.primaryArtist(track.artist))"),
            .init(name: "type", value: "1"),
            .init(name: "limit", value: "20"),
        ]
        do {
            let (data, response) = try await session.data(from: components.url!)
            guard (response as? HTTPURLResponse)?.statusCode == 200,
                  let found = try? JSONDecoder().decode(NetEaseSearch.self, from: data),
                  found.code == 200 else { return (nil, true) }
            // The closest match can have no lyrics while a re-release of it does.
            for song in Self.matches(for: track, in: found.result?.songs ?? []).prefix(3) {
                if let lrc = try await fallbackLyrics(id: song.id) {
                    return (Record(duration: song.seconds, instrumental: nil, plainLyrics: nil, syncedLyrics: lrc), false)
                }
            }
            return (nil, false)
        } catch {
            return (nil, true)
        }
    }

    private func fallbackLyrics(id: Int) async throws -> String? {
        var components = URLComponents(string: "https://music.163.com/api/song/lyric")!
        components.queryItems = [
            .init(name: "id", value: String(id)),
            .init(name: "lv", value: "1"),
        ]
        let (data, _) = try await session.data(from: components.url!)
        guard let raw = (try? JSONDecoder().decode(NetEaseLyric.self, from: data))?.lrc?.lyric else { return nil }
        let lrc = Self.strippingCredits(raw)
        // Unsynced lyrics come back with one placeholder timestamp, if any.
        guard let lines = LRCParser.parse(lrc), Set(lines.map(\.time)).count >= 3 else { return nil }
        return lrc
    }

    /// Candidates that are really this recording, best first. A wrong song is
    /// worse than none, so title, artist and length must all agree.
    private static func matches(for track: Track, in songs: [NetEaseSong]) -> [NetEaseSong] {
        let exactTitle = normalized(track.title)
        let title = normalized(cleanTitle(track.title))
        let artist = normalized(primaryArtist(track.artist))
        guard !title.isEmpty, !artist.isEmpty else { return [] }
        let isVersion = isDistinctVersion(track.title)

        func isExact(_ song: NetEaseSong) -> Bool { normalized(song.name) == exactTitle }
        func distance(_ song: NetEaseSong) -> Double {
            track.duration.map { abs($0 - song.seconds) } ?? 0
        }
        return songs
            .filter { song in
                song.artists.contains { normalized($0.name) == artist }
                    && distance(song) < 15
                    && (isExact(song) || (normalized(cleanTitle(song.name)) == title
                        && (isVersion || !isDistinctVersion(song.name))))
            }
            .sorted {
                if isExact($0) != isExact($1) { return isExact($0) }
                return distance($0) < distance($1)
            }
    }

    private static func normalized(_ text: String) -> String {
        String(text.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil)
            .unicodeScalars.filter(CharacterSet.alphanumerics.contains).map(Character.init))
    }

    /// Versions whose words or timing differ, unlike a remaster or an explicit tag.
    private static func isDistinctVersion(_ title: String) -> Bool {
        title.range(of: "\\b(remix|live|acoustic|instrumental|karaoke|cover|demo)\\b",
                    options: [.regularExpression, .caseInsensitive]) != nil
    }

    /// NetEase mixes credits in with the lyrics, e.g. "[00:00.00] 作词 : Someone".
    private static func strippingCredits(_ lrc: String) -> String {
        let credit = "^\\s*(\\p{Han}{1,10}\\s*[:：]\\s|(作词|作曲|编曲|词|曲|制作人|监制)\\s*[:：]"
            + "|(artist|song|title|album|lyrics?|lyricist|composer|producer|arranger|written by)\\s*[:：]\\s"
            + "|[\\p{L}&/ ]{1,30} [:：] )"
        return lrc.components(separatedBy: .newlines).filter { line in
            let text = line.replacingOccurrences(of: "^(\\[[^\\]]*\\])+", with: "", options: .regularExpression)
            return text.range(of: credit, options: [.regularExpression, .caseInsensitive]) == nil
        }.joined(separator: "\n")
    }

    // MARK: - Matching

    private static func choose(from records: [Record], duration: TimeInterval?) -> LyricsState {
        func distance(_ r: Record) -> Double {
            guard let duration, let d = r.duration else { return 0 }
            return abs(d - duration)
        }
        let synced = records.filter(\.hasSynced)
            .filter { duration == nil || $0.duration == nil || distance($0) < 15 }
            .sorted { distance($0) < distance($1) }
        if let best = synced.first, let lines = LRCParser.parse(best.syncedLyrics ?? ""), !lines.isEmpty {
            return .synced(lines)
        }
        if records.first?.instrumental == true { return .instrumental }
        if let plain = records.first(where: \.hasPlain)?.plainLyrics {
            return .plain(plain.components(separatedBy: .newlines))
        }
        return .unavailable
    }

    static func cleanTitle(_ title: String) -> String {
        let keywords = "feat|ft\\.|with|remaster|version|edit|mix|live|mono|stereo|deluxe|bonus|explicit|official|video|audio|lyrics|visualizer"
        var result = title.replacingOccurrences(
            of: "\\s*[\\(\\[][^\\)\\]]*(\(keywords))[^\\)\\]]*[\\)\\]]",
            with: "", options: [.regularExpression, .caseInsensitive])
        result = result.replacingOccurrences(
            of: "\\s+-\\s+.*(\(keywords)).*$",
            with: "", options: [.regularExpression, .caseInsensitive])
        return result.trimmingCharacters(in: .whitespaces)
    }

    static func primaryArtist(_ artist: String) -> String {
        let separators = [", ", " & ", " feat. ", " feat ", " ft. ", " x ", " and "]
        var result = artist
        for separator in separators {
            if let range = result.range(of: separator, options: .caseInsensitive) {
                result = String(result[..<range.lowerBound])
            }
        }
        return result.trimmingCharacters(in: .whitespaces)
    }
}

enum LRCParser {
    private static let tag = try! NSRegularExpression(pattern: "\\[(\\d+):(\\d+(?:[.:]\\d+)?)\\]")

    static func parse(_ lrc: String) -> [LyricLine]? {
        var entries: [(TimeInterval, String)] = []
        for raw in lrc.components(separatedBy: .newlines) {
            let ns = raw as NSString
            let matches = tag.matches(in: raw, range: NSRange(location: 0, length: ns.length))
            guard let last = matches.last else { continue }
            let text = ns.substring(from: last.range.location + last.range.length)
                .replacingOccurrences(of: "<\\d+:\\d+(?:\\.\\d+)?>", with: "", options: .regularExpression)
                .trimmingCharacters(in: .whitespaces)
            for match in matches {
                let minutes = Double(ns.substring(with: match.range(at: 1))) ?? 0
                let seconds = Double(ns.substring(with: match.range(at: 2)).replacingOccurrences(of: ":", with: ".")) ?? 0
                entries.append((minutes * 60 + seconds, text))
            }
        }
        guard !entries.isEmpty else { return nil }
        entries.sort { $0.0 < $1.0 }

        // Collapse consecutive gaps so instrumental breaks render once.
        var lines: [LyricLine] = []
        for (time, text) in entries {
            if text.isEmpty, lines.last?.isGap == true { continue }
            lines.append(LyricLine(id: lines.count, time: time, text: text))
        }
        while lines.last?.isGap == true { lines.removeLast() }
        return lines
    }
}
