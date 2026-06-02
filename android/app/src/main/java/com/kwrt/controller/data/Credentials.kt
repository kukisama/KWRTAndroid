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
) {
    fun isValid(): Boolean = host.isNotBlank() && username.isNotBlank()

    fun baseUrl(): String {
        val p = port?.let { ":$it" } ?: ""
        return "$scheme://${host.trim()}$p"
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
            .apply()
    }

    fun clear() { sp.edit().clear().apply() }
}
