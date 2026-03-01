package io.visio.mobile.ui.i18n

import android.content.Context
import org.json.JSONObject

object Strings {
    private var translations: MutableMap<String, Map<String, String>> = mutableMapOf()
    val supportedLangs = listOf("en", "fr", "de", "es", "it", "nl")

    fun init(context: Context) {
        for (lang in supportedLangs) {
            try {
                val json = context.assets.open("i18n/$lang.json").bufferedReader().readText()
                val obj = JSONObject(json)
                val map = mutableMapOf<String, String>()
                obj.keys().forEach { key -> map[key] = obj.getString(key) }
                translations[lang] = map
            } catch (_: Exception) {}
        }
    }

    fun t(key: String, lang: String): String {
        return translations[lang]?.get(key) ?: translations["en"]?.get(key) ?: key
    }

    fun detectSystemLang(): String {
        val sysLang = java.util.Locale.getDefault().language
        return if (sysLang in supportedLangs) sysLang else "en"
    }
}
