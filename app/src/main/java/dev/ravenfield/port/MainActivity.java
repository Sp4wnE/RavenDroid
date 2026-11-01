package dev.ravenfield.port;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.media.AudioAttributes;
import android.media.AudioFocusRequest;
import android.media.AudioManager;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.util.Log;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;
import android.webkit.ConsoleMessage;
import android.webkit.RenderProcessGoneDetail;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import android.webkit.WebResourceError;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.TextView;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.Toast;
import androidx.webkit.WebViewAssetLoader;

public final class MainActivity extends Activity {
    private static final String HOST = "appassets.androidplatform.net";
    private WebView web;
    private AudioManager audio;
    private AudioFocusRequest focusRequest;
    private boolean active, audioGranted;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        StartupDiagnostics.record("activity-created");
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        showLauncher(null);
        immersive();
        if (Build.VERSION.SDK_INT >= 33) {
            getOnBackInvokedDispatcher().registerOnBackInvokedCallback(
                android.window.OnBackInvokedDispatcher.PRIORITY_DEFAULT, this::handleBack);
        }
    }

    private void showLauncher(String failure) {
        ScrollView scroll = new ScrollView(this);
        LinearLayout column = new LinearLayout(this);
        column.setOrientation(LinearLayout.VERTICAL);
        int padding = (int) (24 * getResources().getDisplayMetrics().density);
        column.setPadding(padding, padding, padding, padding);
        scroll.setBackgroundColor(0xff111820);
        scroll.addView(column);
        TextView title = new TextView(this);
        title.setText(getString(R.string.launcher_title, BuildConfig.VERSION_NAME));
        title.setTextColor(0xffeeeeee); title.setTextSize(24);
        column.addView(title);
        TextView description = new TextView(this);
        description.setText(failure == null
            ? getString(R.string.launcher_description)
            : failure);
        description.setTextColor(0xffeeeeee); description.setTextSize(16);
        column.addView(description);
        Button start = new Button(this);
        start.setText(R.string.open_game); column.addView(start);
        start.setOnClickListener(view -> {
            start.setEnabled(false);
            description.setText(R.string.preparing_game);
            start.post(() -> {
                try {
                    StartupDiagnostics.record("webview-start-requested");
                    createWebView();
                } catch (RuntimeException | LinkageError e) {
                    StartupDiagnostics.record("webview-start-failed\n" + Log.getStackTraceString(e));
                    disposeWebView();
                    showFailure("Oyun ekranı açılamadı: " + e.getClass().getSimpleName() + "\n" + e.getMessage()
                        + "\nHata raporunu kopyalayıp paylaşabilirsin.");
                }
            });
        });
        Button copy = new Button(this);
        copy.setText(R.string.copy_report); column.addView(copy);
        copy.setOnClickListener(view -> {
            ClipboardManager clipboard = (ClipboardManager) getSystemService(CLIPBOARD_SERVICE);
            clipboard.setPrimaryClip(ClipData.newPlainText("Ravenfield diagnostics", StartupDiagnostics.report(this)));
            Toast.makeText(this, R.string.report_copied, Toast.LENGTH_LONG).show();
        });
        Button details = new Button(this);
        details.setText(R.string.show_report); column.addView(details);
        TextView report = new TextView(this);
        report.setTextColor(0xffeeeeee); report.setTextIsSelectable(true);
        column.addView(report);
        details.setOnClickListener(view -> report.setText(StartupDiagnostics.report(this)));
        setContentView(scroll);
        StartupDiagnostics.record("native-launcher-visible");
    }

    private void createWebView() {
        StartupDiagnostics.record("asset-handler-create");
        final WebViewAssetLoader loader;
        try {
            loader = new WebViewAssetLoader.Builder()
                .addPathHandler("/web/", path -> {
                    if (path.contains("..") || path.contains("\\")) return GameAssetHandler.error(403, "Forbidden");
                    try {
                        String mime = path.endsWith(".js") ? "application/javascript" : path.endsWith(".css") ? "text/css" : "text/html";
                        return new WebResourceResponse(mime, "UTF-8", getAssets().open("web/" + path));
                    } catch (Exception e) { return GameAssetHandler.error(404, "Not Found"); }
                })
                .addPathHandler("/game/", new GameAssetHandler(getAssets())).build();
        } catch (Exception e) {
            StartupDiagnostics.record("asset-handler-failed\n" + Log.getStackTraceString(e));
            showFailure("Oyun dosyaları bulunamadı. Build'e izinli Ravenfield dosyaları eklenmeli.");
            return;
        }
        StartupDiagnostics.record("webview-constructor-enter");
        web = new WebView(this);
        StartupDiagnostics.record("webview-constructor-complete");
        web.setBackgroundColor(0xff111820);
        WebView.setWebContentsDebuggingEnabled(BuildConfig.DEBUG);
        WebSettings settings = web.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        settings.setMediaPlaybackRequiresUserGesture(false);
        settings.setAllowFileAccess(false);
        settings.setAllowContentAccess(false);
        settings.setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);
        settings.setSupportZoom(false);
        settings.setCacheMode(WebSettings.LOAD_NO_CACHE);
        // Chromium provides WASM execution; no JNI runtime is bundled.
        web.setWebChromeClient(new WebChromeClient() {
            @Override public boolean onConsoleMessage(ConsoleMessage message) {
                Log.println(message.messageLevel() == ConsoleMessage.MessageLevel.ERROR ? Log.ERROR : Log.INFO,
                    "RavenfieldWeb", message.message());
                if (message.messageLevel() == ConsoleMessage.MessageLevel.ERROR || message.message().startsWith("[RavenPort]"))
                    StartupDiagnostics.record("web-console " + message.message());
                return true;
            }
        });
        web.setWebViewClient(new WebViewClient() {
            @Override public WebResourceResponse shouldInterceptRequest(WebView view, WebResourceRequest request) {
                Uri uri = request.getUrl();
                if (!"https".equals(uri.getScheme()) || !HOST.equals(uri.getHost())) return GameAssetHandler.error(403, "Forbidden");
                WebResourceResponse response = loader.shouldInterceptRequest(uri);
                return response != null ? response : GameAssetHandler.error(404, "Not Found");
            }
            @Override public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
                return !request.getUrl().toString().equals("https://" + HOST + "/web/index.html");
            }
            @Override public void onReceivedError(WebView view, WebResourceRequest request, WebResourceError error) {
                StartupDiagnostics.record("web-resource-error " + request.getUrl() + " code=" + error.getErrorCode()
                    + " " + error.getDescription());
                if (request.isForMainFrame()) view.post(() -> {
                    if (web != view) return;
                    disposeWebView();
                    showFailure("Yerel oyun ekranı yüklenemedi: " + error.getDescription());
                });
            }
            @Override public void onPageFinished(WebView view, String url) {
                StartupDiagnostics.record("page-finished " + url);
                requestGameAudio(); applyLifecycle();
            }
            @Override public boolean onRenderProcessGone(WebView view, RenderProcessGoneDetail detail) {
                StartupDiagnostics.record("renderer-gone didCrash=" + detail.didCrash()
                    + " priority=" + detail.rendererPriorityAtExit());
                disposeWebView();
                showFailure("WebView renderer kapandı. Hata raporunu kopyalayıp paylaşabilirsin."
                    + "\nRenderer crash: " + detail.didCrash());
                return true;
            }
        });
        setContentView(web);
        StartupDiagnostics.record("webview-load-local-page");
        web.loadUrl("https://" + HOST + "/web/index.html");
    }

    private void showFailure(String message) {
        showLauncher(message);
    }

    private void requestGameAudio() {
        if (!active || web == null || audioGranted) return;
        try {
            if (audio == null) {
                audio = (AudioManager) getSystemService(AUDIO_SERVICE);
                focusRequest = new AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                    .setAudioAttributes(new AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_GAME)
                        .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC).build())
                    .setOnAudioFocusChangeListener(change -> {
                        audioGranted = change == AudioManager.AUDIOFOCUS_GAIN;
                        applyLifecycle();
                    }).build();
            }
            audioGranted = audio.requestAudioFocus(focusRequest) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED;
            StartupDiagnostics.record("audio-focus granted=" + audioGranted);
        } catch (RuntimeException e) {
            StartupDiagnostics.record("audio-focus-failed\n" + Log.getStackTraceString(e));
        }
    }

    private void releaseGameAudio() {
        audioGranted = false;
        if (audio != null && focusRequest != null) {
            try { audio.abandonAudioFocusRequest(focusRequest); }
            catch (RuntimeException e) { StartupDiagnostics.record("audio-release-failed " + e); }
        }
    }

    private void disposeWebView() {
        WebView old = web;
        web = null;
        releaseGameAudio();
        if (old == null) return;
        if (old.getParent() instanceof ViewGroup) ((ViewGroup) old.getParent()).removeView(old);
        try { old.destroy(); }
        catch (RuntimeException e) { StartupDiagnostics.record("webview-destroy-failed " + e); }
    }

    private void applyLifecycle() {
        if (web == null) return;
        boolean paused = !active || !audioGranted;
        web.evaluateJavascript("window.RavenPort && RavenPort.setHostPaused(" + paused + ")", null);
    }

    @Override protected void onResume() {
        super.onResume(); active = true;
        StartupDiagnostics.record("activity-resume");
        requestGameAudio();
        if (web != null) web.onResume();
        applyLifecycle(); immersive();
    }
    @Override protected void onPause() {
        active = false; applyLifecycle();
        if (web != null) web.onPause();
        releaseGameAudio();
        super.onPause();
    }
    @Override protected void onDestroy() {
        disposeWebView();
        super.onDestroy();
    }
    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        if (focused) { immersive(); requestGameAudio(); applyLifecycle(); }
        else if (web != null) web.evaluateJavascript("window.RavenPort && RavenPort.releaseInput()", null);
    }
    @Override public void onBackPressed() {
        handleBack();
    }
    private void handleBack() {
        if (web != null) web.evaluateJavascript("window.RavenPort && RavenPort.togglePanel()", null);
        else finish();
    }
    @Override public void onTrimMemory(int level) {
        super.onTrimMemory(level);
        Log.i("Ravenfield", "Memory pressure: " + level);
        // Clearing WebView data or forcing GC here would disrupt a live Unity game.
    }
    private void immersive() {
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsetsController controller = getWindow().getInsetsController();
            if (controller != null) {
                controller.hide(WindowInsets.Type.systemBars());
                controller.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
            }
        } else {
            getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_FULLSCREEN |
                View.SYSTEM_UI_FLAG_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY |
                View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION |
                View.SYSTEM_UI_FLAG_LAYOUT_STABLE);
        }
    }
}
