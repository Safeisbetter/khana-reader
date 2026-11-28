package org.khana.reader;

import android.app.*;
import android.content.*;
import android.content.pm.ServiceInfo;
import android.graphics.drawable.Icon;
import android.media.*;
import android.media.session.*;
import android.os.*;
import org.json.*;
import java.io.*;

/** Offline native playback of saved MP3/WAV library files with screen-off support and zero metadata/path leaks. */
public final class AudioFileService extends Service {
    private static final String CHANNEL = "audio_library";
    private static volatile AudioFileService live;
    private static volatile String lastError = "";
    private final Handler ui = new Handler(Looper.getMainLooper());
    private MediaPlayer player;
    private PowerManager.WakeLock wake;
    private AudioFocusRequest focus;
    private MediaSession media;
    private AudioManager manager;
    private BroadcastReceiver noisy;
    private String currentName = "";
    private boolean paused;
    private boolean prepared;
    private int durationMs;

    static void start(Context context, String name) {
        lastError = "";
        ReadService.stop(context);
        context.startForegroundService(new Intent(context, AudioFileService.class).setAction("start").putExtra("name", name));
    }

    static void stop(Context context) {
        AudioFileService s = live;
        if (s != null) {
            s.releasePlayer();
            context.stopService(new Intent(context, AudioFileService.class));
        }
    }

    static void stopIfPlaying(Context context, String name) {
        AudioFileService s = live;
        if (s != null && name != null && name.equals(s.currentName)) {
            stop(context);
        }
    }

    static void pause(boolean value) {
        AudioFileService s = live;
        if (s != null) s.ui.post(() -> s.setPaused(value));
    }

    static void seek(int ms) {
        AudioFileService s = live;
        if (s != null) s.ui.post(() -> s.seekTo(ms));
    }

    static String status() {
        AudioFileService s = live;
        JSONObject out = new JSONObject();
        try {
            boolean active = s != null && s.player != null;
            out.put("active", active).put("error", lastError);
            if (active) {
                int pos = 0;
                int dur = s.durationMs;
                try {
                    if (s.prepared) {
                        pos = Math.max(0, s.player.getCurrentPosition());
                        dur = Math.max(dur, s.player.getDuration());
                    }
                } catch (IllegalStateException ignored) { }
                out.put("name", s.currentName)
                    .put("paused", s.paused)
                    .put("positionMs", pos)
                    .put("durationMs", Math.max(0, dur));
            }
        } catch (Exception ignored) { }
        return out.toString();
    }

    private static File resolveLibraryFile(Context ctx, String name) throws IOException {
        if (name == null || name.isEmpty() || name.length() > 200 || name.contains("/") || name.contains("\\") || name.startsWith(".")) {
            return null;
        }
        if (!name.endsWith(".mp3") && !name.endsWith(".wav")) {
            return null;
        }
        File dir = new File(ctx.getFilesDir(), "saved").getCanonicalFile();
        File target = new File(dir, name).getCanonicalFile();
        if (!target.getPath().startsWith(dir.getPath() + File.separator) || !target.isFile()) {
            return null;
        }
        return target;
    }

    @Override
    public void onCreate() {
        super.onCreate();
        live = this;
        NotificationManager notifications = getSystemService(NotificationManager.class);
        NotificationChannel channel = new NotificationChannel(CHANNEL, "پخش فایل صوتی", NotificationManager.IMPORTANCE_LOW);
        channel.setLockscreenVisibility(Notification.VISIBILITY_PRIVATE);
        channel.setShowBadge(false);
        notifications.createNotificationChannel(channel);

        manager = getSystemService(AudioManager.class);
        AudioAttributes attrs = attributes();
        focus = new AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
            .setAudioAttributes(attrs)
            .setOnAudioFocusChangeListener(change -> {
                if (change == AudioManager.AUDIOFOCUS_LOSS || change == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT || change == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK) {
                    setPaused(true);
                }
            }, ui).build();

        media = new MediaSession(this, "khana-audio");
        media.setCallback(new MediaSession.Callback() {
            @Override public void onPause() { setPaused(true); }
            @Override public void onPlay() { setPaused(false); }
            @Override public void onStop() { stop(AudioFileService.this); }
            @Override public void onSeekTo(long pos) { seekTo((int) Math.max(0L, Math.min(Integer.MAX_VALUE, pos))); }
        });
        media.setActive(true);

        Notification n = notification(false);
        if (Build.VERSION.SDK_INT >= 29) startForeground(3, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK);
        else startForeground(3, n);

        wake = getSystemService(PowerManager.class).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "khana:audio");
        noisy = new BroadcastReceiver() {
            @Override public void onReceive(Context c, Intent i) { setPaused(true); }
        };
        if (Build.VERSION.SDK_INT >= 33) registerReceiver(noisy, new IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY), Context.RECEIVER_NOT_EXPORTED);
        else registerReceiver(noisy, new IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY));
        ui.postDelayed(guardTick, 1000L);
    }

    private AudioAttributes attributes() {
        AudioAttributes.Builder b = new AudioAttributes.Builder()
            .setUsage(AudioAttributes.USAGE_MEDIA)
            .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH);
        if (Build.VERSION.SDK_INT >= 29) b.setAllowedCapturePolicy(AudioAttributes.ALLOW_CAPTURE_BY_NONE);
        return b.build();
    }

    private final Runnable guardTick = new Runnable() {
        @Override public void run() {
            if (live != AudioFileService.this) return;
            if (Server.isLocked()) {
                releasePlayer();
                stopSelf();
                return;
            }
            if (player != null && !paused && prepared) {
                if (wake != null && !wake.isHeld()) wake.acquire(60L * 60 * 1000);
                updatePlaybackState();
            }
            ui.postDelayed(this, 1000L);
        }
    };

    private Notification notification(boolean isPaused) {
        PendingIntent open = PendingIntent.getActivity(this, 0,
            new Intent(this, MainActivity.class).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_IMMUTABLE);
        PendingIntent toggle = PendingIntent.getService(this, 20,
            new Intent(this, AudioFileService.class).setAction(isPaused ? "resume" : "pause"),
            PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        PendingIntent stopPi = PendingIntent.getService(this, 21,
            new Intent(this, AudioFileService.class).setAction("stop"), PendingIntent.FLAG_IMMUTABLE);
        String stateText = isPaused ? "پخش مکث شده" : "در حال پخش فایل صوتی";
        // Privacy: never put filename or book title in lockscreen or notification shade
        Notification publicVersion = new Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setContentTitle("کتاب‌خوان")
            .setContentText(stateText)
            .build();
        return new Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setContentTitle("کتاب‌خوان")
            .setContentText(stateText)
            .setContentIntent(open)
            .setOngoing(!isPaused)
            .setVisibility(Notification.VISIBILITY_PRIVATE)
            .setPublicVersion(publicVersion)
            .addAction(new Notification.Action.Builder(
                Icon.createWithResource(this, isPaused ? android.R.drawable.ic_media_play : android.R.drawable.ic_media_pause),
                isPaused ? "ادامه" : "مکث", toggle).build())
            .addAction(new Notification.Action.Builder(
                Icon.createWithResource(this, android.R.drawable.ic_menu_close_clear_cancel),
                "توقف", stopPi).build())
            .setStyle(new Notification.MediaStyle().setMediaSession(media.getSessionToken()).setShowActionsInCompactView(0, 1))
            .build();
    }

    private void updatePlaybackState() {
        long pos = 0;
        try {
            if (player != null && prepared) pos = Math.max(0, player.getCurrentPosition());
        } catch (IllegalStateException ignored) { }
        media.setPlaybackState(new PlaybackState.Builder()
            .setActions(PlaybackState.ACTION_PLAY | PlaybackState.ACTION_PAUSE | PlaybackState.ACTION_STOP | PlaybackState.ACTION_SEEK_TO)
            .setState(paused ? PlaybackState.STATE_PAUSED : PlaybackState.STATE_PLAYING, pos, paused ? 0f : 1f)
            .build());
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        String action = intent == null ? null : intent.getAction();
        if ("stop".equals(action)) {
            releasePlayer();
            stopSelf();
            return START_NOT_STICKY;
        }
        if ("pause".equals(action)) { setPaused(true); return START_NOT_STICKY; }
        if ("resume".equals(action)) { setPaused(false); return START_NOT_STICKY; }
        if ("start".equals(action)) {
            if (Server.isLocked()) {
                releasePlayer();
                stopSelf();
                return START_NOT_STICKY;
            }
            String name = intent.getStringExtra("name");
            startFile(name);
        }
        return START_NOT_STICKY;
    }

    private void startFile(String name) {
        releasePlayer();
        if (name == null || name.isEmpty()) {
            lastError = "فایل پیدا نشد.";
            stopSelf();
            return;
        }
        try {
            File target = resolveLibraryFile(this, name);
            if (target == null) {
                lastError = "فایل پیدا نشد.";
                stopSelf();
                return;
            }
            if (manager.requestAudioFocus(focus) != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) {
                lastError = "دسترسی به خروجی صدا ممکن نشد.";
                stopSelf();
                return;
            }
            currentName = name;
            paused = false;
            prepared = false;
            durationMs = 0;
            MediaPlayer mp = new MediaPlayer();
            mp.setAudioAttributes(attributes());
            mp.setWakeMode(this, PowerManager.PARTIAL_WAKE_LOCK);
            try (FileInputStream fis = new FileInputStream(target)) {
                mp.setDataSource(fis.getFD());
            }
            mp.setOnPreparedListener(p -> {
                if (p != player) return;
                prepared = true;
                durationMs = Math.max(0, p.getDuration());
                if (!paused && !Server.isLocked()) {
                    if (wake != null && !wake.isHeld()) wake.acquire(60L * 60 * 1000);
                    p.start();
                    getSystemService(NotificationManager.class).notify(3, notification(false));
                    updatePlaybackState();
                }
            });
            mp.setOnCompletionListener(p -> {
                if (p != player) return;
                releasePlayer();
                stopSelf();
            });
            mp.setOnErrorListener((p, what, extra) -> {
                if (p != player) return true;
                lastError = "پخش این فایل صوتی ممکن نشد.";
                releasePlayer();
                stopSelf();
                return true;
            });
            player = mp;
            mp.prepareAsync();
        } catch (Exception e) {
            lastError = "پخش این فایل صوتی ممکن نشد.";
            releasePlayer();
            stopSelf();
        }
    }

    private void setPaused(boolean pauseNow) {
        if (player == null) return;
        if (!pauseNow && Server.isLocked()) {
            releasePlayer();
            stopSelf();
            return;
        }
        if (!pauseNow && manager.requestAudioFocus(focus) != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) return;
        paused = pauseNow;
        try {
            if (prepared) {
                if (paused && player.isPlaying()) player.pause();
                else if (!paused && !player.isPlaying()) player.start();
            }
        } catch (IllegalStateException ignored) { }
        if (paused) {
            if (wake != null && wake.isHeld()) wake.release();
            manager.abandonAudioFocusRequest(focus);
        } else if (wake != null && !wake.isHeld()) {
            wake.acquire(60L * 60 * 1000);
        }
        getSystemService(NotificationManager.class).notify(3, notification(paused));
        updatePlaybackState();
    }

    private void seekTo(int ms) {
        if (player == null || !prepared || Server.isLocked()) return;
        try {
            int clamped = Math.max(0, durationMs > 0 ? Math.min(ms, durationMs) : ms);
            player.seekTo(clamped);
            updatePlaybackState();
        } catch (IllegalStateException ignored) { }
    }

    private void releasePlayer() {
        MediaPlayer mp = player;
        player = null;
        prepared = false;
        paused = false;
        currentName = "";
        durationMs = 0;
        if (mp != null) {
            try { if (mp.isPlaying()) mp.stop(); } catch (IllegalStateException ignored) { }
            try { mp.release(); } catch (RuntimeException ignored) { }
        }
        if (wake != null && wake.isHeld()) wake.release();
        if (manager != null && focus != null) manager.abandonAudioFocusRequest(focus);
    }

    @Override
    public void onDestroy() {
        ui.removeCallbacksAndMessages(null);
        releasePlayer();
        if (noisy != null) {
            try { unregisterReceiver(noisy); } catch (IllegalArgumentException ignored) { }
            noisy = null;
        }
        if (media != null) {
            try { media.setActive(false); media.release(); } catch (RuntimeException ignored) { }
            media = null;
        }
        if (live == this) live = null;
        super.onDestroy();
    }

    @Override public IBinder onBind(Intent intent) { return null; }
}
