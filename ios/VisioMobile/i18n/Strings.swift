import Foundation

enum Strings {
    static let supportedLangs = ["en", "fr", "de", "es", "it", "nl"]
    private static var translations: [String: [String: String]] = [:]

    static func initialize() {
        for lang in supportedLangs {
            guard let url = Bundle.main.url(forResource: lang, withExtension: "json", subdirectory: "i18n"),
                  let data = try? Data(contentsOf: url),
                  let dict = try? JSONSerialization.jsonObject(with: data) as? [String: String]
            else { continue }
            translations[lang] = dict
        }
    }

    static func t(_ key: String, lang: String) -> String {
        return translations[lang]?[key] ?? translations["en"]?[key] ?? key
    }

    static func detectSystemLang() -> String {
        let sysLang = Locale.current.language.languageCode?.identifier ?? "en"
        return supportedLangs.contains(sysLang) ? sysLang : "en"
    }
}
