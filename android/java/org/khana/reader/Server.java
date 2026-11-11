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
        // a voice sample other than WAV is decoded by Android first (the Mac uses afconvert)
        if ("/api/myvoice".equals(path) && body != null) {
            String name = u.getQueryParameter("name");
            if (name != null && !name.toLowerCase(Locale.ROOT).endsWith(".wav")) {
                byte[] wav = AudioDecode.toWav(ctx, body);
                if (wav == null) return json(400, "{\"error\":\"این فایل صوتی باز نشد. یک فایل WAV، M4A یا MP3 انتخاب کنید.\"}");
                body = wav;
                query = query.replaceAll("(^|&)name=[^&]*", "$1name=voice.wav");
            }
        }
        Native.Reply r = new Native.Reply(Native.handle(Native.utf8(method), Native.utf8(path), Native.utf8(query), Native.utf8(tok), body));
        if (body != null) Arrays.fill(body, (byte) 0); // (the document's bytes: the core has its own copy now)
        if ("/api/tts".equals(path)) lastSpeech = android.os.SystemClock.elapsedRealtime();
        if (r.status == 200 && "POST".equals(method) && ("/api/mp3".equals(path) || "/api/pdf".equals(path))) {
            JobService.start(ctx);
        }
        if (path.startsWith("/api/")) Exporter.drain(ctx);
        return response(r);
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
