package org.khana.reader;

import android.os.Handler;
import android.os.Looper;

/** 1.29: when the owner closes Khana (swipes it away in the app overview) while it reads aloud or makes a file, its process ends:
 *  no reading, no file-making and no work of the core go on in the background, the wake lock is let go, and the open book's
 *  text leaves memory with the process. The services' own clean-up runs first. */
final class Closed {
    private Closed() { }
    private static boolean ending;
    static synchronized void endSoon() {
        if (ending) return;
        ending = true;
        // (1.29 deep check: a copy being written to a place the reader chose is finished first, up to a minute, so it is not left
        // cut short)
        Handler h = new Handler(Looper.getMainLooper());
        Runnable[] end = new Runnable[1];
        long until = android.os.SystemClock.uptimeMillis() + 60_000;
        end[0] = () -> {
            if (Exporter.WRITING.get() > 0 && android.os.SystemClock.uptimeMillis() < until) h.postDelayed(end[0], 200);
            else android.os.Process.killProcess(android.os.Process.myPid());
        };
        h.postDelayed(end[0], 400);
    }
}
