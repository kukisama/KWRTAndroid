package com.kwrt.controller.net

/** ACL 行：和 Rust 端 acl::AclRule 对齐，但手机端只关心 section/enabled/sources/remarks/interface。 */
data class AclRow(
    val section: String,
    val enabled: Boolean,
    val sources: String,
    val remarks: String,
    val interfaceName: String,
)

object AclRepository {

    /**
     * 等价 Rust `acl::read`：
     *   uci show <config>，按行解析出每个 acl_rule section 的字段。
     */
    suspend fun read(client: LuciClient, config: String): List<AclRow> {
        val res = client.shell("uci show $config")
        if (!res.ok()) error("uci show $config 失败 (exit=${res.code}) ${res.stderr.take(200)}")
        val prefix = "$config."
        val sections = linkedMapOf<String, MutableMap<String, String>>()
        for (line in res.stdout.lineSequence()) {
            val l = line.trim()
            val body = l.removePrefix(prefix).takeIf { it != l } ?: continue
            val eq = body.indexOf('=').takeIf { it >= 0 } ?: continue
            val left = body.substring(0, eq)
            val right = body.substring(eq + 1).trim('\'')
            val dot = left.indexOf('.')
            if (dot < 0) {
                // passwall.cfgXXXX=acl_rule
                sections.getOrPut(left) { mutableMapOf() }[".type"] = right
            } else {
                val sec = left.substring(0, dot)
                val key = left.substring(dot + 1)
                sections.getOrPut(sec) { mutableMapOf() }[key] = right
            }
        }
        val out = mutableListOf<AclRow>()
        for ((name, fields) in sections) {
            if (fields[".type"] != "acl_rule") continue
            out += AclRow(
                section = name,
                enabled = fields["enabled"].let { it == "1" || it == "true" || it == null },
                sources = fields["sources"].orEmpty(),
                remarks = fields["remarks"].orEmpty(),
                interfaceName = fields["interface"].orEmpty(),
            )
        }
        return out
    }

    /** 仅切换 enabled，不 reload。等价 Rust `acl::update` 的最小子集。 */
    suspend fun setEnabled(client: LuciClient, config: String, section: String, on: Boolean) {
        require(section.isNotBlank())
        val v = if (on) "1" else "0"
        val script = "uci set $config.$section.enabled='$v' && uci commit $config && echo OK"
        val r = client.shell(script)
        if (!r.ok() || !r.stdout.contains("OK")) {
            error("切换失败 (exit=${r.code}) ${r.stderr.ifBlank { r.stdout }.take(200)}")
        }
    }

    /** 后台 fork reload，立即返回；等价 Rust `acl::reload`，避免 60s ubus 超时。 */
    suspend fun applyReload(client: LuciClient, config: String) {
        val script =
            "( setsid /etc/init.d/$config reload </dev/null >/dev/null 2>&1 & ) >/dev/null 2>&1 && echo OK"
        val r = client.shell(script)
        if (!r.ok() || !r.stdout.contains("OK")) {
            error("reload $config 失败 (exit=${r.code}) ${r.stderr.take(200)}")
        }
    }
}
