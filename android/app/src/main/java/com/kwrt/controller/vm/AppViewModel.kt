package com.kwrt.controller.vm

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.kwrt.controller.data.CredentialStore
import com.kwrt.controller.data.Credentials
import com.kwrt.controller.net.AclRepository
import com.kwrt.controller.net.AclRow
import com.kwrt.controller.net.LogRepository
import com.kwrt.controller.net.LuciClient
import com.kwrt.controller.net.WifiHelper
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

sealed interface Screen {
    data object Login : Screen
    data object Clients : Screen
}

/** 底部选项卡；当前只有 ACL + 系统日志，未来可动态扩。 */
enum class Tab { Clients, Logs }

/** UI 顶层状态：当前页 / 当前凭据 / ACL 列表 / 加载中 / 错误 / Snackbar 文案 / "有未应用"标记。 */
data class UiState(
    val screen: Screen = Screen.Login,
    val creds: Credentials = Credentials(),
    val remember: Boolean = true,
    val autoLoggingIn: Boolean = false,
    val loginError: String? = null,
    val rows: List<AclRow> = emptyList(),
    val loading: Boolean = false,
    val pending: Boolean = false,    // 有未应用的开关切换
    val applying: Boolean = false,
    val snackbar: String? = null,
    /** 连接路由器失败（包括登录/刷新），列表页显示“网络中断”空态。 */
    val networkDown: Boolean = false,
    // ---- 底部选项卡 + 日志 ----
    val tab: Tab = Tab.Clients,
    val logText: String = "",
    val logLoading: Boolean = false,
    val logError: String? = null,
    /** true=仅看 PassWall(<config>) 日志；false=看系统全量 logread。 */
    val logPasswallOnly: Boolean = true,
    /** 自动刷新开关（对齐 PC 端默认 3s 一轮）。 */
    val logAutoRefresh: Boolean = true,
)

class AppViewModel(app: Application) : AndroidViewModel(app) {
    private val store = CredentialStore(app)
    private var client: LuciClient? = null

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    /** 日志页轮询 Job；离开页/退出时 cancel。 */
    private var logPollJob: Job? = null
    private val LOG_POLL_INTERVAL_MS = 3000L

    init {
        // 启动时若有保存的凭据，自动登录
        val saved = store.load()
        if (saved != null && saved.isValid()) {
            _state.update { it.copy(creds = saved, remember = true, autoLoggingIn = true) }
            login(saved, remember = true, isAuto = true)
        }
    }

    fun updateCreds(transform: (Credentials) -> Credentials) {
        _state.update { it.copy(creds = transform(it.creds), loginError = null) }
    }
    fun setRemember(on: Boolean) { _state.update { it.copy(remember = on) } }
    fun dismissSnackbar() { _state.update { it.copy(snackbar = null) } }

    /** 打开系统 Wi-Fi 面板（连接失败时用户点“切换 Wi-Fi”调用）。 */
    fun openWifiSettings() {
        WifiHelper.openWifiSettings(getApplication())
    }

    fun selectTab(t: Tab) {
        _state.update { it.copy(tab = t) }
        if (t == Tab.Logs) {
            if (_state.value.logText.isEmpty() && !_state.value.logLoading) loadLog()
            startLogPolling()
        } else {
            stopLogPolling()
        }
    }

    fun setLogPasswallOnly(on: Boolean) {
        _state.update { it.copy(logPasswallOnly = on, logText = "") }
        loadLog()
    }

    fun setLogAutoRefresh(on: Boolean) {
        _state.update { it.copy(logAutoRefresh = on) }
        if (on && _state.value.tab == Tab.Logs) startLogPolling() else stopLogPolling()
    }

    private fun startLogPolling() {
        if (!_state.value.logAutoRefresh) return
        if (logPollJob?.isActive == true) return
        logPollJob = viewModelScope.launch {
            while (isActive) {
                delay(LOG_POLL_INTERVAL_MS)
                // 守卫：离开页 / 关开关 / 未登录都不拉
                val s = _state.value
                if (s.tab != Tab.Logs || !s.logAutoRefresh || client == null) break
                loadLog(silent = true)
            }
        }
    }

    private fun stopLogPolling() {
        logPollJob?.cancel()
        logPollJob = null
    }

    /**
     * @param silent true=轮询调起，不设 logLoading（避免 UI 每 3s 闪 spinner）。
     */
    fun loadLog(silent: Boolean = false) {
        val cli = client ?: run {
            _state.update { it.copy(logError = "尚未登录") }
            return
        }
        val cfg = _state.value.creds.config
        val passwallOnly = _state.value.logPasswallOnly
        viewModelScope.launch {
            if (!silent) _state.update { it.copy(logLoading = true, logError = null) }
            runCatching {
                if (passwallOnly) LogRepository.readLog(cli, cfg)
                else LogRepository.readSystemLog(cli)
            }
                .onSuccess { txt ->
                    _state.update { it.copy(logLoading = false, logText = txt, logError = null) }
                }
                .onFailure { e ->
                    // 静默轮询失败不清现有文本，只记录错误；手动/首加载才覆盖。
                    _state.update {
                        it.copy(
                            logLoading = false,
                            logError = e.message ?: "读取失败",
                        )
                    }
                }
        }
    }

    fun login(c: Credentials = _state.value.creds, remember: Boolean = _state.value.remember, isAuto: Boolean = false) {
        if (!c.isValid()) {
            _state.update { it.copy(loginError = "请填写路由器地址", autoLoggingIn = false) }
            return
        }
        val cn = c.normalized()
        viewModelScope.launch {
            _state.update { it.copy(loading = true, loginError = null) }
            runCatching { LuciClient.connect(cn) }
                .onSuccess { cli ->
                    client = cli
                    if (remember) store.save(cn) else store.clear()
                    _state.update {
                        it.copy(
                            screen = Screen.Clients,
                            creds = cn,
                            remember = remember,
                            loading = false,
                            autoLoggingIn = false,
                            loginError = null,
                            networkDown = false,
                        )
                    }
                    refresh()
                }
                .onFailure { e ->
                    _state.update {
                        it.copy(
                            loading = false,
                            autoLoggingIn = false,
                            loginError = "连接路由器失败，可能不在对应网络。(${e.message ?: "未知错误"})",
                        )
                    }
                }
        }
    }

    fun logout() {
        // 只断开会话，凭据依然保留在 SharedPreferences，可以直接点登录重连。
        stopLogPolling()
        client = null
        _state.update {
            it.copy(
                screen = Screen.Login,
                rows = emptyList(),
                pending = false,
                networkDown = false,
                tab = Tab.Clients,
                logText = "",
                logError = null,
                snackbar = "已退出登录，凭据已保留",
            )
        }
    }

    fun refresh(manual: Boolean = false) {
        val cli = client ?: return
        val cfg = _state.value.creds.config
        viewModelScope.launch {
            _state.update { it.copy(loading = true) }
            runCatching { AclRepository.read(cli, cfg) }
                .onSuccess { rows ->
                    _state.update {
                        it.copy(
                            loading = false, rows = rows, pending = false,
                            networkDown = false,
                            snackbar = if (manual) "✓ 刷新成功，共 ${rows.size} 条规则" else it.snackbar,
                        )
                    }
                }
                .onFailure { e ->
                    // 连接失败 → 清空缓存列表，进入“网络中断”空态（不展示老数据）
                    _state.update {
                        it.copy(
                            loading = false,
                            rows = emptyList(),
                            pending = false,
                            networkDown = true,
                            snackbar = "连接路由器失败：${e.message ?: "请检查网络"}",
                        )
                    }
                }
        }
    }

    /** 切换某一行启用状态。乐观更新 + 失败回滚；写 uci 后需点“应用配置”一起 reload。 */
    fun toggle(section: String) {
        val cli = client ?: return
        val cur = _state.value
        val idx = cur.rows.indexOfFirst { it.section == section }.takeIf { it >= 0 } ?: return
        val old = cur.rows[idx]
        val next = !old.enabled
        // 乐观更新
        _state.update { s ->
            s.copy(rows = s.rows.toMutableList().also { it[idx] = old.copy(enabled = next) }, pending = true)
        }
        viewModelScope.launch {
            runCatching { AclRepository.setEnabled(cli, cur.creds.config, section, next) }
                .onFailure { e ->
                    // 回滚
                    _state.update { s ->
                        s.copy(
                            rows = s.rows.toMutableList().also { it[idx] = old },
                            snackbar = "切换失败：${e.message}",
                        )
                    }
                }
        }
    }

    fun applyConfig() {
        val cli = client ?: return
        val cfg = _state.value.creds.config
        viewModelScope.launch {
            _state.update { it.copy(applying = true) }
            runCatching { AclRepository.applyReload(cli, cfg) }
                .onSuccess {
                    _state.update {
                        it.copy(applying = false, pending = false,
                            snackbar = "✓ 已触发后台 reload（约 10–30 秒生效）")
                    }
                }
                .onFailure { e ->
                    _state.update { it.copy(applying = false, snackbar = "应用失败：${e.message}") }
                }
        }
    }
}
