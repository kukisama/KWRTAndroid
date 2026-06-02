package com.kwrt.controller.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

// 复刻 Tauri 端 style.v4.css 的 CSS 变量（--bg/--card/--primary/--ok/--text/--muted/--line）。
// 浅色
private val LightBg       = Color(0xFFF5F7FA)
private val LightCard     = Color(0xFFFFFFFF)
private val LightCard2    = Color(0xFFF0F3F8)
private val LightLine     = Color(0xFFDBE1EB)
private val LightText     = Color(0xFF1C2230)
private val LightMuted    = Color(0xFF687387)
private val LightPrimary  = Color(0xFF2F6CE8)
private val LightOk       = Color(0xFF1F8A3A)

// 深色
private val DarkBg        = Color(0xFF0F1115)
private val DarkCard      = Color(0xFF181B22)
private val DarkCard2     = Color(0xFF1F2330)
private val DarkLine      = Color(0xFF2A2F3D)
private val DarkText      = Color(0xFFE8ECF3)
private val DarkMuted     = Color(0xFF98A2B3)
private val DarkPrimary   = Color(0xFF4F8CFF)
private val DarkOk        = Color(0xFF3FB950)

/**
 * 主题。Android 12+（minSdk=31 保底命中）默认开启**动态取色**，
 * 让 primary / background / surface 跟随系统壁纸；
 * 但 `tertiary`（启用 Switch 用的绿色）永远锁我们自己的 --ok，
 * 保证"绿=启用、灰=停用"的语义不会被壁纸冲掉。
 */
@Composable
fun KwrtTheme(
    dynamicColor: Boolean = true,
    content: @Composable () -> Unit,
) {
    val dark = isSystemInDarkTheme()
    val ctx = LocalContext.current

    val scheme = when {
        dynamicColor -> {
            val base = if (dark) dynamicDarkColorScheme(ctx) else dynamicLightColorScheme(ctx)
            // 强制覆盖 tertiary = 我们自己的 --ok，避免壁纸提色把"启用绿"染成别的色
            base.copy(
                tertiary = if (dark) DarkOk else LightOk,
                onTertiary = Color.White,
            )
        }
        dark -> darkColorScheme(
            primary = DarkPrimary,
            onPrimary = Color.White,
            background = DarkBg,
            onBackground = DarkText,
            surface = DarkCard,
            onSurface = DarkText,
            surfaceVariant = DarkCard2,
            onSurfaceVariant = DarkMuted,
            outline = DarkLine,
            outlineVariant = DarkLine,
            tertiary = DarkOk,
            onTertiary = Color.White,
        )
        else -> lightColorScheme(
            primary = LightPrimary,
            onPrimary = Color.White,
            background = LightBg,
            onBackground = LightText,
            surface = LightCard,
            onSurface = LightText,
            surfaceVariant = LightCard2,
            onSurfaceVariant = LightMuted,
            outline = LightLine,
            outlineVariant = LightLine,
            tertiary = LightOk,
            onTertiary = Color.White,
        )
    }
    MaterialTheme(colorScheme = scheme, content = content)
}
