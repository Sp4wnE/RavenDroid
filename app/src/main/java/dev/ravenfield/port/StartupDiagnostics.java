package dev.ravenfield.port;

import android.app.ActivityManager;
import android.app.Application;
import android.app.ApplicationExitInfo;
import android.content.Context;
import android.os.Build;
import android.util.Log;
import android.webkit.WebView;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayDeque;
import java.util.Date;

/** Local, bounded diagnostics. Nothing is sent to a server. */
final class StartupDiagnostics {
    private static final int LIMIT = 65536;
    private static final ArrayDeque<String> recent = new ArrayDeque<>();
    private static File current;
    private static String previous = "";

    static void initialize(Application app) {
        current = new File(app.getFilesDir(), "startup-diagnostics.txt");
        previous = read(current);
        try (FileOutputStream ignored = new FileOutputStream(current)) { }
        catch (Exception e) { Log.w("Ravenfield", "Cannot reset diagnostics", e); }
        Thread.UncaughtExceptionHandler system = Thread.getDefaultUncaughtExceptionHandler();
        Thread.setDefaultUncaughtExceptionHandler((thread, error) -> {
            try { record("UNCAUGHT " + thread.getName() + "\n" + Log.getStackTraceString(error)); }
            finally {
                if (system != null) system.uncaughtException(thread, error);
                else { android.os.Process.killProcess(android.os.Process.myPid()); System.exit(10); }
            }
        });
        record("application-created version=" + BuildConfig.VERSION_NAME);
    }

    static synchronized void record(String message) {
        String line = new Date() + " " + message;
        if (line.length() > 12000) line = line.substring(0, 12000);
        Log.i("RavenfieldStartup", line);
        recent.addLast(line);
        while (recent.size() > 32) recent.removeFirst();
        if (current == null) return;
        try (FileOutputStream out = new FileOutputStream(current, current.length() < LIMIT)) {
            out.write((line + "\n").getBytes(StandardCharsets.UTF_8));
        } catch (Exception e) { Log.w("Ravenfield", "Cannot save diagnostics", e); }
    }

    private static String read(File file) {
        try (FileInputStream in = new FileInputStream(file)) {
            byte[] bytes = new byte[LIMIT];
            int length = 0, n;
            while (length < bytes.length && (n = in.read(bytes, length, bytes.length - length)) != -1) length += n;
            return new String(bytes, 0, length, StandardCharsets.UTF_8);
        } catch (Exception ignored) { return ""; }
    }

    static synchronized String report(Context context) {
        StringBuilder out = new StringBuilder("Ravenfield " + BuildConfig.VERSION_NAME + "\n");
        out.append(Build.MANUFACTURER).append(' ').append(Build.MODEL)
            .append(" / Android ").append(Build.VERSION.RELEASE).append(" API ").append(Build.VERSION.SDK_INT)
            .append("\nABI: ").append(String.join(", ", Build.SUPPORTED_ABIS)).append('\n');
        try {
            android.content.pm.PackageInfo provider = WebView.getCurrentWebViewPackage();
            out.append("WebView: ").append(provider == null ? "unavailable" : provider.packageName + " " + provider.versionName).append('\n');
        } catch (RuntimeException | LinkageError e) { out.append("WebView lookup: ").append(e).append('\n'); }
        try {
            ActivityManager manager = (ActivityManager) context.getSystemService(Context.ACTIVITY_SERVICE);
            ActivityManager.MemoryInfo memory = new ActivityManager.MemoryInfo();
            manager.getMemoryInfo(memory);
            out.append("RAM total/available: ").append(memory.totalMem).append('/').append(memory.availMem).append('\n');
            if (Build.VERSION.SDK_INT >= 30) {
                for (ApplicationExitInfo exit : manager.getHistoricalProcessExitReasons(context.getPackageName(), 0, 5)) {
                    out.append("Exit ").append(new Date(exit.getTimestamp())).append(" process=").append(exit.getProcessName())
                        .append(" reason=").append(exit.getReason()).append(" status=").append(exit.getStatus())
                        .append(" PSS_KiB=").append(exit.getPss()).append(" description=").append(exit.getDescription()).append('\n');
                }
            }
        } catch (RuntimeException e) { out.append("Exit info: ").append(e).append('\n'); }
        out.append("\nPrevious launch:\n").append(previous.isEmpty() ? "No saved log\n" : previous);
        out.append("\nCurrent launch:\n");
        for (String line : recent) out.append(line).append('\n');
        return out.toString();
    }
}
