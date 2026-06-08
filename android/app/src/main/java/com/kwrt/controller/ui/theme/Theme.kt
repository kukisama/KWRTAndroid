package com.kwrt.controller.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

// === 深色（现代）===
private val DarkBg            = Color(0xFF0B0D12)
private val DarkSurface       = Color(0xFF14171E)
private val DarkSurfaceHigh   = Color(0xFF1B1F28)
private val DarkSurfaceHigher = Color(0xFF222732)
private val DarkOutline       = Color(0xFF2A3040)
private val DarkTextHi        = Color(0xFFEEF1F7)
private val DarkTextLo        = Color(0xFF9AA3B5)
private val DarkPrimary       = Color(0xFF6AA3FF)
private val DarkPrimaryOn     = Color(0xFF06121F)
private val DarkPrimaryCont   = Color(0xFF1E3A66)
private val DarkOnPrimaryCont = Color(0xFFCFE0FF)
private val DarkErrBg         = Color(0xFF3A1B1F)
private val DarkErrFg         = Color(0xFFFFB4B4)
private val DarkOk            = Color(0xFF34C759)

// === 浅色（现代）===
private val LightBg            = Color(0xFFF5F7FB)
private val LightSurface       = Color(0xFFFFFFFF)
private val LightSurfaceHigh   = Color(0xFFF0F3F8)
private val LightSurfaceHigher = Color(0xFFE7ECF3)
private val LightOutline       = Color(0xFFDBE1EB)
private val LightTextHi        = Color(0xFF1C2230)
private val LightTextLo        = Color(0xFF6B7585)
private val LightPrimary       = Color(0xFF2F6CE8)
private val LightPrimaryOn     = Color(0xFFFFFFFF)
private val LightPrimaryCont   = Color(0xFFDEE9FF)
private val LightOnPrimaryCont = Color(0xFF0A2C66)
private val LightErrBg         = Color(0xFFFFE2E2)
private val LightErrFg         = Color(0xFF7A1F1F)
private val LightOk            = Color(0xFF1F8A3A)

/**
 * 跟随系统的浅 / 深双色主题。关闭动态取色（壁纸提色），
 * 保证 KWRT 控制器在任何手机上视觉一致；tertiary 永远是"启用绿"。
 */
@Composable
fun KwrtTheme(content: @Composable () -> Unit) {
    val scheme = if (isSystemInDarkTheme()) {
        darkColorScheme(
            primary = DarkPrimary,
            onPrimary = DarkPrimaryOn,
            primaryContainer = DarkPrimaryCont,
            onPrimaryContainer = DarkOnPrimaryCont,
            background = DarkBg,
            onBackground = DarkTextHi,
            surface = DarkSurface,
            onSurface = DarkTextHi,
            surfaceVariant = DarkSurfaceHigh,
            onSurfaceVariant = DarkTextLo,
            surfaceContainer = DarkSurface,
            surfaceContainerHigh = DarkSurfaceHigh,
            surfaceContainerHighest = DarkSurfaceHigher,
            outline = DarkOutline,
            outlineVariant = DarkOutline,
            tertiary = DarkOk,
            onTertiary = Color.White,
            errorContainer = DarkErrBg,
            onErrorContainer = DarkErrFg,
        )
    } else {
        lightColorScheme(
            primary = LightPrimary,
            onPrimary = LightPrimaryOn,
            primaryContainer = LightPrimaryCont,
            onPrimaryContainer = LightOnPrimaryCont,
            background = LightBg,
            onBackground = LightTextHi,
            surface = LightSurface,
            onSurface = LightTextHi,
            surfaceVariant = LightSurfaceHigh,
            onSurfaceVariant = LightTextLo,
            surfaceContainer = LightSurface,
            surfaceContainerHigh = LightSurfaceHigh,
            surfaceContainerHighest = LightSurfaceHigher,
            outline = LightOutline,
            outlineVariant = LightOutline,
            tertiary = LightOk,
            onTertiary = Color.White,
            errorContainer = LightErrBg,
            onErrorContainer = LightErrFg,
        )
    }
    MaterialTheme(colorScheme = scheme, content = content)
}
