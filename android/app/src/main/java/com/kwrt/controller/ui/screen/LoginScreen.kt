package com.kwrt.controller.ui.screen

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.Person
import androidx.compose.material.icons.outlined.Router
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.kwrt.controller.vm.AppViewModel
import com.kwrt.controller.vm.UiState

/**
 * 登录页：路由器地址 / 账号 / 密码 / "记住并下次自动登录" 开关。
 * 设计语言：单卡片居中、大输入框、底部主按钮、safeDrawing 避开手势条/状态栏。
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LoginScreen(state: UiState, vm: AppViewModel) {
    val c = state.creds

    Column(
        Modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 24.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Spacer(Modifier.height(8.dp))
        Column(Modifier.padding(horizontal = 4.dp)) {
            Text(
                "KWRT 控制器",
                style = MaterialTheme.typography.headlineMedium.copy(fontWeight = FontWeight.SemiBold),
                color = MaterialTheme.colorScheme.onBackground,
            )
            Spacer(Modifier.height(4.dp))
            Text(
                "连接到运行 OpenWrt + PassWall 的路由器",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        if (state.autoLoggingIn) {
            Surface(
                tonalElevation = 0.dp,
                color = MaterialTheme.colorScheme.surfaceVariant,
                shape = MaterialTheme.shapes.large,
            ) {
                Row(
                    Modifier.padding(16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    CircularProgressIndicator(strokeWidth = 2.dp, modifier = Modifier.size(20.dp))
                    Text(
                        "正在自动登录…",
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }

        // 输入卡片
        Surface(
            color = MaterialTheme.colorScheme.surface,
            shape = MaterialTheme.shapes.large,
            tonalElevation = 1.dp,
        ) {
            Column(
                Modifier.padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                // 协议 + 主机 + 端口
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    SchemeToggle(
                        value = c.scheme,
                        onChange = {
                            vm.updateCreds { s ->
                                s.copy(scheme = if (s.scheme == "http") "https" else "http")
                            }
                        },
                        modifier = Modifier.weight(0f),
                    )
                    OutlinedTextField(
                        value = c.host,
                        onValueChange = { vm.updateCreds { s -> s.copy(host = it) } },
                        label = { Text("路由器 IP / 域名") },
                        leadingIcon = { Icon(Icons.Outlined.Router, null) },
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(
                            keyboardType = KeyboardType.Uri,
                            imeAction = ImeAction.Next,
                        ),
                        modifier = Modifier.weight(1f),
                    )
                }
                OutlinedTextField(
                    value = c.port?.toString() ?: "",
                    onValueChange = {
                        vm.updateCreds { s ->
                            s.copy(port = it.filter { ch -> ch.isDigit() }.toIntOrNull())
                        }
                    },
                    label = { Text("端口（留空 = 默认 80/443）") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Number,
                        imeAction = ImeAction.Next,
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = c.username,
                    onValueChange = { vm.updateCreds { s -> s.copy(username = it) } },
                    label = { Text("用户名") },
                    leadingIcon = { Icon(Icons.Outlined.Person, null) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
                    modifier = Modifier.fillMaxWidth(),
                )
                var pwVisible by remember { mutableStateOf(false) }
                OutlinedTextField(
                    value = c.password,
                    onValueChange = { vm.updateCreds { s -> s.copy(password = it) } },
                    label = { Text("密码") },
                    leadingIcon = { Icon(Icons.Outlined.Lock, null) },
                    singleLine = true,
                    visualTransformation = if (pwVisible) androidx.compose.ui.text.input.VisualTransformation.None
                                           else PasswordVisualTransformation(),
                    trailingIcon = {
                        TextButton(onClick = { pwVisible = !pwVisible }) {
                            Text(if (pwVisible) "隐藏" else "显示")
                        }
                    },
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Password,
                        imeAction = ImeAction.Done,
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = c.config,
                    onValueChange = { vm.updateCreds { s -> s.copy(config = it.ifBlank { "passwall" }) } },
                    label = { Text("UCI 配置名（默认 passwall）") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )

                Row(
                    Modifier
                        .fillMaxWidth()
                        .padding(top = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Switch(checked = state.remember, onCheckedChange = { vm.setRemember(it) })
                    Spacer(Modifier.width(8.dp))
                    Text("记住并下次自动登录", color = MaterialTheme.colorScheme.onSurface)
                }
            }
        }

        // 错误信息
        state.loginError?.takeIf { !state.autoLoggingIn }?.let { msg ->
            Surface(
                color = MaterialTheme.colorScheme.errorContainer,
                shape = MaterialTheme.shapes.medium,
            ) {
                Text(
                    msg,
                    Modifier.padding(12.dp),
                    color = MaterialTheme.colorScheme.onErrorContainer,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }

        Button(
            onClick = { vm.login() },
            enabled = !state.loading && !state.autoLoggingIn,
            modifier = Modifier
                .fillMaxWidth()
                .height(52.dp),
            shape = MaterialTheme.shapes.large,
        ) {
            if (state.loading) {
                CircularProgressIndicator(
                    strokeWidth = 2.dp,
                    color = Color.White,
                    modifier = Modifier.size(20.dp),
                )
                Spacer(Modifier.width(8.dp))
                Text("登录中…")
            } else {
                Text("登录", style = MaterialTheme.typography.titleMedium)
            }
        }

        Spacer(Modifier.height(8.dp))
    }
}

@Composable
private fun SchemeToggle(value: String, onChange: () -> Unit, modifier: Modifier = Modifier) {
    AssistChip(
        onClick = onChange,
        label = { Text(value.uppercase()) },
        modifier = modifier.height(56.dp),
    )
}
