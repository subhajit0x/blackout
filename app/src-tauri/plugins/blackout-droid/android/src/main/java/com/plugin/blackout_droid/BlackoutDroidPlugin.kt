package com.plugin.blackout_droid

import android.app.Activity
import android.app.KeyguardManager
import android.bluetooth.BluetoothManager
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.wifi.WifiManager
import android.os.Build
import android.provider.Settings
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class PanelArgs {
    lateinit var panel: String
}

@TauriPlugin
class BlackoutDroidPlugin(private val activity: Activity) : Plugin(activity) {

    // ---------- OPSEC: read live device state ----------
    @Command
    fun opsecFacts(invoke: Invoke) {
        val res = JSObject()
        val ctx: Context = activity.applicationContext
        res.put("sdkInt", Build.VERSION.SDK_INT)
        res.put("osVersion", Build.VERSION.RELEASE ?: "")
        res.put("model", Build.MODEL ?: "")
        res.put("vpnActive", isVpnActive(ctx))
        res.put("wifiOn", isWifiOn(ctx))
        res.put("bluetoothOn", isBluetoothOn(ctx))
        res.put("airplaneOn", readGlobalInt(ctx, Settings.Global.AIRPLANE_MODE_ON) == 1)
        res.put("screenLockSet", isDeviceSecure(ctx))
        res.put("developerOptions", readGlobalInt(ctx, Settings.Global.DEVELOPMENT_SETTINGS_ENABLED) == 1)
        res.put("locationOn", isLocationOn(ctx))
        invoke.resolve(res)
    }

    // ---------- LOCKDOWN/PANIC: jump to a system panel ----------
    @Command
    fun openPanel(invoke: Invoke) {
        val ok = try {
            val args = invoke.parseArgs(PanelArgs::class.java)
            val action = when (args.panel) {
                "wifi" -> Settings.ACTION_WIFI_SETTINGS
                "bluetooth" -> Settings.ACTION_BLUETOOTH_SETTINGS
                "airplane" -> Settings.ACTION_AIRPLANE_MODE_SETTINGS
                "location" -> Settings.ACTION_LOCATION_SOURCE_SETTINGS
                "security" -> Settings.ACTION_SECURITY_SETTINGS
                "permissions", "privacy" -> Settings.ACTION_APPLICATION_DETAILS_SETTINGS
                else -> Settings.ACTION_SETTINGS
            }
            val intent = Intent(action)
            if (action == Settings.ACTION_APPLICATION_DETAILS_SETTINGS) {
                intent.data = android.net.Uri.fromParts("package", activity.packageName, null)
            }
            intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            activity.startActivity(intent)
            true
        } catch (e: Exception) {
            false
        }
        val res = JSObject()
        res.put("ok", ok)
        invoke.resolve(res)
    }

    // ---------- PANIC: wipe the clipboard ----------
    @Command
    fun clearClipboard(invoke: Invoke) {
        val ok = try {
            val cm = activity.applicationContext.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                cm.clearPrimaryClip()
            } else {
                cm.setPrimaryClip(ClipData.newPlainText("", ""))
            }
            true
        } catch (e: Exception) {
            false
        }
        val res = JSObject()
        res.put("ok", ok)
        invoke.resolve(res)
    }

    // ---------- helpers ----------
    private fun isVpnActive(ctx: Context): Boolean {
        return try {
            val cm = ctx.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
            val net = cm.activeNetwork ?: return false
            val caps = cm.getNetworkCapabilities(net) ?: return false
            caps.hasTransport(NetworkCapabilities.TRANSPORT_VPN)
        } catch (e: Exception) {
            false
        }
    }

    private fun isWifiOn(ctx: Context): Boolean {
        return try {
            val wm = ctx.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
            wm.isWifiEnabled
        } catch (e: Exception) {
            false
        }
    }

    private fun isBluetoothOn(ctx: Context): Boolean {
        return try {
            val bm = ctx.getSystemService(Context.BLUETOOTH_SERVICE) as BluetoothManager
            bm.adapter?.isEnabled ?: false
        } catch (e: Exception) {
            false
        }
    }

    private fun isDeviceSecure(ctx: Context): Boolean {
        return try {
            val km = ctx.getSystemService(Context.KEYGUARD_SERVICE) as KeyguardManager
            km.isDeviceSecure
        } catch (e: Exception) {
            false
        }
    }

    private fun isLocationOn(ctx: Context): Boolean {
        return try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                val lm = ctx.getSystemService(Context.LOCATION_SERVICE) as android.location.LocationManager
                lm.isLocationEnabled
            } else {
                Settings.Secure.getInt(ctx.contentResolver, Settings.Secure.LOCATION_MODE, 0) != 0
            }
        } catch (e: Exception) {
            false
        }
    }

    private fun readGlobalInt(ctx: Context, key: String): Int {
        return try {
            Settings.Global.getInt(ctx.contentResolver, key, 0)
        } catch (e: Exception) {
            0
        }
    }
}
