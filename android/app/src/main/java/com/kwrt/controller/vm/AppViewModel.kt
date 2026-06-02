package com.kwrt.controller.vm

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.kwrt.controller.data.CredentialStore
import com.kwrt.controller.data.Credentials
import com.kwrt.controller.net.AclRepository
import com.kwrt.controller.net.AclRow
import com.kwrt.controller.net.LuciClient
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

sealed interface Screen {
    data object Login : Screen
    data object Clients : Screen
}

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
)

class AppViewModel(app: Application) : AndroidViewModel(app) {
    private val store = CredentialStore(app)
    private var client: LuciClient? = null

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

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

    fun login(c: Credentials = _state.value.creds, remember: Boolean = _state.value.remember, isAuto: Boolean = false) {
        if (!c.isValid()) {
            _state.update { it.copy(loginError = "请填写路由器地址", autoLoggingIn = false) }
            return
        }
        viewModelScope.launch {
            _state.update { it.copy(loading = true, loginError = null) }
            runCatching { LuciClient.connect(c) }
                .onSuccess { cli ->
                    client = cli
                    if (remember) store.save(c) else store.clear()
                    _state.update {
                        it.copy(
                            screen = Screen.Clients,
                            creds = c,
                            remember = remember,
                            loading = false,
                            autoLoggingIn = false,
                            loginError = null,
                        )
                    }
                    refresh()
                }
                .onFailure { e ->
                    _state.update {
                        it.copy(
                            loading = false,
                            autoLoggingIn = false,
                            loginError = if (isAuto) "自动登录失败：${e.message}" else (e.message ?: "登录失败"),
                        )
                    }
                }
        }
    }

    fun logout() {
        client = null
        store.clear()
        _state.update {
            it.copy(
                screen = Screen.Login,
                rows = emptyList(),
                pending = false,
                remember = false,
                creds = it.creds.copy(password = ""),
            )
        }
    }

    fun refresh() {
        val cli = client ?: return
        val cfg = _state.value.creds.config
        viewModelScope.launch {
            _state.update { it.copy(loading = true) }
            runCatching { AclRepository.read(cli, cfg) }
                .onSuccess { rows ->
                    _state.update { it.copy(loading = false, rows = rows, pending = false) }
                }
                .onFailure { e ->
                    _state.update { it.copy(loading = false, snackbar = "拉取失败：${e.message}") }
                }
        }
    }

    /** 切换某一行启用状态。乐观更新 + 失败回滚。 */
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
