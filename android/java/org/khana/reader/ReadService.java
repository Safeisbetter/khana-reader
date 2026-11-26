package org.khana.reader;

import android.app.*;
import android.content.*;
import android.content.pm.ServiceInfo;
import android.graphics.drawable.Icon;
import android.media.*;
import android.media.session.*;
import android.os.*;
import org.json.*;
import java.nio.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.*;

/** Offline native playback: the same player owns reading with the display on or off. Never exported. */
public final class ReadService extends Service {
    private static final String CHANNEL = "reading";
    private static final int RATE = 24000;
    private static volatile ReadService live;
    private static volatile String lastError = "";
    private static volatile String lastStopReason = "";
    private final Handler ui = new Handler(Looper.getMainLooper());
    private volatile Run run;
    private PowerManager.WakeLock wake;
    private AudioFocusRequest focus;
    private MediaSession media;
    private AudioManager manager;
    private BroadcastReceiver noisy;

    static void start(Context context, JSONObject request) {
        lastError = "";
        lastStopReason = "";
        context.startForegroundService(new Intent(context, ReadService.class).setAction("start").putExtra("request", request.toString()));
    }
    static void stop(Context context) {
        lastStopReason = "";
        ReadService s = live;
        if (s != null) {
            s.cancel();
            context.stopService(new Intent(context, ReadService.class));
        }
    }
    static boolean reading() { ReadService s = live; return s != null && s.run != null && !s.run.cancelled; }
    static void pause(boolean value) { ReadService s = live; if (s != null) s.ui.post(() -> s.setPaused(value, value ? "خواندن با دستور شما مکث شد." : "")); }
    static String status() {
        ReadService s = live;
        Run r = s == null ? null : s.run;
        JSONObject out = new JSONObject();
        try {
            out.put("active", r != null && !r.cancelled).put("error", lastError).put("stopReason", lastStopReason);
            if (r != null && !r.cancelled) {
                long head = r.track == null ? r.baseFrame : Integer.toUnsignedLong(r.track.getPlaybackHeadPosition());
                while (r.spans.size() > 1) {
                    Span first = r.spans.peekFirst();
                    if (first == null || head < first.endFrame) break;
                    r.spans.pollFirst();
                }
                Span cur = r.spans.peekFirst();
                int page = cur != null ? cur.page : r.page;
                int para = cur != null ? cur.para : r.para;
                int start = cur != null ? cur.start : r.start;
                int end = cur != null ? cur.end : r.end;
                double duration = cur != null ? cur.duration : r.duration;
                double gap = cur != null ? cur.gap : r.gap;
                long base = cur != null ? cur.startFrame : r.baseFrame;
                out.put("session", r.id).put("doc", r.request.getString("doc")).put("paused", r.paused)
                    .put("page", page).put("para", para).put("start", start).put("end", end)
                    .put("duration", duration).put("stalls", r.stalls.get()).put("stalled", r.stalled.get())
                    .put("phase", r.paused ? "paused" : r.phase)
                    .put("waitReason", r.paused ? "" : r.waitReason)
                    .put("pauseReason", r.paused ? r.pauseReason : "")
                    .put("progress", r.paused ? -1 : r.waitProgress)
                    .put("elapsed", Math.max(0, (head - base) / (double) RATE - gap));
            }
        } catch (Exception ignored) { }
        return out.toString();
    }
    private static final class Span {
        final long startFrame, endFrame;
        final int page, para, start, end;
        final double duration, gap;
        Span(long startFrame, long endFrame, int page, int para, int start, int end, double duration, double gap) {
            this.startFrame = startFrame; this.endFrame = endFrame;
            this.page = page; this.para = para; this.start = start; this.end = end;
            this.duration = duration; this.gap = gap;
        }
    }
    private static final class Sound {
        byte[] pcm;
        int page, para, start, end;
        double gap, duration;
        boolean endOfBook;
        void wipe() { if (pcm != null) Arrays.fill(pcm, (byte) 0); pcm = null; }
    }
    private static long lastGen;
    private static synchronized long nextGen() { lastGen = Math.max(lastGen + 1, System.currentTimeMillis()); return lastGen; }
    /** Tells the core synchronously (in-process JNI, <0.1ms) to abort older readings immediately. */
    private static void dropBefore(long gen) {
        try { call("POST", "/api/tts_gen", "", Native.utf8("{\"gen\":" + gen + "}")); } catch (Exception ignored) { }
    }
    private static double bufferedSeconds(Run r) {
        double secs = 0.0;
        AudioTrack t = r.track;
        if (t != null) {
            try {
                long head = Integer.toUnsignedLong(t.getPlaybackHeadPosition());
                if (r.writtenFrames > head) secs += (r.writtenFrames - head) / (double) RATE;
            } catch (RuntimeException ignored) { }
        }
        for (Future<Sound> f : r.queue) {
            if (f.isDone() && !f.isCancelled()) {
                try {
                    Sound s = f.get(0, TimeUnit.MILLISECONDS);
                    if (s != null) secs += s.duration + s.gap;
                } catch (Exception ignored) { }
            }
        }
        return secs;
    }
    private static final class Run {
        final String id = UUID.randomUUID().toString();
        final JSONObject request;
        // 2 workers match the 2 mobile ONNX slots cleanly — avoids thrashing big/mid cores to 80C+
        final ExecutorService workers = Executors.newFixedThreadPool(2);
        // Buffer up to 32 upcoming clauses/sentences ahead (~2 min of audio) so warm CPUs never starve playback.
        final ArrayBlockingQueue<Future<Sound>> queue = new ArrayBlockingQueue<>(32);
        final ConcurrentLinkedDeque<Span> spans = new ConcurrentLinkedDeque<>();
        final List<Future<Sound>> pending = Collections.synchronizedList(new ArrayList<>());
        volatile boolean cancelled, paused;
        // (finishing: a new reading has taken over, but the sentence being heard is said to its end first; previous: that reading)
        volatile boolean finishing;
        volatile Run previous;
        volatile int page, para, start, end;
        volatile double duration, gap;
        volatile long baseFrame, writtenFrames;
        // (how often, and for how long in all, the voice had to wait for its next sentence: the measure of a smooth reading)
        final java.util.concurrent.atomic.AtomicInteger stalls = new java.util.concurrent.atomic.AtomicInteger();
        final java.util.concurrent.atomic.AtomicLong stalled = new java.util.concurrent.atomic.AtomicLong();
        final String startReason;
        volatile String phase = "starting";
        volatile String waitReason;
        volatile String pauseReason = "";
        volatile int waitProgress = -1;
        volatile AudioTrack track;
        Thread feeder, player;
        // (this reading's number: the core drops a sentence of an older reading, so a new voice or place does not wait behind it)
        final long gen = nextGen();
        Run(JSONObject request) {
            this.request = request;
            page = request.optInt("page", 1); para = request.optInt("para", -1); start = request.optInt("start"); end = start;
            String custom = request.optString("reason", "").trim();
            startReason = custom.isEmpty() ? "در حال آماده‌سازی صدا…" : custom;
            waitReason = startReason;
        }
    }
    @Override public void onCreate() {
        super.onCreate(); live = this;
        NotificationManager notifications = getSystemService(NotificationManager.class);
        NotificationChannel channel = new NotificationChannel(CHANNEL, "خواندن متن", NotificationManager.IMPORTANCE_LOW);
        channel.setLockscreenVisibility(Notification.VISIBILITY_PRIVATE); channel.setShowBadge(false);
        notifications.createNotificationChannel(channel);
        manager = getSystemService(AudioManager.class);
        AudioAttributes attributes = attributes();
        focus = new AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN).setAudioAttributes(attributes)
            .setOnAudioFocusChangeListener(change -> {
                if (change == AudioManager.AUDIOFOCUS_LOSS || change == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT || change == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK)
                    setPaused(true, "به‌علت پخش صدا از برنامه‌ی دیگر مکث شد.");
            }, ui).build();
        media = new MediaSession(this, "khana-reading");
        media.setCallback(new MediaSession.Callback() {
            @Override public void onPause() { setPaused(true, "خواندن با دستور شما مکث شد."); }
            @Override public void onPlay() { setPaused(false, ""); }
            @Override public void onStop() { stopSelf(); }
        });
        media.setActive(true);
        Notification notification = notification(false);
        if (Build.VERSION.SDK_INT >= 29) startForeground(2, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK);
        else startForeground(2, notification);
        wake = getSystemService(PowerManager.class).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "khana:reading");
        noisy = new BroadcastReceiver() { @Override public void onReceive(Context c, Intent i) { setPaused(true, "به‌علت قطع شدن هدفون یا بلندگو مکث شد."); } };
        if (Build.VERSION.SDK_INT >= 33) registerReceiver(noisy, new IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY), Context.RECEIVER_NOT_EXPORTED);
        else registerReceiver(noisy, new IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY));
        ui.post(lease);
    }
    private AudioAttributes attributes() {
        AudioAttributes.Builder b = new AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA).setContentType(AudioAttributes.CONTENT_TYPE_SPEECH);
        if (Build.VERSION.SDK_INT >= 29) b.setAllowedCapturePolicy(AudioAttributes.ALLOW_CAPTURE_BY_NONE);
        return b.build();
    }
    private final Runnable lease = new Runnable() { public void run() {
        Run r = run;
        if (wake != null && r != null && !r.cancelled && !r.paused) {
            if (wake.isHeld()) wake.release();
            wake.acquire(60L * 60 * 1000);
        }
        ui.postDelayed(this, 30L * 60 * 1000);
    }};
    private Notification notification(boolean paused) {
        PendingIntent open = PendingIntent.getActivity(this, 0, new Intent(this, MainActivity.class).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_IMMUTABLE);
        PendingIntent toggle = PendingIntent.getService(this, 10, new Intent(this, ReadService.class).setAction(paused ? "resume" : "pause"), PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        PendingIntent stop = PendingIntent.getService(this, 11, new Intent(this, ReadService.class).setAction("stop"), PendingIntent.FLAG_IMMUTABLE);
        Notification publicVersion = new Notification.Builder(this, CHANNEL).setSmallIcon(android.R.drawable.ic_media_play).setContentTitle("خوانا").setContentText(paused ? "خواندن مکث شده" : "در حال خواندن").build();
        return new Notification.Builder(this, CHANNEL).setSmallIcon(android.R.drawable.ic_media_play).setContentTitle("خوانا")
            .setContentText(paused ? "خواندن مکث شده" : "در حال خواندن").setContentIntent(open).setOngoing(!paused)
            .setVisibility(Notification.VISIBILITY_PRIVATE).setPublicVersion(publicVersion)
            .addAction(new Notification.Action.Builder(Icon.createWithResource(this, paused ? android.R.drawable.ic_media_play : android.R.drawable.ic_media_pause), paused ? "ادامه" : "مکث", toggle).build())
            .addAction(new Notification.Action.Builder(Icon.createWithResource(this, android.R.drawable.ic_menu_close_clear_cancel), "توقف", stop).build())
            .setStyle(new Notification.MediaStyle().setMediaSession(media.getSessionToken()).setShowActionsInCompactView(0, 1)).build();
    }
    private boolean setPaused(boolean paused) { return setPaused(paused, paused ? "خواندن مکث شد." : ""); }
    private boolean setPaused(boolean paused, String reason) {
        Run r = run; if (r == null || r.cancelled) return false;
        if (!paused) {
            try { if (json("/api/lock_status", "").optBoolean("locked", true)) { lastStopReason = "برنامه قفل شد و خواندن متوقف گردید."; stopSelf(); return false; } }
            catch (Exception e) { stopSelf(); return false; }
        }
        if (!paused && manager.requestAudioFocus(focus) != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) return false;
        r.paused = paused;
        r.pauseReason = paused ? Objects.requireNonNullElse(reason, "") : "";
        try { if (r.track != null) { if (paused) r.track.pause(); else r.track.play(); } } catch (RuntimeException ignored) { }
        if (paused) { if (wake.isHeld()) wake.release(); manager.abandonAudioFocusRequest(focus); }
        else if (!wake.isHeld()) wake.acquire(60L * 60 * 1000);
        media.setPlaybackState(new PlaybackState.Builder().setActions(PlaybackState.ACTION_PLAY | PlaybackState.ACTION_PAUSE | PlaybackState.ACTION_STOP)
            .setState(paused ? PlaybackState.STATE_PAUSED : PlaybackState.STATE_PLAYING, 0, paused ? 0f : 1f).build());
        getSystemService(NotificationManager.class).notify(2, notification(paused));
        return true;
    }
    private static Native.Reply call(String method, String path, String query, byte[] body) throws Exception {
        Native.Reply r = new Native.Reply(Native.handle(Native.utf8(method), Native.utf8(path), Native.utf8(query), Native.token(), body));
        if (r.status != 200) { Arrays.fill(r.body, (byte) 0); throw new Exception("Reading request refused"); }
        return r;
    }
    private static JSONObject json(String path, String query) throws Exception {
        Native.Reply r = call("GET", path, query, null);
        try { return new JSONObject(Native.text(r.body)); } finally { Arrays.fill(r.body, (byte) 0); }
    }
    /** 1.29: each sentence asked for gets the next number: the core makes the lowest first when the cores are busy. */
    private static final java.util.concurrent.atomic.AtomicLong ORDER = new java.util.concurrent.atomic.AtomicLong();
    private void submit(Run r, String text, int page, int para, SpeechPieces.Piece piece, double gap, boolean quick) throws Exception {
        if (r.cancelled) throw new InterruptedException();
        final long order = ORDER.incrementAndGet();
        Future<Sound> future = r.workers.submit(() -> {
            if (r.cancelled) throw new InterruptedException();
            JSONObject body = new JSONObject().put("text", text).put("voice", r.request.getString("voice")).put("speed", r.request.getDouble("speed")).put("gen", r.gen).put("order", order);
            // Fast-path whenever starting up OR when buffered playback runway drops below 22s — prevents mid-page pauses on all devices.
            if (quick || bufferedSeconds(r) < 22.0) body.put("quick", true);
            byte[] request = Native.utf8(body.toString());
            Native.Reply reply;
            try { reply = call("POST", "/api/tts", "", request); } finally { Arrays.fill(request, (byte) 0); }
            try {
                if (r.cancelled) throw new InterruptedException();
                byte[] wav = reply.body;
                if (wav.length < 44) throw new Exception("Invalid speech output");
                if (wav.length == 44) return null;
                ByteBuffer h = ByteBuffer.wrap(wav).order(ByteOrder.LITTLE_ENDIAN);
                if (h.getInt(24) != RATE || h.getShort(22) != 1 || h.getShort(34) != 16 || (wav.length - 44) % 2 != 0) throw new Exception("Invalid speech format");
                Sound sound = new Sound(); sound.page = page; sound.para = para; sound.start = piece == null ? 0 : piece.start; sound.end = piece == null ? 0 : piece.end;
                sound.gap = gap; sound.duration = (wav.length - 44) / (2.0 * RATE);
                int silence = (int) (RATE * gap) * 2;
                sound.pcm = new byte[silence + wav.length - 44]; System.arraycopy(wav, 44, sound.pcm, silence, wav.length - 44);
                return sound;
            } finally { Arrays.fill(reply.body, (byte) 0); }
        });
        r.pending.add(future);
        while (!r.cancelled && !r.queue.offer(future, 200, TimeUnit.MILLISECONDS)) { /* wait for space in queue */ }
        if (r.cancelled) { future.cancel(true); throw new InterruptedException(); }
    }
    private static boolean isTitleLine(String p) {
        String t = p == null ? "" : p.trim();
        if (t.isEmpty() || t.length() > 70) return false;
        if (t.split("\\s+").length > 10) return false;
        char last = t.charAt(t.length() - 1);
        return Character.isLetterOrDigit(last) || "»”)]".indexOf(last) >= 0;
    }

    private static double naturalPause(boolean beginning, boolean paragraphStart, boolean title, boolean afterTitle, boolean clauseCont, String prevText, double scale) {
        if (beginning) return 0.0;
        double base;
        if (title) base = 0.58;
        else if (afterTitle) base = 0.48;
        else if (paragraphStart) base = 0.52;
        else if (clauseCont) base = 0.14;
        else {
            String prev = prevText == null ? "" : prevText.trim();
            while (!prev.isEmpty() && "»”\"'’)]".indexOf(prev.charAt(prev.length() - 1)) >= 0) prev = prev.substring(0, prev.length() - 1).trim();
            if (prev.endsWith("...") || prev.endsWith("…")) base = 0.42;
            else if (prev.endsWith("؟") || prev.endsWith("?") || prev.endsWith("!")) base = 0.38;
            else base = 0.26;
        }
        return Math.max(0.05, base * scale);
    }

    private void feed(Run r) {
        try {
            JSONObject q = r.request;
            String doc = java.net.URLEncoder.encode(q.getString("doc"), StandardCharsets.UTF_8);
            String options = "doc=" + doc + "&rev=" + q.optBoolean("rev") + "&yeh=" + q.optBoolean("yeh", true) + "&tr=" + q.optBoolean("tr");
            int first = q.getInt("page"), pages = q.getInt("pages"), firstPara = q.optInt("para"), offset = q.optInt("start");
            boolean exact = q.optBoolean("exact");
            String pauseMode = q.optString("sentencePause", "normal");
            double pauseScale = "short".equals(pauseMode) ? 0.45 : "long".equals(pauseMode) ? 1.45 : 1.0;
            boolean beginning = true;
            int pieceIndex = 0;
            boolean afterTitle = false;
            String prevText = "";
            int lastPage = first - 1;
            boolean translating = q.optBoolean("tr");
            for (int page = first; page <= pages && !r.cancelled; page++) {
                int fedPara = page == first ? firstPara : 0;
                boolean pageDone = false;
                while (!pageDone && !r.cancelled) {
                    if (beginning) {
                        r.phase = "loading_page";
                        r.waitReason = translating ? "در حال بارگذاری و ترجمه‌ی آفلاین بند اول…" : r.startReason;
                    }
                    String req = options + "&n=" + page + (translating ? "&partial=1" : "");
                    JSONObject result = json("/api/page", req);
                    if (result.optBoolean("needsPicture")) throw new Exception("Scanned page was not prepared");
                    JSONArray paras = result.getJSONArray("paragraphs");
                    pageDone = !translating || result.optBoolean("done", true);
                    int totalParas = result.optInt("total", paras.length());
                    if (translating && !pageDone && paras.length() <= fedPara) {
                        if (beginning || bufferedSeconds(r) < 1.0) {
                            r.phase = "loading_page";
                            if (totalParas > 0) {
                                int pct = Math.min(99, (paras.length() * 100) / totalParas);
                                r.waitProgress = pct;
                                r.waitReason = "در حال ترجمه‌ی آفلاین صفحه… (" + paras.length() + " از " + totalParas + " بند)";
                            } else {
                                r.waitProgress = -1;
                                r.waitReason = "در حال بارگذاری مدل و ترجمه‌ی آفلاین صفحه…";
                            }
                        }
                        Thread.sleep(120);
                        continue;
                    }
                    for (int p = fedPara; p < paras.length() && !r.cancelled; p++) {
                        boolean paragraphStart = true;
                        String text = paras.getString(p);
                        boolean title = isTitleLine(text);
                        for (SpeechPieces.Piece whole : SpeechPieces.split(text)) {
                            if (page == first && p == firstPara && whole.end <= offset) continue;
                            SpeechPieces.Piece target = (exact && page == first && p == firstPara && whole.start < offset) ? new SpeechPieces.Piece(text, offset, whole.end) : whole;
                            List<SpeechPieces.Piece> parts = SpeechPieces.clauses(text, target, beginning);
                            for (int k = 0; k < parts.size(); k++) {
                                SpeechPieces.Piece piece = parts.get(k);
                                double gap = naturalPause(beginning, paragraphStart, title, afterTitle, k > 0, prevText, pauseScale);
                                if (beginning) {
                                    r.phase = "synthesizing";
                                    r.waitProgress = -1;
                                    r.waitReason = "در حال آماده‌سازی صدا…".equals(r.startReason) ? "در حال ساختن صدای جمله‌ی اول…" : r.startReason;
                                }
                                // Fast ramp-up for the first 3 clauses builds a smooth buffer cushion immediately
                                submit(r, piece.text, page, p, piece, gap, pieceIndex < 3);
                                pieceIndex++;
                                prevText = piece.text;
                                beginning = false; paragraphStart = false; afterTitle = false;
                            }
                        }
                        afterTitle = title;
                        fedPara = p + 1;
                    }
                    if (!pageDone && !r.cancelled) Thread.sleep(120);
                }
                lastPage = page;
                if (!q.optBoolean("autoNext", true)) break;
            }
            if (!r.cancelled) {
                if (lastPage == pages && q.optBoolean("announceEnd", true)) submit(r, "پایان متن.", pages, -1, null, 0.9, false);
                FutureTask<Sound> end = new FutureTask<>(() -> { Sound s = new Sound(); s.endOfBook = true; return s; }); end.run();
                while (!r.cancelled && !r.queue.offer(end, 200, TimeUnit.MILLISECONDS)) { }
            }
        } catch (InterruptedException ignored) { } catch (Exception e) { android.util.Log.w("KhanaRead", "feed error: " + e.getClass().getSimpleName() + ": " + e.getMessage()); fail(r); }
    }
    private void play(Run r) {
        long frames = 0;
        try {
            // (the reading before this one is finishing its sentence: this one's voice starts when it ends)
            Run before = r.previous;
            if (before != null && before.player != null) before.player.join(20000);
            r.previous = null;
            int min = AudioTrack.getMinBufferSize(RATE, AudioFormat.CHANNEL_OUT_MONO, AudioFormat.ENCODING_PCM_16BIT);
            // 0.5s hardware ring buffer with low start threshold so short typed phrases (<2s) and first clauses play immediately!
            r.track = new AudioTrack.Builder().setAudioAttributes(attributes()).setAudioFormat(new AudioFormat.Builder().setSampleRate(RATE).setChannelMask(AudioFormat.CHANNEL_OUT_MONO).setEncoding(AudioFormat.ENCODING_PCM_16BIT).build())
                .setBufferSizeInBytes(Math.max(min * 2, RATE)).setTransferMode(AudioTrack.MODE_STREAM).build();
            if (Build.VERSION.SDK_INT >= 31) {
                try { r.track.setStartThresholdInFrames(Math.max(256, min / 4)); } catch (RuntimeException ignored) { }
            }
            if (r.cancelled) return;
            boolean trackStarted = false;
            boolean first = true;
            long waitFrom = SystemClock.elapsedRealtime();
            while (!r.cancelled) {
                Future<Sound> next = r.queue.poll(180, TimeUnit.MILLISECONDS);
                if (next == null) {
                    // Queue is empty (e.g. waiting for next translated paragraph or page boundary): tell the user clearly!
                    if (!r.paused && r.track != null) {
                        long headNow = Integer.toUnsignedLong(r.track.getPlaybackHeadPosition());
                        if (first || frames <= headNow + RATE / 2) {
                            r.phase = first ? "starting" : "buffering";
                            if (r.waitReason == null || r.waitReason.isEmpty()) {
                                r.waitReason = r.request.optBoolean("tr")
                                    ? "در حال ترجمه‌ی آفلاین ادامه‌ی متن…"
                                    : "در حال آماده‌سازی ادامه‌ی متن…";
                            }
                        }
                    }
                    continue;
                }
                Sound sound = null;
                boolean gotResult = false;
                while (!gotResult && !r.cancelled) {
                    try {
                        sound = next.get(180, TimeUnit.MILLISECONDS);
                        gotResult = true;
                    } catch (TimeoutException te) {
                        if (first) {
                            r.phase = "synthesizing";
                            r.waitReason = "در حال آماده‌سازی صدا…".equals(r.startReason) ? "در حال ساختن صدای جمله‌ی اول…" : r.startReason;
                        } else if (!r.paused) {
                            long headNow = Integer.toUnsignedLong(r.track.getPlaybackHeadPosition());
                            if (frames <= headNow + RATE / 2) {
                                r.phase = "buffering";
                                r.waitProgress = -1;
                                r.waitReason = "در حال ساختن صدای جمله‌ی بعدی… (پردازش آفلاین روی دستگاه)";
                            }
                        }
                    } catch (CancellationException | ExecutionException ce) {
                        if (r.cancelled) break;
                        throw ce;
                    }
                }
                r.pending.remove(next);
                if (r.cancelled) break;
                if (sound == null) continue;
                // Smooth pre-buffer cushion before starting playback:
                // - "extra" mode (for older/slower phones & tablets): pre-buffer ~4 clauses (~16s) with live progress bar
                // - "normal" mode: fast start, plus a short 650ms wait if clause #1 is very short (<2.2s)
                if (first && !sound.endOfBook) {
                    boolean extraBuf = "extra".equals(r.request.optString("smoothBuffer", "normal")) && !r.request.optBoolean("isTyped", false);
                    if (extraBuf) {
                        long deadline = SystemClock.elapsedRealtime() + 18000L;
                        while (!r.cancelled && SystemClock.elapsedRealtime() < deadline) {
                            int ready = 1;
                            double readySecs = sound.duration;
                            boolean hasEnd = false;
                            for (Future<Sound> f : r.queue) {
                                if (f.isDone() && !f.isCancelled()) {
                                    try {
                                        Sound s = f.get(0, TimeUnit.MILLISECONDS);
                                        if (s != null) {
                                            if (s.endOfBook) { hasEnd = true; break; }
                                            ready++;
                                            readySecs += s.duration + s.gap;
                                        }
                                    } catch (Exception ignored) { }
                                }
                            }
                            if (hasEnd || ready >= 4 || readySecs >= 16.0) break;
                            r.phase = "synthesizing";
                            r.waitProgress = Math.min(95, (ready * 100) / 4);
                            r.waitReason = "در حال آماده‌سازی برای خواندنِ بی‌وقفه… (" + ready + " از ۴ جمله)";
                            Thread.sleep(150);
                        }
                    } else if (sound.duration < 2.2) {
                        Future<Sound> second = r.queue.peek();
                        if (second != null && !second.isDone()) {
                            try { second.get(650, TimeUnit.MILLISECONDS); } catch (Exception ignored) { }
                        }
                    }
                }
                long waited = SystemClock.elapsedRealtime() - waitFrom;
                if (!first && !r.paused && waited > 300) { r.stalls.incrementAndGet(); r.stalled.addAndGet(waited); }
                first = false;
                r.phase = "playing";
                r.waitReason = "";
                r.waitProgress = -1;
                try {
                    if (r.cancelled) break;
                    if (Server.isLocked()) { ui.post(this::stopSelf); break; }
                    if (sound.endOfBook) {
                        while (r.paused && !r.cancelled) Thread.sleep(50);
                        if (r.cancelled) break;
                        if (frames > 0 && !r.paused) {
                            // In MODE_STREAM, stop() flushes/plays partial buffers smaller than bufferSizeInBytes!
                            try {
                                if (!trackStarted) r.track.play();
                                r.track.stop();
                            } catch (RuntimeException ignored) { }
                            long head0 = Integer.toUnsignedLong(r.track.getPlaybackHeadPosition());
                            long remainMs = Math.max(0L, (frames - head0) * 1000L / RATE);
                            long deadline = SystemClock.elapsedRealtime() + Math.min(15000L, remainMs + 1200L);
                            while (!r.cancelled && !r.paused && SystemClock.elapsedRealtime() < deadline
                                    && Integer.toUnsignedLong(r.track.getPlaybackHeadPosition()) < frames) {
                                Thread.sleep(20);
                            }
                        }
                        lastStopReason = r.request.optBoolean("autoNext", true)
                            ? "خواندن متن به پایان رسید."
                            : "خواندن این صفحه به پایان رسید (ادامه‌ی خودکار به صفحه‌ی بعد خاموش است).";
                        ui.post(() -> { if (run == r) stopSelf(); });
                        break;
                    }
                    if (!trackStarted && !r.paused) {
                        r.track.play();
                        trackStarted = true;
                    }
                    long startFrame = frames;
                    long endFrame = frames + (sound.pcm.length / 2L);
                    r.spans.addLast(new Span(startFrame, endFrame, sound.page, sound.para, sound.start, sound.end, sound.duration, sound.gap));
                    r.page = sound.page; r.para = sound.para; r.start = sound.start; r.end = sound.end;
                    r.duration = sound.duration; r.gap = sound.gap; r.baseFrame = startFrame;
                    for (int at = 0; at < sound.pcm.length && !(r.cancelled && !r.finishing);) {
                        if (r.paused) { Thread.sleep(50); continue; }
                        if (!trackStarted) { r.track.play(); trackStarted = true; }
                        int wrote = r.track.write(sound.pcm, at, Math.min(4096, sound.pcm.length - at), AudioTrack.WRITE_NON_BLOCKING);
                        if (wrote < 0) throw new Exception("Audio output stopped");
                        if (wrote == 0) Thread.sleep(8); else { at += wrote; frames += wrote / 2; r.writtenFrames = frames; }
                    }
                    if (r.finishing) break;
                    // Otherwise DO NOT drain AudioTrack — immediately queue the next sentence for 100% gapless playback!
                } finally { sound.wipe(); waitFrom = SystemClock.elapsedRealtime(); }
            }
            // If user switched voice/speed mid-sentence, let already-buffered audio finish smoothly while the new voice prepares in parallel.
            if (r.finishing && r.track != null && frames > 0) {
                long deadline = SystemClock.elapsedRealtime() + 4000L;
                while (r.finishing && SystemClock.elapsedRealtime() < deadline && Integer.toUnsignedLong(r.track.getPlaybackHeadPosition()) < frames) {
                    Thread.sleep(15);
                }
            }
        } catch (InterruptedException ignored) { } catch (Exception e) { if (!r.cancelled) { android.util.Log.w("KhanaRead", "play error: " + e.getClass().getSimpleName() + ": " + e.getMessage()); fail(r); } }
        finally { r.spans.clear(); AudioTrack track = r.track; r.track = null; if (track != null) try { track.release(); } catch (RuntimeException ignored) { } }
    }
    private void fail(Run r) { ui.post(() -> { if (run == r) { lastError = "خواندن متوقف شد. صفحه را دوباره باز کنید و تلاش کنید."; stopSelf(); } }); }
    private void cancel() { cancel(false); }
    /** Ends the reading; finish: a new one takes over, and the sentence being heard is said to its end (not when paused). */
    private Run cancel(boolean finish) {
        Run r = run; run = null; if (r == null) return null;
        boolean soft = finish && !r.paused && r.track != null && r.player != null;
        r.finishing = soft;
        r.cancelled = true;
        dropBefore(nextGen());
        if (r.feeder != null) r.feeder.interrupt(); if (r.player != null && !soft) r.player.interrupt();
        r.workers.shutdownNow();
        synchronized (r.pending) {
            for (Future<Sound> f : r.pending) {
                if (f.isDone() && !f.isCancelled()) try { f.get().wipe(); } catch (Exception ignored) { }
                f.cancel(true);
            }
            r.pending.clear();
        }
        r.queue.clear();
        if (soft) return r;
        Run prev = r.previous;
        r.previous = null;
        while (prev != null) {
            prev.finishing = false;
            prev.cancelled = true;
            if (prev.player != null) prev.player.interrupt();
            AudioTrack pt = prev.track;
            if (pt != null) try { pt.pause(); pt.flush(); } catch (RuntimeException ignored) { }
            Run nextPrev = prev.previous;
            prev.previous = null;
            prev = nextPrev;
        }
        AudioTrack track = r.track; if (track != null) try { track.pause(); track.flush(); } catch (RuntimeException ignored) { }
        if (wake != null && wake.isHeld()) wake.release();
        manager.abandonAudioFocusRequest(focus);
        return null;
    }
    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        String action = intent == null ? "stop" : intent.getAction();
        if ("pause".equals(action)) setPaused(true);
        else if ("resume".equals(action)) setPaused(false);
        else if ("start".equals(action)) {
            JSONObject q = null;
            try {
                String req = intent.getStringExtra("request");
                if (req != null) q = new JSONObject(req);
            } catch (Exception ignored) { }
            Run finishing = cancel(q != null && q.optBoolean("exact")); lastError = "";
            try {
                if (q == null) throw new Exception("Invalid reading request");
                if (q.getString("doc").length() > 128 || q.getInt("page") < 1 || q.getInt("page") > q.getInt("pages") || q.getInt("pages") > 1000000 || q.optInt("para") < 0 || q.optInt("start") < 0 || !Double.isFinite(q.getDouble("speed")) || q.getDouble("speed") < 0.5 || q.getDouble("speed") > 2.0) throw new Exception("Invalid reading request");
                Run r = new Run(q); r.previous = finishing; run = r;
                if (manager.requestAudioFocus(focus) != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) throw new Exception("Audio focus unavailable");
                if (!setPaused(false)) { cancel(false); return START_NOT_STICKY; }
                r.feeder = new Thread(() -> feed(r), "khana-read-feed"); r.player = new Thread(() -> play(r), "khana-read-play");
                r.feeder.start(); r.player.start();
            } catch (Exception e) { android.util.Log.w("KhanaRead", "start error: " + e.getClass().getSimpleName() + ": " + e.getMessage()); lastError = "خواندن شروع نشد. دوباره تلاش کنید."; stopSelf(); }
        } else stopSelf();
        return START_NOT_STICKY;
    }
    @Override public void onDestroy() {
        cancel(); if (live == this) live = null;
        ui.removeCallbacksAndMessages(null);
        if (noisy != null) unregisterReceiver(noisy);
        if (media != null) media.release();
        stopForeground(STOP_FOREGROUND_REMOVE);
        super.onDestroy();
    }
    /** 1.29: Khana closed by the owner (swiped away in the app overview): the reading stops and nothing goes on in the background.
     *  Leaving the app (Home, screen off) keeps reading, as before. */
    @Override public void onTaskRemoved(Intent root) {
        stopSelf();
        Closed.endSoon();
    }
    @Override public IBinder onBind(Intent intent) { return null; }
}
