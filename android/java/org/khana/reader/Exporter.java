package org.khana.reader;

import android.content.Context;
import android.net.Uri;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import android.widget.Toast;
import java.io.File;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.util.ArrayDeque;
import java.util.HashMap;
import java.util.Map;

/**
 * What the Rust core saved (Native.takeEvents) is put where the user chooses, with the system's own
 * «save as» screen (ACTION_CREATE_DOCUMENT), as GrapheneOS's PDF Viewer saves: the app has no
 * storage permission and sees no other file. An MP3 is copied; a translated PDF comes as its
 * layout's records and is made into the PDF here (FarsiPdf). The app's private copy is deleted once
 * it is saved; until then (cancelled, or the app was closed) it is offered again.
 */
public final class Exporter {
    /** 1.29 deep check: copies being written to a place the reader chose; closing the app waits for them (Closed). */
    static final java.util.concurrent.atomic.AtomicInteger WRITING = new java.util.concurrent.atomic.AtomicInteger();

    /** a file the core made, and where it was saved (null: not yet) */
    public static final class Saved {
        final File file;
        final String kind;
        Uri uri;

        Saved(File file, String kind) {
            this.file = file;
            this.kind = kind;
        }

        String mime() {
            return "mp3".equals(kind) ? "audio/mpeg" : "wav".equals(kind) ? "audio/wav" : "json".equals(kind) ? "application/json" : "application/pdf";
        }
    }

    /** the window, while it is in front: it shows the «save as» screen */
    public interface Asker {
        void ask(Saved s);
    }

    private static final Map<String, Saved> byPath = new HashMap<>();
    private static final ArrayDeque<Saved> waiting = new ArrayDeque<>();
    private static final Handler main = new Handler(Looper.getMainLooper());
    private static Asker asker;
    private static boolean asking;

    private Exporter() {}

    /** Files left from an earlier run (made, never saved): offered again. */
    static synchronized void unsaved(File dir) {
        File[] fs = dir.listFiles();
        if (fs == null) return;
        for (File f : fs) {
            String n = f.getName();
            if (byPath.containsKey(f.getPath())) continue;
            // (a sound file stays in the app, in «فایل‌های من»: only a PDF, which is made at the moment it is saved, is offered again)
            if (n.endsWith(".mp3") || n.endsWith(".wav")) byPath.put(f.getPath(), new Saved(f, n.endsWith(".mp3") ? "mp3" : "wav"));
            else if (n.endsWith(".pdf")) add(new Saved(f, "pdf"));
            else if (n.equals("khana-lexicon.json")) { //noinspection ResultOfMethodCallIgnored
                f.delete();
            }
        }
    }

    private static void add(Saved s) {
        byPath.put(s.file.getPath(), s);
        waiting.add(s);
    }

    static synchronized void drain(Context ctx) {
        if (Server.isLocked()) return;
        String ev = Native.text(Native.takeEvents());
        for (String line : ev.isEmpty() ? new String[0] : ev.split("\n")) {
            int tab = line.indexOf('\t');
            if (tab < 0) continue;
            String kind = line.substring(0, tab);
            File f = new File(line.substring(tab + 1));
            if ("mp3".equals(kind) || "wav".equals(kind)) {
                // (kept inside the app: no «save as» screen until the reader asks for a copy, «ذخیره در گوشی…»)
                byPath.put(f.getPath(), new Saved(f, kind));
            } else if ("pdf".equals(kind)) {
                add(new Saved(f, kind));
            } else if ("reveal".equals(kind)) {
                Saved s = byPath.get(f.getPath());
                if (s == null && f.isFile() && (f.getName().endsWith(".mp3") || f.getName().endsWith(".wav") || f.getName().endsWith(".json"))) {
                    String k = f.getName().endsWith(".mp3") ? "mp3" : f.getName().endsWith(".wav") ? "wav" : "json";
                    s = new Saved(f, k);
                    byPath.put(f.getPath(), s);
                }
                if (s == null) continue;
                if (!"pdf".equals(s.kind)) {
                    // (a sound file of the library: a copy may be saved as often as the reader likes)
                    if (!waiting.contains(s)) waiting.addFirst(s);
                } else if (s.uri != null) {
                    // (saved already: not handed to another app; it is in the place chosen, for the Files app)
                    main.post(() -> Toast.makeText(ctx.getApplicationContext(), "این فایل ذخیره شده است. آن را در برنامه‌ی «فایل‌ها» باز کنید.", Toast.LENGTH_LONG).show());
                }
                else if (!waiting.contains(s)) waiting.addFirst(s); // (not saved yet: «save as» again)
            }
        }
        askNext();
    }

    static synchronized void setAsker(Asker a) {
        asker = a;
        if (a == null) asking = false;
        askNext();
    }

    static synchronized void onUnlocked() {
        askNext();
    }

    static synchronized void clearAll(Context ctx) {
        waiting.clear();
        byPath.clear();
        asking = false;
        try {
            ctx.stopService(new android.content.Intent(ctx, JobService.class));
        } catch (Throwable ignored) { }
        File dir = new File(ctx.getFilesDir(), "saved");
        File[] fs = dir.listFiles();
        if (fs != null) {
            for (File f : fs) {
                if (f.isFile()) {
                    //noinspection ResultOfMethodCallIgnored
                    f.delete();
                }
            }
        }
    }

    private static void askNext() {
        if (asker == null || asking || waiting.isEmpty() || Server.isLocked()) return;
        asking = true;
        Saved s = waiting.peek();
        Asker a = asker;
        main.post(() -> a.ask(s));
    }

    /** The «save as» screen's answer: the place chosen, or null (cancelled: kept, offered again later). */
    static void chosen(Context ctx, Saved s, Uri u) {
        synchronized (Exporter.class) {
            waiting.remove(s);
            asking = false;
        }
        if (u == null) {
            if ("json".equals(s.kind)) { //noinspection ResultOfMethodCallIgnored
                s.file.delete();
            }
            Toast.makeText(ctx, "pdf".equals(s.kind) || "json".equals(s.kind) ? "فایل ذخیره نشد. با «ذخیره / باز کردن» دوباره امتحان کنید." : "رونوشت ذخیره نشد. فایل در «فایل‌های من» می‌ماند.", Toast.LENGTH_LONG).show();
            return;
        }
        Context app = ctx.getApplicationContext();
        new Thread(() -> {
            String msg;
            WRITING.incrementAndGet();
            try {
                write(app, s, u);
                if (s.kind.equals("pdf") || s.kind.equals("json")) {
                    synchronized (Exporter.class) {
                        s.uri = u;
                    }
                    //noinspection ResultOfMethodCallIgnored
                    s.file.delete();
                    msg = "ذخیره شد.";
                } else {
                    // (the app keeps its own copy, in «فایل‌های من», until the reader deletes it there)
                    msg = "یک رونوشت ذخیره شد. فایل در «فایل‌های من» هم می‌ماند.";
                }
            } catch (Exception e) {
                Log.w("Khana", "saving failed: " + e.getClass().getSimpleName()); // (not the message: it can hold the file's name)
                msg = "ذخیره نشد. دوباره امتحان کنید.";
            }
            WRITING.decrementAndGet();
            final String m = msg;
            main.post(() -> {
                Toast.makeText(app, m, Toast.LENGTH_LONG).show();
                synchronized (Exporter.class) {
                    askNext();
                }
            });
        }, "khana-save").start();
    }

    private static void write(Context ctx, Saved s, Uri u) throws Exception {
        if (Server.isLocked()) throw new IllegalStateException("App Lock is active");
        try (OutputStream out = ctx.getContentResolver().openOutputStream(u, "wt")) {
            if (out == null) throw new IllegalStateException("the place cannot be written");
            if (s.kind.equals("mp3") || s.kind.equals("wav") || s.kind.equals("json")) {
                try (InputStream in = Files.newInputStream(s.file.toPath())) {
                    byte[] buf = new byte[1 << 16];
                    int n;
                    while ((n = in.read(buf)) > 0) out.write(buf, 0, n);
                }
            } else {
                byte[] recs = Files.readAllBytes(s.file.toPath());
                try {
                    FarsiPdf.write(ctx, recs, out);
                } finally {
                    java.util.Arrays.fill(recs, (byte) 0);
                }
            }
        }
    }
}
