package com.kwrt.controller.net

import com.kwrt.controller.data.Credentials
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONArray
import org.json.JSONObject
import java.security.cert.X509Certificate
import java.util.concurrent.TimeUnit
import javax.net.ssl.SSLContext
import javax.net.ssl.X509TrustManager

/**
 * 直连 OpenWrt LuCI 的 /ubus 端点。和 Rust 端 [client.rs] 行为对齐：
 *   1. session.login → 拿 ubus_rpc_session token
 *   2. file.exec sh -c "<script>" 跑 shell，回 {code, stdout, stderr}
 *
 * 为了和家用路由器自签 HTTPS / 旧证书兼容，OkHttp 这里关掉证书校验
 * （等价 Rust 那边的 `accept_invalid_certs`；LAN 信任前提下）。
 */
class LuciClient private constructor(
    private val baseUrl: String,
    private val http: OkHttpClient,
    private val session: String,
) {
    /** 跑 sh -c "<script>"，行为同 Rust 端 LuciClient::shell。 */
    suspend fun shell(script: String): ShellResult {
        val params = JSONObject().apply {
            put("command", "sh")
            put("params", JSONArray(listOf("-c", script)))
        }
        val res = ubusCall("file", "exec", params)
        return ShellResult(
            code = res.optInt("code", -1),
            stdout = res.optString("stdout", ""),
            stderr = res.optString("stderr", ""),
        )
    }

    private suspend fun ubusCall(obj: String, method: String, params: JSONObject): JSONObject =
        withContext(Dispatchers.IO) {
            val body = JSONObject().apply {
                put("jsonrpc", "2.0")
                put("id", 1)
                put("method", "call")
                put("params", JSONArray(listOf(session, obj, method, params)))
            }
            val req = Request.Builder()
                .url("$baseUrl/ubus")
                .post(body.toString().toRequestBody(JSON))
                .build()
            http.newCall(req).execute().use { resp ->
                if (!resp.isSuccessful) error("HTTP ${resp.code} from /ubus")
                val text = resp.body?.string().orEmpty()
                val v = JSONObject(text)
                v.optJSONObject("error")?.let { error("ubus $obj.$method 出错: $it") }
                val result = v.optJSONArray("result") ?: error("ubus 响应缺少 result: $text")
                val code = if (result.length() > 0) result.optInt(0, -1) else -1
                if (code != 0) error("ubus $obj.$method 返回码 $code（4=对象不存在，6=权限）")
                if (result.length() < 2) JSONObject() else result.optJSONObject(1) ?: JSONObject()
            }
        }

    companion object {
        private val JSON = "application/json; charset=utf-8".toMediaType()
        private const val ANON = "00000000000000000000000000000000"

        suspend fun connect(c: Credentials): LuciClient = withContext(Dispatchers.IO) {
            require(c.isValid()) { "host 不能为空" }
            val http = buildHttp()
            val loginBody = JSONObject().apply {
                put("jsonrpc", "2.0")
                put("id", 1)
                put("method", "call")
                put("params", JSONArray(listOf(
                    ANON, "session", "login",
                    JSONObject().apply {
                        put("username", c.username)
                        put("password", c.password)
                        put("timeout", 0)
                    },
                )))
            }
            val base = c.baseUrl()
            val req = Request.Builder()
                .url("$base/ubus")
                .post(loginBody.toString().toRequestBody(JSON))
                .build()
            val token = http.newCall(req).execute().use { resp ->
                if (!resp.isSuccessful) error("无法访问 ${base}（${resp.code}）；路由器可达性 / 端口是否正确？")
                val text = resp.body?.string().orEmpty()
                val v = runCatching { JSONObject(text) }
                    .getOrElse { error("/ubus 不是 JSON（uhttpd 可能未启用 ubus）: ${text.take(200)}") }
                v.optJSONObject("error")?.let { error("ubus 登录错误: $it") }
                val result = v.optJSONArray("result")
                    ?: error("ubus 响应缺少 result: $text")
                val code = if (result.length() > 0) result.optInt(0, -1) else -1
                if (code != 0) error("登录失败 (code=$code)：账号或密码错误，或 ACL 不允许登录")
                val payload = result.optJSONObject(1) ?: error("ubus 响应缺少 session 对象")
                payload.optString("ubus_rpc_session").ifBlank { error("响应缺少 ubus_rpc_session") }
            }
            LuciClient(base, http, token)
        }

        private fun buildHttp(): OkHttpClient {
            // 关闭证书校验，等价 Rust accept_invalid_certs。
            // 仅用于 LAN 路由器；公网 HTTPS 场景请勿沿用此模式。
            val tm = object : X509TrustManager {
                override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?) {}
                override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?) {}
                override fun getAcceptedIssuers(): Array<X509Certificate> = arrayOf()
            }
            val ssl = SSLContext.getInstance("TLS").apply {
                init(null, arrayOf(tm), java.security.SecureRandom())
            }
            return OkHttpClient.Builder()
                .connectTimeout(5, TimeUnit.SECONDS)
                .readTimeout(60, TimeUnit.SECONDS)
                .writeTimeout(30, TimeUnit.SECONDS)
                .sslSocketFactory(ssl.socketFactory, tm)
                .hostnameVerifier { _, _ -> true }
                .build()
        }
    }
}

data class ShellResult(val code: Int, val stdout: String, val stderr: String) {
    fun ok(): Boolean = code == 0
}
