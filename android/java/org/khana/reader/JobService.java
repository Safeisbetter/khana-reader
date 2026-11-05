package org.khana.reader;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.os.PowerManager;

/**
 * While an MP3 or a translated PDF is being made (hours for a book), the phone stays awake (not the
 * screen) and Android keeps the app running: a foreground service with a partial wake lock, as the
 * Mac app holds an IOKit power assertion. It ends itself when the Rust core has no job running, after
 * taking the finished files into Download.
 */
public final class JobService extends Service {
    private static final String CHANNEL = "jobs";
    private static final long CHECK_MS = 3000;
    private final Handler handler = new Handler(Looper.getMainLooper());
    private PowerManager.WakeLock lock;

    static void start(Context ctx) {
        ctx.startForegroundService(new Intent(ctx, JobService.class));
    }

    @Override
    public void onCreate() {
        super.onCreate();
        NotificationManager nm = getSystemService(NotificationManager.class);
        nm.createNotificationChannel(new NotificationChannel(CHANNEL, "ساختن MP3 و PDF", NotificationManager.IMPORTANCE_LOW));
        PendingIntent open = PendingIntent.getActivity(this, 0, new Intent(this, MainActivity.class).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
                PendingIntent.FLAG_IMMUTABLE);
        Notification n = new Notification.Builder(this, CHANNEL)
                .setSmallIcon(android.R.drawable.stat_sys_download)
                .setContentTitle("خوانا")
                .setContentText("در حال ساختن فایل… (گوشی بیدار می‌ماند)")
                .setContentIntent(open)
                .setOngoing(true)
                .build();
        // («special use»: a job can take longer than the 6 hours Android 15 and newer allow a data-sync
        // or media-processing service)
        if (Build.VERSION.SDK_INT >= 34) startForeground(1, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE);
        else startForeground(1, n);
        lock = getSystemService(PowerManager.class).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "khana:job");
        lock.acquire(24L * 3600 * 1000);
        handler.postDelayed(this::check, CHECK_MS);
    }

    private void check() {
        new Thread(() -> {
            Exporter.drain(this);
            if (Native.busy()) {
                handler.postDelayed(this::check, CHECK_MS);
            } else {
                Exporter.drain(this);
                handler.post(this::stopSelf);
            }
        }, "khana-job-check").start();
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        return START_NOT_STICKY;
    }

    @Override
    public void onDestroy() {
        handler.removeCallbacksAndMessages(null);
        if (lock != null && lock.isHeld()) lock.release();
        super.onDestroy();
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
