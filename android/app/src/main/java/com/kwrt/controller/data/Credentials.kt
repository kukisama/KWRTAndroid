package com.kwrt.controller.data

import android.content.Context
import android.content.SharedPreferences

/**
 * 登录凭据持久化。存到应用私有 SharedPreferences（其它应用读不到，但 root 设备能看）。
 * 字段对应 LuCI ubus session.login + uci 配置：scheme/host/port/username/password/uci-config 名。
 */
data class Credentials(
    val scheme: String = "http",
    val host: String = "",
    val port: Int? = null,
    val username: String = "root",
    val password: String = "",
    val config: String = "passwall",
    /** 首次成功登录时记录的 Wi-Fi SSID；后续自动登录会检测当前 SSID 是否匹配。 */
    val wifiSsid: String? = null,
) {
    fun isValid(): Boolean = host.isNotBlank() && username.isNotBlank()

    fun baseUrl(): String {
        val n = normalized()
        val p = n.port?.let { ":$it" } ?: ""
        return "${n.scheme}://${n.host}$p"
    }

    /**
     * 从用户输入的 host 智能解析 scheme/host/port：
     *  - 包含 `https://` 前缀 → https
     *  - 否则一律 http（与 PC 版一致；想用 https 就在地址里明写）
     *  - host 中带 `:port` 会被提取，否则按 scheme 默认 80 / 443
     */
    fun normalized(): Credentials {
        var raw = host.trim()
        var sch = "http"
        if (raw.startsWith("https://", ignoreCase = true)) {
            sch = "https"
            raw = raw.substring(8)
        } else if (raw.startsWith("http://", ignoreCase = true)) {
            raw = raw.substring(7)
        }
        raw = raw.trimEnd('/')
        var h = raw
        var p: Int? = port
        val colon = raw.lastIndexOf(':')
        // 防止 IPv6 误判（IPv6 多冒号，简单处理：仅当 host 不含 '[' 时才剥端口）
        if (colon > 0 && !raw.contains('[')) {
            raw.substring(colon + 1).toIntOrNull()?.let {
                h = raw.substring(0, colon)
                p = it
            }
        }
        return copy(scheme = sch, host = h, port = p)
    }
}

class CredentialStore(ctx: Context) {
    private val sp: SharedPreferences =
        ctx.applicationContext.getSharedPreferences("kwrt_creds", Context.MODE_PRIVATE)

    fun load(): Credentials? {
        if (!sp.getBoolean("saved", false)) return null
        return Credentials(
            scheme = sp.getString("scheme", "http") ?: "http",
            host = sp.getString("host", "") ?: "",
            port = sp.getInt("port", -1).takeIf { it > 0 },
            username = sp.getString("username", "root") ?: "root",
            password = sp.getString("password", "") ?: "",
            config = sp.getString("config", "passwall") ?: "passwall",
            wifiSsid = sp.getString("wifi_ssid", null),
        )
    }

    fun save(c: Credentials) {
        sp.edit()
            .putBoolean("saved", true)
            .putString("scheme", c.scheme)
            .putString("host", c.host)
            .apply {
                if (c.port != null) putInt("port", c.port) else remove("port")
            }
            .putString("username", c.username)
            .putString("password", c.password)
            .putString("config", c.config)
            .apply {
                if (c.wifiSsid != null) putString("wifi_ssid", c.wifiSsid) else remove("wifi_ssid")
            }
            .apply()
    }

    fun clear() { sp.edit().clear().apply() }
}
