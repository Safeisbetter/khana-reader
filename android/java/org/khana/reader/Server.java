package org.khana.reader;

import android.content.Context;
import android.net.Uri;
import android.util.Base64;
import android.webkit.WebMessage;
import android.webkit.WebMessagePort;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import java.io.ByteArrayInputStream;
import java.util.Arrays;
import java.util.HashMap;
import java.util.Locale;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Answers every request of the WebView inside the app. The page's address, https://khana.invalid,
 * is a name that can never be looked up (RFC 6761): nothing reaches a network, and the app has no
 * permission to use one. A request for any other address is refused here.
 *
 * WebView does not pass request bodies on, so the page sends a body first through a message port
 * that only the reader page holds (web/android.js), in base64 pieces, then names it in the request
 * (X-Khana-Body). A body is used once and dropped.
 */
final class Server {
    /** When speech was last asked for (reading aloud): MainActivity keeps the screen on while this is recent. */
    static volatile long lastSpeech;

    static final String HOST = "khana.invalid";
    static final String ORIGIN = "https://" + HOST;
    /** bodies larger than this are refused (the app's own limit for a document is 300 MB) */
    private static final long MAX_BODY = 420L * 1024 * 1024;

    private final Context ctx;
    private final String token;
    private final Map<String, Body> bodies = new ConcurrentHashMap<>();

    /** A request's body as it arrives in pieces: a buffer that is overwritten when it is let go (a ByteArrayOutputStream's is not). */
    static final class Body {
        private byte[] buf = new byte[1 << 16];
        private int n;

        synchronized long size() {
            return n;
        }

        synchronized void write(byte[] p, int len) {
            if (n + len > buf.length) {
                byte[] bigger = new byte[(int) Math.min(Integer.MAX_VALUE - 8, Math.max((long) buf.length * 2, (long) n + len))];
                System.arraycopy(buf, 0, bigger, 0, n);
                Arrays.fill(buf, (byte) 0);
                buf = bigger;
            }
            System.arraycopy(p, 0, buf, n, len);
            n += len;
        }

        /** The bytes, once; the buffer itself is overwritten. */
        synchronized byte[] take() {
            byte[] r = Arrays.copyOf(buf, n);
            wipe();
            return r;
        }

        synchronized void wipe() {
            Arrays.fill(buf, (byte) 0);
            n = 0;
        }
    }

    /** A reply's bytes, overwritten when the WebView has read them and closes the stream. */
    private static final class WipingStream extends ByteArrayInputStream {
        WipingStream(byte[] b) {
            super(b);
        }

        @Override
        public void close() {
            Arrays.fill(buf, (byte) 0);
        }
    }

    Server(Context ctx) {
        this.ctx = ctx.getApplicationContext();
        this.token = Native.text(Native.token());
    }

    String token() {
        return token;
    }

    /** A message from the page's port: «B<id>:<base64>» a piece of a body, «E<id>» its end. */
    void onPortMessage(WebMessagePort port, WebMessage m) {
        String d = m.getData();
        if (d == null || d.isEmpty()) return;
        if (d.charAt(0) == 'B') {
            int c = d.indexOf(':');
            if (c < 2) return;
            String id = d.substring(1, c);
            byte[] piece = Base64.decode(d.substring(c + 1), Base64.DEFAULT);
            Body b = bodies.computeIfAbsent(id, k -> new Body());
            if (b.size() + (long) piece.length > MAX_BODY) {
                bodies.remove(id);
                b.wipe();
            } else {
                b.write(piece, piece.length);
            }
            Arrays.fill(piece, (byte) 0);
        } else if (d.charAt(0) == 'E') {
            String id = d.substring(1);
            bodies.computeIfAbsent(id, k -> new Body());
            port.postMessage(new WebMessage("A" + id));
        }
    }

    WebResourceResponse handle(WebResourceRequest req) {
        Uri u = req.getUrl();
        if (!"https".equals(u.getScheme()) || !HOST.equals(u.getHost())) return refuse();
        String method = req.getMethod() == null ? "GET" : req.getMethod().toUpperCase(Locale.ROOT);
        // (as the Mac window passes them: the path and query still percent-encoded)
        String path = u.getEncodedPath() == null || u.getEncodedPath().isEmpty() ? "/" : u.getEncodedPath();
        String query = u.getEncodedQuery() == null ? "" : u.getEncodedQuery();
        String tok = header(req, "X-Khana-Token");
        byte[] body = null;
        String bodyId = header(req, "X-Khana-Body");
        if (bodyId != null) {
            Body b = bodies.remove(bodyId);
            if (b == null) return text(400, "the request's body did not arrive");
            body = b.take();
        }
        // a voice sample other than WAV is decoded by Android first (the Mac uses afconvert); only for the page itself
        if ("/api/myvoice".equals(path) && body != null) {
            if (!tokenOk(tok)) {
                Arrays.fill(body, (byte) 0);
                return json(403, "{}");
            }
            String name = u.getQueryParameter("name");
            if (name != null && !name.toLowerCase(Locale.ROOT).endsWith(".wav")) {
                byte[] wav = AudioDecode.toWav(ctx, body);
                if (wav == null) return json(400, "{\"error\":\"این فایل صوتی باز نشد. یک فایل WAV، M4A یا MP3 انتخاب کنید.\"}");
                body = wav;
                query = query.replaceAll("(^|&)name=[^&]*", "$1name=voice.wav");
            }
        }
        // The page's «copy page text»: put on the clipboard here, not by the web view, so that it can be marked as sensitive
        // (Android 13 and newer then keep it out of the clipboard preview) and taken off again after a minute and a half
        // (on Android 9 and older any app can read the clipboard for as long as the text lies there).
        if ("POST".equals(method) && "/api/clip".equals(path)) {
            if (!tokenOk(tok)) return json(403, "{}");
            copySensitive(ctx, body == null ? "" : Native.text(body));
            if (body != null) Arrays.fill(body, (byte) 0);
            return json(200, "{\"ok\":true}");
        }
        Native.Reply r = new Native.Reply(Native.handle(Native.utf8(method), Native.utf8(path), Native.utf8(query), Native.utf8(tok), body));
        if (body != null) Arrays.fill(body, (byte) 0); // (the document's bytes: the core has its own copy now)
        if ("/api/tts".equals(path)) lastSpeech = android.os.SystemClock.elapsedRealtime();
        if (r.status == 200 && "POST".equals(method) && ("/api/settings".equals(path) || "/api/forget".equals(path))) MainActivity.applyScreenGuard();
        if (r.status == 200 && "POST".equals(method) && ("/api/mp3".equals(path) || "/api/pdf".equals(path))) {
            JobService.start(ctx);
        }
        if (path.startsWith("/api/")) Exporter.drain(ctx);
        return response(r);
    }

    /** The page's token, compared in constant time (MessageDigest.isEqual). */
    private boolean tokenOk(String tok) {
        return tok != null && !token.isEmpty() && java.security.MessageDigest.isEqual(Native.utf8(tok), Native.utf8(token));
    }

    private static String header(WebResourceRequest req, String name) {
        for (Map.Entry<String, String> e : req.getRequestHeaders().entrySet()) {
            if (e.getKey() != null && e.getKey().equalsIgnoreCase(name)) return e.getValue();
        }
        return null;
    }

    private static WebResourceResponse response(Native.Reply r) {
        String ctype = r.headers.getOrDefault("Content-Type", "application/octet-stream");
        String mime = ctype, charset = null;
        int semi = ctype.indexOf(';');
        if (semi >= 0) {
            mime = ctype.substring(0, semi).trim();
            int cs = ctype.toLowerCase(Locale.ROOT).indexOf("charset=");
            if (cs >= 0) charset = ctype.substring(cs + 8).trim();
        }
        Map<String, String> headers = new HashMap<>(r.headers);
        headers.remove("Content-Type");
        int status = r.status >= 300 && r.status < 400 || r.status < 100 || r.status > 599 ? 500 : r.status;
        return new WebResourceResponse(mime, charset, status, reason(status), headers, new WipingStream(r.body));
    }

    private static String reason(int s) {
        switch (s) {
            case 200: return "OK";
            case 400: return "Bad Request";
            case 403: return "Forbidden";
            case 404: return "Not Found";
            default: return s < 400 ? "OK" : "Error";
        }
    }

    private static WebResourceResponse text(int status, String msg) {
        return new WebResourceResponse("text/plain", "utf-8", status, reason(status), noStore(), new ByteArrayInputStream(Native.utf8(msg)));
    }

    private static final String CLIP_LABEL = "Khana";

    private static void copySensitive(Context ctx, String text) {
        final android.content.ClipboardManager cm = (android.content.ClipboardManager) ctx.getSystemService(Context.CLIPBOARD_SERVICE);
        if (cm == null) return;
        android.content.ClipData clip = android.content.ClipData.newPlainText(CLIP_LABEL, text);
        android.os.PersistableBundle extras = new android.os.PersistableBundle();
        extras.putBoolean("android.content.extra.IS_SENSITIVE", true);
        if (android.os.Build.VERSION.SDK_INT >= 24) clip.getDescription().setExtras(extras);
        new android.os.Handler(android.os.Looper.getMainLooper()).post(() -> {
            try {
                cm.setPrimaryClip(clip);
            } catch (Throwable e) {
                return;
            }
            new android.os.Handler(android.os.Looper.getMainLooper()).postDelayed(() -> {
                try {
                    // (only if it is still this app's text: what the reader copied elsewhere since is left alone)
                    android.content.ClipDescription d = cm.getPrimaryClipDescription();
                    if (d == null || d.getLabel() == null || !CLIP_LABEL.contentEquals(d.getLabel())) return;
                    if (android.os.Build.VERSION.SDK_INT >= 28) cm.clearPrimaryClip();
                    else cm.setPrimaryClip(android.content.ClipData.newPlainText("", ""));
                } catch (Throwable e) {
                    // (the clipboard cannot be read from the background on newer Android: the text stays until something else is copied)
                }
            }, 90_000);
        });
    }

    private static WebResourceResponse json(int status, String body) {
        return new WebResourceResponse("application/json", "utf-8", status, reason(status), noStore(), new ByteArrayInputStream(Native.utf8(body)));
    }

    /** Any other address: refused, never fetched. */
    static WebResourceResponse refuse() {
        return text(403, "");
    }

    private static Map<String, String> noStore() {
        Map<String, String> h = new HashMap<>();
        h.put("Cache-Control", "no-store");
        h.put("X-Content-Type-Options", "nosniff");
        return h;
    }
}
