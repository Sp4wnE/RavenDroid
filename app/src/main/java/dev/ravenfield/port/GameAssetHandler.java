package dev.ravenfield.port;

import android.content.res.AssetManager;
import android.util.Log;
import android.webkit.WebResourceResponse;
import androidx.webkit.WebViewAssetLoader;
import org.json.JSONArray;
import org.json.JSONObject;
import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Locale;
import java.util.Map;
import java.util.zip.GZIPInputStream;

final class GameAssetHandler implements WebViewAssetLoader.PathHandler {
    private final AssetManager assets;
    private final JSONObject manifest;

    GameAssetHandler(AssetManager assets) throws Exception {
        this.assets = assets;
        try (InputStream in = assets.open("game/manifest.json")) {
            manifest = new JSONObject(new String(readSmall(in), StandardCharsets.UTF_8));
        }
    }

    private static byte[] readSmall(InputStream in) throws IOException {
        java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
        byte[] buf = new byte[4096];
        int n;
        while ((n = in.read(buf)) != -1) {
            if (out.size() + n > 1024 * 1024) throw new IOException("Manifest too large");
            out.write(buf, 0, n);
        }
        return out.toByteArray();
    }

    @Override public WebResourceResponse handle(String path) {
        if (path.contains("..") || path.contains("\\") || path.startsWith("/")) return error(403, "Forbidden");
        try {
            InputStream stream;
            String mime;
            long length = -1;
            if (path.equals("game.data")) {
                JSONArray parts = manifest.getJSONArray("dataParts");
                stream = new GZIPInputStream(new PartInputStream(parts.length(), index -> {
                    try { return assets.open("game/" + parts.getString(index), AssetManager.ACCESS_STREAMING); }
                    catch (org.json.JSONException e) { throw new IOException(e); }
                }), 65536);
                mime = "application/octet-stream";
                length = manifest.getLong("dataSize");
            } else if (path.equals("game.wasm")) {
                stream = new GZIPInputStream(assets.open("game/game.wasm.bin", AssetManager.ACCESS_STREAMING), 65536);
                mime = "application/wasm";
                length = manifest.getLong("wasmSize");
            } else if (path.equals("loader.js") || path.equals("framework.js") || path.equals("manifest.json") || path.startsWith("StreamingAssets/")) {
                stream = assets.open("game/" + path, AssetManager.ACCESS_STREAMING);
                mime = path.endsWith(".js") ? "application/javascript" : path.endsWith(".json") ? "application/json" : "application/octet-stream";
            } else return error(404, "Not Found");
            Map<String, String> headers = new HashMap<>();
            headers.put("Cache-Control", "no-store");
            headers.put("X-Content-Type-Options", "nosniff");
            if (length >= 0) headers.put("Content-Length", String.format(Locale.ROOT, "%d", length));
            // Stream is already decompressed. Do NOT add Content-Encoding: gzip.
            return new WebResourceResponse(mime, null, 200, "OK", headers, stream);
        } catch (Exception e) {
            Log.e("RavenfieldAssets", "Cannot serve " + path, e);
            return error(404, "Not Found");
        }
    }

    static WebResourceResponse error(int status, String reason) {
        return new WebResourceResponse("text/plain", "UTF-8", status, reason,
            java.util.Collections.singletonMap("Cache-Control", "no-store"),
            new ByteArrayInputStream(reason.getBytes(StandardCharsets.UTF_8)));
    }
}
