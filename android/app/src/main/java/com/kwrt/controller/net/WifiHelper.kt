package com.kwrt.controller.net

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.wifi.WifiInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.provider.Settings
import androidx.core.content.ContextCompat

/**
 * Wi-Fi 工具：读取当前 SSID + 打开系统 Wi-Fi 设置（Android 10+ 不允许应用直接静默切网）。
 *
 * 读取 SSID 的复杂度：
 *  - Android 13+：用 `ConnectivityManager.NetworkCapabilities.transportInfo` 拿 WifiInfo；
 *    建议持有 `NEARBY_WIFI_DEVICES` 权限，否则系统返回 `<unknown ssid>`。
 *  - Android 10–12：同上路径，但必须 `ACCESS_FINE_LOCATION` + 定位服务开启，否则同样是 `<unknown ssid>`。
 *  - Android 8–9：直接 `WifiManager.connectionInfo.ssid`。
 *
 * 返回值已剥掉两端引号；无 Wi-Fi 或拿不到时返回 null。
 */
object WifiHelper {

    fun hasSsidPermission(ctx: Context): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            ContextCompat.checkSelfPermission(ctx, Manifest.permission.NEARBY_WIFI_DEVICES) ==
                PackageManager.PERMISSION_GRANTED
        } else {
            ContextCompat.checkSelfPermission(ctx, Manifest.permission.ACCESS_FINE_LOCATION) ==
                PackageManager.PERMISSION_GRANTED
        }
    }

    fun requiredSsidPermission(): String =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU)
            Manifest.permission.NEARBY_WIFI_DEVICES
        else
            Manifest.permission.ACCESS_FINE_LOCATION

    /** 当前是否处于 Wi-Fi 连接（不区分 SSID）。 */
    fun isOnWifi(ctx: Context): Boolean {
        val cm = ctx.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager ?: return false
        val nw = cm.activeNetwork ?: return false
        val caps = cm.getNetworkCapabilities(nw) ?: return false
        return caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)
    }

    /** 读取当前 SSID；权限不足或非 Wi-Fi 时返回 null。 */
    fun getCurrentSsid(ctx: Context): String? {
        if (!isOnWifi(ctx)) return null
        if (!hasSsidPermission(ctx)) return null

        val cm = ctx.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager
        val nw = cm?.activeNetwork
        val caps = nw?.let { cm.getNetworkCapabilities(it) }
        val info: WifiInfo? = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            caps?.transportInfo as? WifiInfo
        } else {
            @Suppress("DEPRECATION")
            (ctx.applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager)?.connectionInfo
        }
        val raw = info?.ssid ?: return null
        // SSID 通常带引号 "MyWifi"；剥掉
        val trimmed = raw.removePrefix("\"").removeSuffix("\"")
        if (trimmed.isBlank() || trimmed == "<unknown ssid>" || trimmed == "0x") return null
        return trimmed
    }

    /** 打开系统 Wi-Fi 设置页（Android 10+ 静默切网受限，让用户手动选）。 */
    fun openWifiSettings(ctx: Context) {
        val intent = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            Intent(Settings.Panel.ACTION_WIFI)
        } else {
            Intent(Settings.ACTION_WIFI_SETTINGS)
        }.apply { addFlags(Intent.FLAG_ACTIVITY_NEW_TASK) }
        runCatching { ctx.startActivity(intent) }
    }
}
