package com.plugin.blackout_droid

import android.app.Activity
import android.app.KeyguardManager
import android.app.admin.DevicePolicyManager
import android.bluetooth.BluetoothManager
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.Uri
import android.net.wifi.WifiManager
import android.os.Build
import android.provider.Settings
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class PanelArgs {
    lateinit var panel: String
}

@InvokeArg
class PkgArgs {
    lateinit var pkg: String
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
        res.put("usbDebugging", readGlobalInt(ctx, Settings.Global.ADB_ENABLED) == 1)
        res.put("locationOn", isLocationOn(ctx))
        res.put("nfcOn", isNfcOn(ctx))
        res.put("patchAgeDays", securityPatchAgeDays())
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
                "developer" -> Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS
                "nfc" -> Settings.ACTION_NFC_SETTINGS
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

    // ---------- "Am I hacked?" — installed-app inventory + threat flags ----------
    @Command
    fun listApps(invoke: Invoke) {
        try {
            val pm = activity.packageManager
            val ctx = activity.applicationContext
            // Apps running an enabled accessibility service (a classic RAT vector).
            val a11y = Settings.Secure.getString(
                ctx.contentResolver, Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES
            ) ?: ""
            // Active device administrators (malware persistence).
            val admins = try {
                val dpm = ctx.getSystemService(Context.DEVICE_POLICY_SERVICE) as DevicePolicyManager
                (dpm.activeAdmins ?: emptyList()).map { it.packageName }.toSet()
            } catch (e: Exception) { emptySet<String>() }

            val stores = setOf("com.android.vending", "com.google.android.feedback", "com.android.packageinstaller")
            val danger = setOf(
                "android.permission.READ_SMS", "android.permission.SEND_SMS", "android.permission.RECEIVE_SMS",
                "android.permission.READ_CALL_LOG", "android.permission.READ_CONTACTS", "android.permission.RECORD_AUDIO",
                "android.permission.CAMERA", "android.permission.SYSTEM_ALERT_WINDOW", "android.permission.REQUEST_INSTALL_PACKAGES",
                "android.permission.READ_PHONE_STATE", "android.permission.ACCESS_FINE_LOCATION", "android.permission.BIND_DEVICE_ADMIN"
            )

            val apps = JSArray()
            for (p in pm.getInstalledPackages(PackageManager.GET_PERMISSIONS)) {
                val ai = p.applicationInfo ?: continue
                val isSystem = (ai.flags and ApplicationInfo.FLAG_SYSTEM) != 0
                val installer = try {
                    if (Build.VERSION.SDK_INT >= 30) pm.getInstallSourceInfo(p.packageName).installingPackageName
                    else @Suppress("DEPRECATION") pm.getInstallerPackageName(p.packageName)
                } catch (e: Exception) { null }
                val perms = p.requestedPermissions ?: arrayOf<String>()
                val o = JSObject()
                o.put("name", pm.getApplicationLabel(ai).toString())
                o.put("package", p.packageName)
                o.put("system", isSystem)
                o.put("sideloaded", !isSystem && (installer == null || installer !in stores))
                o.put("accessibility", a11y.contains(p.packageName))
                o.put("deviceAdmin", admins.contains(p.packageName))
                o.put("riskyPerms", perms.count { it in danger })
                o.put("installed", p.firstInstallTime)
                o.put("updated", p.lastUpdateTime)
                o.put("installer", installer ?: "")
                apps.put(o)
            }
            val res = JSObject()
            res.put("apps", apps)
            invoke.resolve(res)
        } catch (e: Exception) {
            invoke.reject(e.message ?: "could not list apps")
        }
    }

    @Command
    fun uninstallApp(invoke: Invoke) {
        val ok = try {
            val args = invoke.parseArgs(PkgArgs::class.java)
            val intent = Intent(Intent.ACTION_DELETE)
                .setData(Uri.fromParts("package", args.pkg, null))
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            activity.startActivity(intent)
            true
        } catch (e: Exception) { false }
        val res = JSObject(); res.put("ok", ok); invoke.resolve(res)
    }

    @Command
    fun openAppSettings(invoke: Invoke) {
        val ok = try {
            val args = invoke.parseArgs(PkgArgs::class.java)
            val intent = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS)
                .setData(Uri.fromParts("package", args.pkg, null))
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            activity.startActivity(intent)
            true
        } catch (e: Exception) { false }
        val res = JSObject(); res.put("ok", ok); invoke.resolve(res)
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

    private fun isNfcOn(ctx: Context): Boolean {
        return try {
            android.nfc.NfcAdapter.getDefaultAdapter(ctx)?.isEnabled ?: false
        } catch (e: Exception) {
            false
        }
    }

    /// Days since the OS security patch level, or -1 if unknown.
    private fun securityPatchAgeDays(): Int {
        return try {
            val patch = Build.VERSION.SECURITY_PATCH
            if (patch.isNullOrEmpty()) return -1
            val sdf = java.text.SimpleDateFormat("yyyy-MM-dd", java.util.Locale.US)
            val date = sdf.parse(patch) ?: return -1
            ((System.currentTimeMillis() - date.time) / (1000L * 60 * 60 * 24)).toInt()
        } catch (e: Exception) {
            -1
        }
    }
}
