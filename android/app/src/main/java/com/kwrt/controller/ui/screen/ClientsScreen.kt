package com.kwrt.controller.ui.screen

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Logout
import androidx.compose.material.icons.outlined.Refresh
import androidx.compose.material.icons.outlined.SignalWifiOff
import androidx.compose.material.icons.outlined.Wifi
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.pulltorefresh.rememberPullToRefreshState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.kwrt.controller.net.AclRow
import com.kwrt.controller.vm.AppViewModel
import com.kwrt.controller.vm.UiState

/**
 * 客户端开关页：列表 + 底部"应用配置"。每行只显示源 IP/MAC + 备注 + 开关。
 * 全面屏：顶部 TopAppBar 走 statusBars 内边距，底部按钮区贴 navigationBars。
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ClientsScreen(state: UiState, vm: AppViewModel) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text("客户端开关", style = MaterialTheme.typography.titleLarge)
                        Text(
                            state.creds.baseUrl(),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                },
                actions = {
                    IconButton(onClick = { vm.refresh(manual = true) }, enabled = !state.loading) {
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
        bottomBar = {
            Column {
                BottomApplyBar(state, vm)
                BottomNav(state.tab, vm::selectTab)
            }
        },
        containerColor = MaterialTheme.colorScheme.background,
    ) { inner ->
        val pullState = rememberPullToRefreshState()
        PullToRefreshBox(
            isRefreshing = state.loading,
            onRefresh = { vm.refresh(manual = true) },
            state = pullState,
            modifier = Modifier
                .fillMaxSize()
                .padding(inner),
        ) {
            Column(Modifier.fillMaxSize()) {
                when {
                    state.networkDown -> {
                        NetworkDownHint(vm)
                    }
                    state.loading && state.rows.isEmpty() -> {
                        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                            CircularProgressIndicator()
                        }
                    }
                    state.rows.isEmpty() -> {
                        EmptyHint()
                    }
                    else -> {
                        LazyColumn(
                            modifier = Modifier.fillMaxSize(),
                            contentPadding = PaddingValues(
                                start = 16.dp, end = 16.dp,
                                top = 12.dp, bottom = 12.dp,
                            ),
                            verticalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            item {
                                val total = state.rows.size
                                val on = state.rows.count { it.enabled }
                                Text(
                                    "共 $total 条，启用 $on 条" + if (state.pending) " · 有未应用改动" else "",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    modifier = Modifier.padding(start = 4.dp, bottom = 2.dp),
                                )
                            }
                            items(state.rows, key = { it.section }) { row ->
                                AclRowCard(row, onToggle = { vm.toggle(row.section) })
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun AclRowCard(row: AclRow, onToggle: () -> Unit) {
    Surface(
        color = MaterialTheme.colorScheme.surface,
        shape = MaterialTheme.shapes.large,
        tonalElevation = 1.dp,
    ) {
        Row(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(
                    row.remarks.ifBlank { "(无备注)" },
                    style = MaterialTheme.typography.titleMedium.copy(
                        fontWeight = FontWeight.SemiBold,
                    ),
                    color = if (row.remarks.isBlank())
                        MaterialTheme.colorScheme.onSurfaceVariant
                    else MaterialTheme.colorScheme.onSurface,
                )
                Spacer(Modifier.height(4.dp))
                Text(
                    text = row.sources.ifBlank { "(无源地址)" },
                    style = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Spacer(Modifier.width(12.dp))
            Switch(
                checked = row.enabled,
                onCheckedChange = { onToggle() },
                colors = SwitchDefaults.colors(
                    checkedThumbColor = Color.White,
                    checkedTrackColor = MaterialTheme.colorScheme.tertiary,    // 绿色 --ok
                    uncheckedThumbColor = MaterialTheme.colorScheme.surface,
                    uncheckedTrackColor = MaterialTheme.colorScheme.surfaceVariant,
                    uncheckedBorderColor = MaterialTheme.colorScheme.outline,
                ),
            )
        }
    }
}

@Composable
private fun BottomApplyBar(state: UiState, vm: AppViewModel) {
    Surface(
        color = MaterialTheme.colorScheme.surface,
        tonalElevation = 3.dp,
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .windowInsetsPadding(WindowInsets.navigationBars)
                .padding(horizontal = 16.dp, vertical = 12.dp),
        ) {
            if (state.pending) {
                Text(
                    "有未应用的切换，点下方按钮把它们 reload 到 PassWall",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(bottom = 8.dp, start = 4.dp),
                )
            }
            Button(
                onClick = { vm.applyConfig() },
                enabled = !state.applying && state.rows.isNotEmpty(),
                modifier = Modifier
                    .fillMaxWidth()
                    .height(52.dp),
                shape = MaterialTheme.shapes.large,
            ) {
                if (state.applying) {
                    CircularProgressIndicator(
                        strokeWidth = 2.dp,
                        color = Color.White,
                        modifier = Modifier.size(20.dp),
                    )
                    Spacer(Modifier.width(8.dp))
                    Text("应用中…")
                } else {
                    Text("应用配置", style = MaterialTheme.typography.titleMedium)
                }
            }
        }
    }
}

@Composable
private fun EmptyHint() {
    Column(
        Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            "暂无 ACL 规则",
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Spacer(Modifier.height(6.dp))
        Text(
            "请在桌面端 / LuCI 网页里先添加规则，再回到这里切换启用状态。",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/** 网络中断空态：清空列表，提示连接失败 + 打开 Wi-Fi 设置 + 重试。 */
@Composable
private fun NetworkDownHint(vm: AppViewModel) {
    Column(
        Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(
            Icons.Outlined.SignalWifiOff,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.error,
            modifier = Modifier.size(56.dp),
        )
        Spacer(Modifier.height(12.dp))
        Text(
            "连接路由器失败",
            style = MaterialTheme.typography.titleLarge,
            color = MaterialTheme.colorScheme.onBackground,
        )
        Spacer(Modifier.height(6.dp))
        Text(
            "可能不在对应网络。请检查 Wi-Fi 后重试。",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(20.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Button(onClick = { vm.openWifiSettings() }) {
                Icon(Icons.Outlined.Wifi, null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text("打开 Wi-Fi 设置")
            }
            OutlinedButton(onClick = { vm.refresh(manual = true) }) {
                Icon(Icons.Outlined.Refresh, null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text("重试")
            }
        }
    }
}
