package com.kwrt.controller.ui.screen

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.Person
import androidx.compose.material.icons.outlined.Router
import androidx.compose.material.icons.outlined.Shield
import androidx.compose.material.icons.outlined.Wifi
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import com.kwrt.controller.vm.AppViewModel
import com.kwrt.controller.vm.UiState

/**
 * 登录页（深色现代）：
 *  - 顶部品牌徽标 + 标题
 *  - 单卡片：地址 / 用户名 / 密码 / 记住开关
 *  - scheme 与端口由地址栏智能推断：默认 http:80；写 https:// 则 https:443；可写 host:port 覆盖
 *  - 不再暴露 UCI 配置名（固定 passwall）
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LoginScreen(state: UiState, vm: AppViewModel) {
    val c = state.creds
    val cs = MaterialTheme.colorScheme

    Column(
        Modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 24.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        // 顶部品牌区
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = 24.dp, bottom = 8.dp),
        ) {
            Box(
                Modifier
                    .size(72.dp)
                    .clip(RoundedCornerShape(20.dp))
                    .background(Brush.linearGradient(listOf(cs.primary, cs.primaryContainer))),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    Icons.Outlined.Shield,
                    contentDescription = null,
                    tint = cs.onPrimary,
                    modifier = Modifier.size(36.dp),
                )
            }
            Spacer(Modifier.height(16.dp))
            Text(
                "KWRT 控制器",
                style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.Bold),
                color = cs.onBackground,
            )
            Spacer(Modifier.height(6.dp))
            Text(
                "连接到 OpenWrt + PassWall 路由器",
                style = MaterialTheme.typography.bodyMedium,
                color = cs.onSurfaceVariant,
            )
        }

        if (state.autoLoggingIn) {
            Surface(color = cs.surfaceVariant, shape = RoundedCornerShape(16.dp)) {
                Row(
                    Modifier.padding(horizontal = 16.dp, vertical = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    CircularProgressIndicator(strokeWidth = 2.dp, modifier = Modifier.size(18.dp))
                    Text("正在自动登录…", color = cs.onSurfaceVariant)
                }
            }
        }

        // 输入卡片
        Surface(color = cs.surface, shape = RoundedCornerShape(20.dp), tonalElevation = 0.dp) {
            Column(
                Modifier.padding(20.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                OutlinedTextField(
                    value = c.host,
                    onValueChange = { vm.updateCreds { s -> s.copy(host = it) } },
                    label = { Text("路由器地址") },
                    placeholder = { Text("192.168.1.1 或 https://router:8443") },
                    leadingIcon = { Icon(Icons.Outlined.Router, null) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Uri,
                        imeAction = ImeAction.Next,
                    ),
                    shape = RoundedCornerShape(14.dp),
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = c.username,
                    onValueChange = { vm.updateCreds { s -> s.copy(username = it) } },
                    label = { Text("用户名") },
                    leadingIcon = { Icon(Icons.Outlined.Person, null) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
                    shape = RoundedCornerShape(14.dp),
                    modifier = Modifier.fillMaxWidth(),
                )
                var pwVisible by remember { mutableStateOf(false) }
                OutlinedTextField(
                    value = c.password,
                    onValueChange = { vm.updateCreds { s -> s.copy(password = it) } },
                    label = { Text("密码") },
                    leadingIcon = { Icon(Icons.Outlined.Lock, null) },
                    singleLine = true,
                    visualTransformation = if (pwVisible) VisualTransformation.None
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
                    shape = RoundedCornerShape(14.dp),
                    modifier = Modifier.fillMaxWidth(),
                )

                Row(
                    Modifier
                        .fillMaxWidth()
                        .padding(top = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Switch(checked = state.remember, onCheckedChange = { vm.setRemember(it) })
                    Spacer(Modifier.width(10.dp))
                    Column(Modifier.weight(1f)) {
                        Text("记住凭据", color = cs.onSurface, style = MaterialTheme.typography.bodyLarge)
                        Text(
                            "下次启动自动登录",
                            color = cs.onSurfaceVariant,
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                }
            }
        }

        // 错误信息 + “打开 Wi-Fi 设置”按钮
        state.loginError?.takeIf { !state.autoLoggingIn }?.let { msg ->
            Surface(color = cs.errorContainer, shape = RoundedCornerShape(14.dp)) {
                Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(
                        msg,
                        color = cs.onErrorContainer,
                        style = MaterialTheme.typography.bodyMedium,
                    )
                    OutlinedButton(
                        onClick = { vm.openWifiSettings() },
                        modifier = Modifier.fillMaxWidth(),
                        shape = RoundedCornerShape(12.dp),
                    ) {
                        Icon(Icons.Outlined.Wifi, null, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(6.dp))
                        Text("打开 Wi-Fi 设置")
                    }
                }
            }
        }

        Button(
            onClick = { vm.login() },
            enabled = !state.loading && !state.autoLoggingIn,
            modifier = Modifier
                .fillMaxWidth()
                .height(54.dp),
            shape = RoundedCornerShape(16.dp),
        ) {
            if (state.loading) {
                CircularProgressIndicator(
                    strokeWidth = 2.dp,
                    color = cs.onPrimary,
                    modifier = Modifier.size(18.dp),
                )
                Spacer(Modifier.width(10.dp))
                Text("登录中…", style = MaterialTheme.typography.titleMedium)
            } else {
                Text(
                    "登录",
                    style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                )
            }
        }

        Text(
            "提示：默认走 HTTP（端口 80）。如需 HTTPS，在地址栏写 https:// 前缀；端口非默认时写 host:port。",
            style = MaterialTheme.typography.bodySmall,
            color = cs.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 4.dp),
        )

        Spacer(Modifier.height(8.dp))
    }
}
