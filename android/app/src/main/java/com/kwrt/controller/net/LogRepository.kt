package com.kwrt.controller.net

/**
 * 读取路由器系统/PassWall 日志。
 * 复刻 PC 端 `passwall::read_log` 的容错策略：
 *   1) 优先 cat /tmp/log/<config>.log
 *   2) 回退 /var/log/<config>.log
 *   3) 都没有 → logread -e <config>
 *
 * 全部走 shell 一条命令，单次 ubus 调用即可拿结果。
 */
object LogRepository {

    /**
     * @param lines 取末尾多少行
     * @return 日志原文（已 tail -n lines）；返回空字符串表示没拿到任何输出。
     */
    suspend fun readLog(client: LuciClient, config: String, lines: Int = 500): String {
        val n = lines.coerceIn(50, 5000)
        // sh 内联三段回退；用 || 串起来。grep -F 防止 config 名里的特殊字符。
        val script = buildString {
            append("LOG_CFG=").append(shellQuote(config)).append('\n')
            append("LINES=").append(n).append('\n')
            append("""
                if [ -s /tmp/log/${'$'}LOG_CFG.log ]; then
                    tail -n ${'$'}LINES /tmp/log/${'$'}LOG_CFG.log
                elif [ -s /var/log/${'$'}LOG_CFG.log ]; then
                    tail -n ${'$'}LINES /var/log/${'$'}LOG_CFG.log
                else
                    logread -e "${'$'}LOG_CFG" | tail -n ${'$'}LINES
                fi
            """.trimIndent())
        }
        val r = client.shell(script)
        if (!r.ok()) {
            error("读取日志失败 (exit=${r.code}) ${r.stderr.ifBlank { r.stdout }.take(200)}")
        }
        return r.stdout
    }

    /** 系统日志（dmesg / logread 全量），不按 config 过滤。 */
    suspend fun readSystemLog(client: LuciClient, lines: Int = 500): String {
        val n = lines.coerceIn(50, 5000)
        val r = client.shell("logread | tail -n $n")
        if (!r.ok()) {
            error("logread 失败 (exit=${r.code}) ${r.stderr.take(200)}")
        }
        return r.stdout
    }

    private fun shellQuote(s: String): String = "'" + s.replace("'", "'\\''") + "'"
}
