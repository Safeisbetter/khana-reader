package org.khana.reader;

import android.app.*;
import android.content.*;
import android.content.pm.ServiceInfo;
import android.media.*;
import android.media.session.*;
import android.os.*;
import org.json.*;
import java.nio.*;
import java.util.*;
import java.util.concurrent.*;

/** Offline native playback: the same player owns reading with the display on or off. Never exported. */
public final class ReadService extends Service {
    private static final String CHANNEL = "reading";
    private static final int RATE = 24000;
    private static volatile ReadService live;
    private static volatile String lastError = "";
    private final Handler ui = new Handler(Looper.getMainLooper());
    private volatile Run run;
    private PowerManager.WakeLock wake;
    private AudioFocusRequest focus;
    private MediaSession media;
    private AudioManager manager;
    private BroadcastReceiver noisy;

    static void start(Context context, JSONObject request) {
        lastError = "";
        context.startForegroundService(new Intent(context, ReadService.class).setAction("start").putExtra("request", request.toString()));
    }
    static void stop(Context context) { context.stopService(new Intent(context, ReadService.class)); }
    static boolean reading() { ReadService s = live; return s != null && s.run != null && !s.run.cancelled; }
    static void pause(boolean value) { ReadService s = live; if (s != null) s.ui.post(() -> s.setPaused(value)); }
    static String status() {
        ReadService s = live;
        Run r = s == null ? null : s.run;
        JSONObject out = new JSONObject();
        try {
            out.put("active", r != null && !r.cancelled).put("error", lastError);
            if (r != null && !r.cancelled) {
                out.put("session", r.id).put("doc", r.request.getString("doc")).put("paused", r.paused)
                    .put("page", r.page).put("para", r.para).put("start", r.start).put("end", r.end)
                    .put("duration", r.duration);
                long head = r.track == null ? r.baseFrame : Integer.toUnsignedLong(r.track.getPlaybackHeadPosition());
                out.put("elapsed", Math.max(0, (head - r.baseFrame) / (double) RATE - r.gap));
            }
        } catch (Exception ignored) { }
        return out.toString();
    }
    private static final class Sound {
        byte[] pcm;
        int page, para, start, end;
        double gap, duration;
        boolean endOfBook;
        void wipe() { if (pcm != null) Arrays.fill(pcm, (byte) 0); pcm = null; }
    }
    private static final class Run {
        final String id = UUID.randomUUID().toString();
        final JSONObject request;
        final ExecutorService workers = Executors.newFixedThreadPool(3);
        final ArrayBlockingQueue<Future<Sound>> queue = new ArrayBlockingQueue<>(4);
        final List<Future<Sound>> pending = Collections.synchronizedList(new ArrayList<>());
        volatile boolean cancelled, paused;
        volatile int page, para = -1, start, end;
        volatile double duration, gap;
        volatile long baseFrame;
        volatile AudioTrack track;
        Thread feeder, player;
        Run(JSONObject request) { this.request = request; page = request.optInt("page", 1); }
    }
    @Override public void onCreate() {
        super.onCreate(); live = this;
        NotificationManager notifications = getSystemService(NotificationManager.class);
        NotificationChannel channel = new NotificationChannel(CHANNEL, "خواندن کتاب", NotificationManager.IMPORTANCE_LOW);
        channel.setLockscreenVisibility(Notification.VISIBILITY_PRIVATE); channel.setShowBadge(false);
        notifications.createNotificationChannel(channel);
        manager = getSystemService(AudioManager.class);
        AudioAttributes attributes = attributes();
        focus = new AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN).setAudioAttributes(attributes)
            .setOnAudioFocusChangeListener(change -> {
                if (change == AudioManager.AUDIOFOCUS_LOSS || change == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT || change == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK) setPaused(true);
            }, ui).build();
        media = new MediaSession(this, "khana-reading");
        media.setCallback(new MediaSession.Callback() {
            @Override public void onPause() { setPaused(true); }
            @Override public void onPlay() { setPaused(false); }
            @Override public void onStop() { stopSelf(); }
        });
        media.setActive(true);
        Notification notification = notification(false);
        if (Build.VERSION.SDK_INT >= 29) startForeground(2, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK);
        else startForeground(2, notification);
        wake = getSystemService(PowerManager.class).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "khana:reading");
        noisy = new BroadcastReceiver() { @Override public void onReceive(Context c, Intent i) { setPaused(true); } };
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
            .addAction(new Notification.Action.Builder(paused ? android.R.drawable.ic_media_play : android.R.drawable.ic_media_pause, paused ? "ادامه" : "مکث", toggle).build())
            .addAction(new Notification.Action.Builder(android.R.drawable.ic_menu_close_clear_cancel, "توقف", stop).build())
            .setStyle(new Notification.MediaStyle().setMediaSession(media.getSessionToken()).setShowActionsInCompactView(0, 1)).build();
    }
    private void setPaused(boolean paused) {
        Run r = run; if (r == null || r.cancelled) return;
        if (!paused) {
            try { if (json("/api/lock_status", "").optBoolean("locked", true)) { stopSelf(); return; } }
            catch (Exception e) { stopSelf(); return; }
        }
        if (!paused && manager.requestAudioFocus(focus) != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) return;
        r.paused = paused;
        try { if (r.track != null) { if (paused) r.track.pause(); else r.track.play(); } } catch (RuntimeException ignored) { }
        if (paused) { if (wake.isHeld()) wake.release(); manager.abandonAudioFocusRequest(focus); }
        else if (!wake.isHeld()) wake.acquire(60L * 60 * 1000);
        media.setPlaybackState(new PlaybackState.Builder().setActions(PlaybackState.ACTION_PLAY | PlaybackState.ACTION_PAUSE | PlaybackState.ACTION_STOP)
            .setState(paused ? PlaybackState.STATE_PAUSED : PlaybackState.STATE_PLAYING, 0, paused ? 0f : 1f).build());
        getSystemService(NotificationManager.class).notify(2, notification(paused));
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
    private void submit(Run r, String text, int page, int para, SpeechPieces.Piece piece, double gap) throws Exception {
        if (r.cancelled) throw new InterruptedException();
        Future<Sound> future = r.workers.submit(() -> {
            if (r.cancelled) throw new InterruptedException();
            JSONObject body = new JSONObject().put("text", text).put("voice", r.request.getString("voice")).put("speed", r.request.getDouble("speed"));
            byte[] request = Native.utf8(body.toString());
            Native.Reply reply;
            try { reply = call("POST", "/api/tts", "", request); } finally { Arrays.fill(request, (byte) 0); }
            try {
                if (r.cancelled) throw new InterruptedException();
                byte[] wav = reply.body;
                if (wav.length < 44) throw new Exception("Invalid speech output");
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
        while (!r.cancelled && !r.queue.offer(future, 200, TimeUnit.MILLISECONDS)) { }
        if (r.cancelled) { future.cancel(true); throw new InterruptedException(); }
    }
    private void feed(Run r) {
        try {
            JSONObject q = r.request;
            String doc = java.net.URLEncoder.encode(q.getString("doc"), "UTF-8");
            String options = "doc=" + doc + "&rev=" + q.optBoolean("rev") + "&yeh=" + q.optBoolean("yeh", true) + "&tr=" + q.optBoolean("tr");
            int first = q.getInt("page"), pages = q.getInt("pages"), firstPara = q.optInt("para"), offset = q.optInt("start");
            boolean beginning = true;
            int lastPage = first - 1;
            for (int page = first; page <= pages && !r.cancelled; page++) {
                JSONObject result = json("/api/page", options + "&n=" + page);
                if (result.optBoolean("needsPicture")) throw new Exception("Scanned page was not prepared");
                JSONArray paras = result.getJSONArray("paragraphs");
                for (int p = page == first ? firstPara : 0; p < paras.length() && !r.cancelled; p++) {
                    boolean paragraphStart = true;
                    for (SpeechPieces.Piece piece : SpeechPieces.split(paras.getString(p))) {
                        if (page == first && p == firstPara && piece.end <= offset) continue;
                        submit(r, piece.text, page, p, piece, beginning ? 0 : paragraphStart ? 0.9 : 0.45);
                        beginning = false; paragraphStart = false;
                    }
                }
                lastPage = page;
                if (!q.optBoolean("autoNext", true)) break;
            }
            if (!r.cancelled) {
                if (lastPage == pages && q.optBoolean("announceEnd", true)) submit(r, "پایان متن.", pages, -1, null, 1.2);
                FutureTask<Sound> end = new FutureTask<>(() -> { Sound s = new Sound(); s.endOfBook = true; return s; }); end.run();
                while (!r.cancelled && !r.queue.offer(end, 200, TimeUnit.MILLISECONDS)) { }
            }
        } catch (InterruptedException ignored) { } catch (Exception e) { fail(r); }
    }
    private void play(Run r) {
        long frames = 0;
        try {
            int min = AudioTrack.getMinBufferSize(RATE, AudioFormat.CHANNEL_OUT_MONO, AudioFormat.ENCODING_PCM_16BIT);
            r.track = new AudioTrack.Builder().setAudioAttributes(attributes()).setAudioFormat(new AudioFormat.Builder().setSampleRate(RATE).setChannelMask(AudioFormat.CHANNEL_OUT_MONO).setEncoding(AudioFormat.ENCODING_PCM_16BIT).build())
                .setBufferSizeInBytes(Math.max(min * 2, RATE)).setTransferMode(AudioTrack.MODE_STREAM).build();
            if (r.cancelled) return;
            r.track.play();
            while (!r.cancelled) {
                Future<Sound> next = r.queue.poll(200, TimeUnit.MILLISECONDS); if (next == null) continue;
                Sound sound = next.get(); r.pending.remove(next);
                try {
                    if (r.cancelled) break;
                    if (sound.endOfBook) { ui.post(() -> { if (run == r) stopSelf(); }); break; }
                    r.page = sound.page; r.para = sound.para; r.start = sound.start; r.end = sound.end;
                    r.duration = sound.duration; r.gap = sound.gap; r.baseFrame = frames;
                    for (int at = 0; at < sound.pcm.length && !r.cancelled;) {
                        if (r.paused) { Thread.sleep(50); continue; }
                        int wrote = r.track.write(sound.pcm, at, Math.min(4096, sound.pcm.length - at), AudioTrack.WRITE_NON_BLOCKING);
                        if (wrote < 0) throw new Exception("Audio output stopped");
                        if (wrote == 0) Thread.sleep(10); else { at += wrote; frames += wrote / 2; }
                    }
                    while (!r.cancelled && Integer.toUnsignedLong(r.track.getPlaybackHeadPosition()) < frames) Thread.sleep(20);
                } finally { sound.wipe(); }
            }
        } catch (InterruptedException ignored) { } catch (Exception e) { if (!r.cancelled) fail(r); }
        finally { AudioTrack track = r.track; r.track = null; if (track != null) try { track.release(); } catch (RuntimeException ignored) { } }
    }
    private void fail(Run r) { ui.post(() -> { if (run == r) { lastError = "خواندن متوقف شد. صفحه را دوباره باز کنید و تلاش کنید."; stopSelf(); } }); }
    private void cancel() {
        Run r = run; run = null; if (r == null) return;
        r.cancelled = true;
        if (r.feeder != null) r.feeder.interrupt(); if (r.player != null) r.player.interrupt();
        r.workers.shutdownNow();
        synchronized (r.pending) {
            for (Future<Sound> f : r.pending) {
                if (f.isDone() && !f.isCancelled()) try { f.get().wipe(); } catch (Exception ignored) { }
                f.cancel(true);
            }
            r.pending.clear();
        }
        r.queue.clear();
        AudioTrack track = r.track; if (track != null) try { track.pause(); track.flush(); } catch (RuntimeException ignored) { }
        if (wake != null && wake.isHeld()) wake.release();
        manager.abandonAudioFocusRequest(focus);
    }
    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        String action = intent == null ? "stop" : intent.getAction();
        if ("pause".equals(action)) setPaused(true);
        else if ("resume".equals(action)) setPaused(false);
        else if ("start".equals(action)) {
            cancel(); lastError = "";
            try {
                JSONObject q = new JSONObject(intent.getStringExtra("request"));
                if (q.getString("doc").length() > 128 || q.getInt("page") < 1 || q.getInt("page") > q.getInt("pages") || q.getInt("pages") > 1000000 || q.optInt("para") < 0 || q.optInt("start") < 0 || !Double.isFinite(q.getDouble("speed")) || q.getDouble("speed") < 0.5 || q.getDouble("speed") > 2.0) throw new Exception("Invalid reading request");
                Run r = new Run(q); run = r;
                if (manager.requestAudioFocus(focus) != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) throw new Exception("Audio focus unavailable");
                setPaused(false);
                r.feeder = new Thread(() -> feed(r), "khana-read-feed"); r.player = new Thread(() -> play(r), "khana-read-play");
                r.feeder.start(); r.player.start();
            } catch (Exception e) { lastError = "خواندن شروع نشد. دوباره تلاش کنید."; stopSelf(); }
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
    @Override public IBinder onBind(Intent intent) { return null; }
}
