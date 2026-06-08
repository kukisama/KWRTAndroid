package com.kwrt.controller.ui.screen

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Article
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.Logout
import androidx.compose.material.icons.outlined.Refresh
import androidx.compose.material.icons.outlined.Tune
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import com.kwrt.controller.vm.AppViewModel
import com.kwrt.controller.vm.Tab
import com.kwrt.controller.vm.UiState

/** 系统日志页：顶部切换 PassWall/系统全量，底部 Tab 一致。 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LogsScreen(state: UiState, vm: AppViewModel) {
    val clipboard = LocalClipboardManager.current
    val snackbarHostState = remember { SnackbarHostState() }

    LaunchedEffect(state.logError) {
        state.logError?.let { snackbarHostState.showSnackbar("读取失败：$it") }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text("系统日志", style = MaterialTheme.typography.titleLarge)
                        Text(
                            if (state.logPasswallOnly) "来源：PassWall (${state.creds.config})"
                            else "来源：logread 全量",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                },
                actions = {
                    IconButton(
                        onClick = {
                            if (state.logText.isNotEmpty()) {
                                clipboard.setText(AnnotatedString(state.logText))
                            }
                        },
                        enabled = state.logText.isNotEmpty(),
                    ) { Icon(Icons.Outlined.ContentCopy, "复制日志") }
                    IconButton(onClick = { vm.loadLog() }, enabled = !state.logLoading) {
                        Icon(Icons.Outlined.Refresh, "刷新")
                    }
                    IconButton(onClick = { vm.logout() }) {
                        Icon(Icons.Outlined.Logout, "退出登录")
                    }
                },
                windowInsets = TopAppBarDefaults.windowInsets,
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surface,
                ),
            )
        },
        bottomBar = { BottomNav(state.tab, vm::selectTab) },
        snackbarHost = { SnackbarHost(snackbarHostState) },
        containerColor = MaterialTheme.colorScheme.background,
    ) { inner ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(inner),
        ) {
            // 来源切换段控件 + 自动刷新开关
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                SingleChoiceSegmentedButtonRow(modifier = Modifier.weight(1f)) {
                    SegmentedButton(
                        selected = state.logPasswallOnly,
                        onClick = { if (!state.logPasswallOnly) vm.setLogPasswallOnly(true) },
                        shape = SegmentedButtonDefaults.itemShape(index = 0, count = 2),
                        icon = { Icon(Icons.Outlined.Tune, null, modifier = Modifier.size(16.dp)) },
                    ) { Text("PassWall") }
                    SegmentedButton(
                        selected = !state.logPasswallOnly,
                        onClick = { if (state.logPasswallOnly) vm.setLogPasswallOnly(false) },
                        shape = SegmentedButtonDefaults.itemShape(index = 1, count = 2),
                        icon = { Icon(Icons.Outlined.Article, null, modifier = Modifier.size(16.dp)) },
                    ) { Text("系统全量") }
                }
                Spacer(Modifier.width(12.dp))
                Text(
                    "自动",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.width(4.dp))
                Switch(
                    checked = state.logAutoRefresh,
                    onCheckedChange = { vm.setLogAutoRefresh(it) },
                )
            }

            when {
                state.logLoading && state.logText.isEmpty() -> {
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator()
                    }
                }
                state.logText.isEmpty() -> {
                    Column(
                        Modifier
                            .fillMaxSize()
                            .padding(24.dp),
                        verticalArrangement = Arrangement.Center,
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        Text(
                            state.logError ?: "暂无日志",
                            style = MaterialTheme.typography.titleMedium,
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                        Spacer(Modifier.height(6.dp))
                        Text(
                            "可能服务未启动，或当前 ACL 不允许执行 logread。",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Spacer(Modifier.height(16.dp))
                        OutlinedButton(onClick = { vm.loadLog() }) {
                            Icon(Icons.Outlined.Refresh, null, modifier = Modifier.size(18.dp))
                            Spacer(Modifier.width(6.dp))
                            Text("重试")
                        }
                    }
                }
                else -> {
                    // 每次 logText 变化（首载 / 刷新 / 切来源）后自动滚到末端。
                    val scroll = rememberScrollState()
                    LaunchedEffect(state.logText) {
                        if (state.logText.isNotEmpty()) scroll.scrollTo(scroll.maxValue)
                    }
                    Surface(
                        color = MaterialTheme.colorScheme.surface,
                        tonalElevation = 1.dp,
                        shape = MaterialTheme.shapes.medium,
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(horizontal = 12.dp, vertical = 4.dp),
                    ) {
                        SelectionContainer {
                            Text(
                                text = state.logText,
                                style = MaterialTheme.typography.bodySmall.copy(
                                    fontFamily = FontFamily.Monospace,
                                ),
                                color = MaterialTheme.colorScheme.onSurface,
                                modifier = Modifier
                                    .fillMaxSize()
                                    .verticalScroll(scroll)
                                    .padding(12.dp),
                            )
                        }
                    }
                }
            }
        }
    }
}

/** 底部 NavigationBar：被 ClientsScreen 与 LogsScreen 共用。 */
@Composable
fun BottomNav(current: Tab, onSelect: (Tab) -> Unit) {
    NavigationBar(
        containerColor = MaterialTheme.colorScheme.surface,
        tonalElevation = 3.dp,
    ) {
        NavigationBarItem(
            selected = current == Tab.Clients,
            onClick = { onSelect(Tab.Clients) },
            icon = { Icon(Icons.Outlined.Tune, null) },
            label = { Text("规则") },
        )
        NavigationBarItem(
            selected = current == Tab.Logs,
            onClick = { onSelect(Tab.Logs) },
            icon = { Icon(Icons.Outlined.Article, null) },
            label = { Text("日志") },
        )
    }
}
