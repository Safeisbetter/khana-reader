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
        new Handler(Looper.getMainLooper()).postDelayed(() -> android.os.Process.killProcess(android.os.Process.myPid()), 400);
    }
}
