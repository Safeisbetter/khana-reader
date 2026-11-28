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
 * Answers every request of the WebView inside the app. The page's address, {@code https://khana.invalid},
 * is a name that can never be looked up (RFC 6761): nothing reaches a network, and the app has no
 * permission to use one. A request for any other address is refused here.
 * <p>
 * WebView does not pass request bodies on, so the page sends a body first through a message port
 * that only the reader page holds (web/android.js), in base64 pieces, then names it in the request
 * (X-Khana-Body). A body is used once and dropped.
 */
final class Server {
    /** When speech was last asked for (reading aloud): MainActivity keeps the screen on while this is recent. */
    static volatile long lastSpeech;

    static final String HOST = "khana.invalid";
    static final String ORIGIN = "https://" + HOST;
    static final String PDF_HOST = "khana-pdf.invalid";
    @SuppressWarnings("unused")
    static final String PDF_ORIGIN = "https://" + PDF_HOST;
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
            long need = (long) n + len;
            if (need > MAX_BODY) throw new IllegalArgumentException();
            if (need > buf.length) {
                int cap = (int) Math.min(MAX_BODY, Math.max((long) buf.length * 2, need));
                byte[] bigger = new byte[cap];
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
    synchronized void clearBodies() {
        for (Body b : bodies.values()) b.wipe();
        bodies.clear();
    }

    private synchronized byte[] takeBody(String id) {
        Body b = bodies.remove(id);
        return b == null ? null : b.take();
    }

    /** Bound both individual uploads and the aggregate retained by abandoned requests. */
    synchronized void onPortMessage(WebMessagePort port, WebMessage m) {
        String d = m.getData();
        if (d == null || d.isEmpty()) return;
        char op = d.charAt(0);
        int c = op == 'B' ? d.indexOf(':') : d.length();
        if ((op != 'B' && op != 'E') || c < 2 || c > 65) return;
        String id = d.substring(1, c);
        if (!id.matches("[A-Za-z0-9-]{1,64}")) return;
        byte[] piece = null;
        try {
            if (op == 'B') {
                // The page sends 3 MiB pieces; check before allocating the decoded bytes.
                if (d.length() - c - 1 > 4 * 1024 * 1024) throw new IllegalArgumentException();
                piece = Base64.decode(d.substring(c + 1), Base64.DEFAULT);
            }
            long total = 0;
            for (Body pending : bodies.values()) total += pending.size();
            if ((!bodies.containsKey(id) && bodies.size() >= 8)
                    || total + (piece == null ? 0 : piece.length) > MAX_BODY) throw new IllegalArgumentException();
            Body b = bodies.computeIfAbsent(id, k -> new Body());
            if (piece != null) b.write(piece, piece.length);
            else port.postMessage(new WebMessage("A" + id));
        } catch (IllegalArgumentException | OutOfMemoryError e) {
            // Clear the channel's incomplete uploads; never acknowledge a truncated body as complete.
            clearBodies();
            port.postMessage(new WebMessage("F" + id));
        } finally {
            if (piece != null) Arrays.fill(piece, (byte) 0);
        }
    }

    WebResourceResponse handle(WebResourceRequest req) {
        Uri u = req.getUrl();
        if (!"https".equals(u.getScheme()) || (u.getPort() != -1 && u.getPort() != 443) || u.getUserInfo() != null) return refuse();
        String host = u.getHost();
        boolean isApp = HOST.equals(host);
        boolean isPdf = PDF_HOST.equals(host);
        if (!isApp && !isPdf) return refuse();
        String method = req.getMethod() == null ? "GET" : req.getMethod().toUpperCase(Locale.ROOT);
        // (as the Mac window passes them: the path and query still percent-encoded)
        String path = u.getEncodedPath() == null || u.getEncodedPath().isEmpty() ? "/" : u.getEncodedPath();
        // Site isolation: khana-pdf.invalid only serves GET /pdfjs/*; khana.invalid never serves /pdfjs/*.
        if (isPdf && (!"GET".equals(method) || !path.startsWith("/pdfjs/"))) return refuse();
        if (isApp && path.startsWith("/pdfjs/")) return refuse();
        String query = u.getEncodedQuery() == null ? "" : u.getEncodedQuery();
        String tok = header(req, "X-Khana-Token");
        byte[] body = null;
        String bodyId = header(req, "X-Khana-Body");
        if (bodyId != null) {
            if (!tokenOk(tok)) return json(403, "{}");
            body = takeBody(bodyId);
            if (body == null) return text(400, "the request's body did not arrive");
        }
        try {
            if (path.startsWith("/api/read_")) {
                if (!tokenOk(tok)) return json(403, "{}");
                if ("POST".equals(method) && "/api/read_stop".equals(path)) {
                    ReadService.stop(ctx); return json(200, "{\"ok\":true}");
                }
                if (isLocked()) { ReadService.stop(ctx); AudioFileService.stop(ctx); return json(423, "{\"locked\":true}"); }
                if ("GET".equals(method) && "/api/read_status".equals(path)) return json(200, ReadService.status());
                if (!"POST".equals(method) || body == null || body.length > 4096) return json(400, "{}");
                try {
                    org.json.JSONObject j = new org.json.JSONObject(Native.text(body));
                    if ("/api/read_start".equals(path)) { AudioFileService.stop(ctx); ReadService.start(ctx, j); }
                    else if ("/api/read_pause".equals(path)) ReadService.pause(j.optBoolean("paused", true));
                    else return json(404, "{}");
                    return json(200, "{\"ok\":true}");
                } catch (Exception e) { return json(400, "{\"error\":\"خواندن شروع نشد. دوباره تلاش کنید.\"}"); }
            }
            if (path.startsWith("/api/lib_play_")) {
                if (!tokenOk(tok)) return json(403, "{}");
                if ("POST".equals(method) && "/api/lib_play_stop".equals(path)) {
                    AudioFileService.stop(ctx); return json(200, "{\"ok\":true}");
                }
                if (isLocked()) { ReadService.stop(ctx); AudioFileService.stop(ctx); return json(423, "{\"locked\":true}"); }
                if ("GET".equals(method) && "/api/lib_play_status".equals(path)) return json(200, AudioFileService.status());
                if (!"POST".equals(method) || body == null || body.length > 2048) return json(400, "{}");
                try {
                    org.json.JSONObject j = new org.json.JSONObject(Native.text(body));
                    if ("/api/lib_play_start".equals(path)) AudioFileService.start(ctx, j.optString("name", ""));
                    else if ("/api/lib_play_pause".equals(path)) AudioFileService.pause(j.optBoolean("paused", true));
                    else if ("/api/lib_play_seek".equals(path)) AudioFileService.seek(j.optInt("positionMs", 0));
                    else return json(404, "{}");
                    return json(200, "{\"ok\":true}");
                } catch (Exception e) { return json(400, "{\"error\":\"پخش فایل شروع نشد.\"}"); }
            }
            // a voice sample other than WAV is decoded by Android first (the Mac uses afconvert); only for the page itself
            if ("POST".equals(method) && "/api/myvoice".equals(path) && body != null) {
                if (!tokenOk(tok)) {
                    Arrays.fill(body, (byte) 0);
                    return json(403, "{}");
                }
                if (body.length > 50 * 1024 * 1024) {
                    Arrays.fill(body, (byte) 0);
                    return json(400, "{\"error\":\"فایل صوتی خیلی بزرگ است. یک ضبط کوتاه (۱۰ تا ۳۰ ثانیه) کافی است.\"}");
                }
                String name = u.getQueryParameter("name");
                if (name != null && !name.toLowerCase(Locale.ROOT).endsWith(".wav")) {
                    if (isLocked()) return json(423, "{\"locked\":true}");
                    byte[] wav = AudioDecode.toWav(ctx, body);
                    Arrays.fill(body, (byte) 0);
                    if (wav == null) return json(400, "{\"error\":\"این فایل صوتی باز نشد. یک فایل WAV، M4A یا MP3 انتخاب کنید.\"}");
                    body = wav;
                    query = query.replaceAll("(^|&)name=[^&]*", "$1name=voice.wav");
                }
            }
            // The page's «copy page text»: put on the clipboard here, not by the web view, so that it can be marked as sensitive
            // (Android 13 and newer then keep it out of the clipboard preview) and taken off again after a minute and a half
            // (on Android 9 and older any app can read the clipboard for as long as the text lies there).
            // The page's colours for the shell's system-bar margins («رنگ پس‌زمینه»): the margins are not part of the page, so the page says
            // what it looks like now, «#rrggbb #rrggbb» (ground, writing). Only two colours are read, nothing else.
            if ("POST".equals(method) && "/api/shellbar".equals(path)) {
                if (!tokenOk(tok)) return json(403, "{}");
                if (body != null && body.length > 64) return json(400, "{}");
                String t = body == null ? "" : Native.text(body);
                java.util.regex.Matcher m = java.util.regex.Pattern.compile("^#([0-9a-fA-F]{6}) #([0-9a-fA-F]{6})$").matcher(t.trim());
                if (!m.find()) return json(400, "{}");
                String g1 = m.group(1), g2 = m.group(2);
                if (g1 == null || g2 == null) return json(400, "{}");
                MainActivity.paintBar(0xFF000000 | Integer.parseInt(g1, 16), 0xFF000000 | Integer.parseInt(g2, 16));
                return json(200, "{\"ok\":true}");
            }
            if ("POST".equals(method) && "/api/clip".equals(path)) {
                if (!tokenOk(tok)) return json(403, "{}");
                if (isLocked()) { // (the app lock: this request does not go through the core, so it asks)
                    if (body != null) Arrays.fill(body, (byte) 0);
                    return json(423, "{\"locked\":true}");
                }
                if (body != null && body.length > 512 * 1024) {
                    Arrays.fill(body, (byte) 0);
                    return json(413, "{\"error\":\"متن برای کپی خیلی بزرگ است.\"}");
                }
                copySensitive(ctx, body == null ? "" : Native.text(body));
                if (body != null) Arrays.fill(body, (byte) 0);
                return json(200, "{\"ok\":true}");
            }
            if ("POST".equals(method) && tokenOk(tok) && ("/api/open".equals(path) || "/api/open_layout".equals(path) || "/api/open_pages".equals(path) || "/api/close".equals(path) || "/api/lock_now".equals(path) || "/api/forget".equals(path) || "/api/lock_erase".equals(path))) ReadService.stop(ctx);
            if ("POST".equals(method) && tokenOk(tok) && ("/api/lock_now".equals(path) || "/api/forget".equals(path) || "/api/lock_erase".equals(path))) AudioFileService.stop(ctx);
            if ("POST".equals(method) && tokenOk(tok) && "/api/library_delete".equals(path)) AudioFileService.stopIfPlaying(ctx, u.getQueryParameter("name"));
            if ("POST".equals(method) && tokenOk(tok) && ("/api/close".equals(path) || "/api/lock_now".equals(path) || "/api/forget".equals(path) || "/api/lock_erase".equals(path))) clearClipboard(ctx);
            Native.Reply r = new Native.Reply(Native.handle(Native.utf8(method), Native.utf8(path), Native.utf8(query), Native.utf8(tok), body));
            if (body != null) Arrays.fill(body, (byte) 0); // (the document's bytes: the core has its own copy now)
            if ("/api/tts".equals(path)) lastSpeech = android.os.SystemClock.elapsedRealtime();
            if (r.status == 200 && "POST".equals(method) && ("/api/forget".equals(path) || "/api/lock_erase".equals(path))) {
                Exporter.clearAll(ctx);
                MainActivity.clearWebData();
            }
            if (r.status == 423 || (r.status == 200 && "POST".equals(method) && ("/api/settings".equals(path) || "/api/forget".equals(path) || "/api/unlock".equals(path) || "/api/lock_now".equals(path)))) {
                MainActivity.applyScreenGuard();
            }
            if (r.status == 200 && "POST".equals(method) && "/api/unlock".equals(path)) {
                Exporter.onUnlocked();
            }
            if (r.status == 200 && "POST".equals(method) && ("/api/mp3".equals(path) || "/api/pdf".equals(path))) {
                JobService.start(ctx);
            }
            if (path.startsWith("/api/") && r.status != 423) {
                boolean runningJobPoll = "/api/job".equals(path) && Native.text(r.body).contains("\"status\":\"running\"");
                if (!runningJobPoll) Exporter.drain(ctx);
            }
            return response(r, isApp);
        } finally {
            if (body != null) Arrays.fill(body, (byte) 0);
        }
    }

    /** Whether the app lock is on and the app locked now (the core says so; it refuses the request itself otherwise). */
    static boolean isLocked() {
        try {
            Native.Reply r = new Native.Reply(Native.handle(Native.utf8("GET"), Native.utf8("/api/lock_status"), Native.utf8(""), Native.token(), null));
            return Native.text(r.body).replace(" ", "").contains("\"locked\":true");
        } catch (RuntimeException e) {
            return true; // (cannot tell: shut)
        }
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

    private static WebResourceResponse response(Native.Reply r, boolean isApp) {
        String ctype = r.headers.getOrDefault("Content-Type", "application/octet-stream");
        if (ctype == null) ctype = "application/octet-stream";
        String mime = ctype, charset = null;
        int semi = ctype.indexOf(';');
        if (semi >= 0) {
            mime = ctype.substring(0, semi).trim();
            int cs = ctype.toLowerCase(Locale.ROOT).indexOf("charset=");
            if (cs >= 0) charset = ctype.substring(cs + 8).trim();
        }
        Map<String, String> headers = new HashMap<>(noStore());
        headers.put("Referrer-Policy", "no-referrer");
        if (isApp) {
            headers.put("Cross-Origin-Opener-Policy", "same-origin");
            headers.put("Cross-Origin-Resource-Policy", "same-origin");
        }
        headers.putAll(r.headers);
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
    private static final long CLIP_LIFETIME_MS = 90_000;
    // Used only on the main thread. An old timer must never clear a newer copy.
    private static long clipboardExpiresAt;
    private static long clipboardGeneration;

    static void clearClipboard(Context ctx) {
        new android.os.Handler(android.os.Looper.getMainLooper()).post(() -> tryClearClipboard(ctx));
    }

    static void clearExpiredClipboard(Context ctx) {
        if (clipboardExpiresAt != 0 && android.os.SystemClock.elapsedRealtime() >= clipboardExpiresAt) {
            tryClearClipboard(ctx);
        }
    }

    private static void tryClearClipboard(Context ctx) {
        android.content.ClipboardManager cm = (android.content.ClipboardManager) ctx.getSystemService(Context.CLIPBOARD_SERVICE);
        if (cm == null) return;
        try {
            // Android may hide the clipboard while the app is in the background.
            android.content.ClipDescription d = cm.getPrimaryClipDescription();
            if (d == null) return;
            if (d.getLabel() != null && CLIP_LABEL.contentEquals(d.getLabel())) {
                if (android.os.Build.VERSION.SDK_INT >= 28) cm.clearPrimaryClip();
                else cm.setPrimaryClip(android.content.ClipData.newPlainText("", ""));
            }
            clipboardExpiresAt = 0;
        } catch (Exception ignored) {
            // The next window focus retries an expired copy.
        }
    }

    private static void copySensitive(Context ctx, String text) {
        android.content.ClipboardManager cm = (android.content.ClipboardManager) ctx.getSystemService(Context.CLIPBOARD_SERVICE);
        if (cm == null) return;
        android.content.ClipData clip = android.content.ClipData.newPlainText(CLIP_LABEL, text);
        android.os.PersistableBundle extras = new android.os.PersistableBundle();
        extras.putBoolean("android.content.extra.IS_SENSITIVE", true);
        clip.getDescription().setExtras(extras);
        android.os.Handler main = new android.os.Handler(android.os.Looper.getMainLooper());
        main.post(() -> {
            try {
                if (isLocked()) return;
                cm.setPrimaryClip(clip);
            } catch (Exception e) {
                return;
            }
            long generation = ++clipboardGeneration;
            clipboardExpiresAt = android.os.SystemClock.elapsedRealtime() + CLIP_LIFETIME_MS;
            main.postDelayed(() -> {
                if (generation == clipboardGeneration) clearExpiredClipboard(ctx);
            }, CLIP_LIFETIME_MS);
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
        h.put("Origin-Agent-Cluster", "?1");
        // (the fallback policy of every reply that brings none of its own: nothing at all may run or load in it. The page, pdf.js's
        // frame and its scripts send their own policies from the core, which replace this one)
        h.put("Content-Security-Policy", "default-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'");
        // (1.29: and no power either, as GrapheneOS's PDF Viewer sends with every reply: no motion, orientation or light sensors,
        // microphone, camera, location … The page and pdf.js's frame send their own lists from the core, which replace this one)
        h.put("Permissions-Policy", "accelerometer=(), ambient-light-sensor=(), autoplay=(), battery=(), bluetooth=(), camera=(), "
            + "clipboard-read=(), clipboard-write=(), display-capture=(), document-domain=(), encrypted-media=(), fullscreen=(), gamepad=(), "
            + "geolocation=(), gyroscope=(), hid=(), idle-detection=(), interest-cohort=(), local-fonts=(), magnetometer=(), microphone=(), "
            + "midi=(), payment=(), picture-in-picture=(), publickey-credentials-get=(), screen-wake-lock=(), serial=(), speaker-selection=(), "
            + "storage-access=(), sync-xhr=(), usb=(), web-share=(), window-management=(), xr-spatial-tracking=()");
        return h;
    }
}
