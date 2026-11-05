package org.khana.reader;

import android.content.Context;
import android.net.Uri;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import android.widget.Toast;
import java.io.File;
import java.io.FileInputStream;
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
final class Exporter {
    /** a file the core made, and where it was saved (null: not yet) */
    static final class Saved {
        final File file;
        final String kind;
        Uri uri;

        Saved(File file, String kind) {
            this.file = file;
            this.kind = kind;
        }

        String mime() {
            return kind.equals("mp3") ? "audio/mpeg" : "application/pdf";
        }
    }

    /** the window, while it is in front: it shows the «save as» screen */
    interface Asker {
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
            if (n.endsWith(".mp3") || n.endsWith(".pdf")) add(new Saved(f, n.endsWith(".mp3") ? "mp3" : "pdf"));
        }
    }

    private static void add(Saved s) {
        byPath.put(s.file.getPath(), s);
        waiting.add(s);
    }

    static synchronized void drain(Context ctx) {
        String ev = Native.text(Native.takeEvents());
        for (String line : ev.isEmpty() ? new String[0] : ev.split("\n")) {
            int tab = line.indexOf('\t');
            if (tab < 0) continue;
            String kind = line.substring(0, tab);
            File f = new File(line.substring(tab + 1));
            if (kind.equals("mp3") || kind.equals("pdf")) {
                add(new Saved(f, kind));
            } else if (kind.equals("reveal")) {
                Saved s = byPath.get(f.getPath());
                if (s == null) continue;
                if (s.uri != null) {
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

    private static void askNext() {
        if (asker == null || asking || waiting.isEmpty()) return;
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
            Toast.makeText(ctx, "فایل ذخیره نشد. با «ذخیره / باز کردن» دوباره می‌توانید.", Toast.LENGTH_LONG).show();
            return;
        }
        Context app = ctx.getApplicationContext();
        new Thread(() -> {
            String msg;
            try {
                write(app, s, u);
                synchronized (Exporter.class) {
                    s.uri = u;
                }
                //noinspection ResultOfMethodCallIgnored
                s.file.delete();
                msg = "ذخیره شد.";
            } catch (Exception e) {
                Log.w("Khana", "saving: " + e);
                msg = "ذخیره نشد: " + e.getMessage();
            }
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
        try (OutputStream out = ctx.getContentResolver().openOutputStream(u, "wt")) {
            if (out == null) throw new IllegalStateException("the place cannot be written");
            if (s.kind.equals("mp3")) {
                try (InputStream in = new FileInputStream(s.file)) {
                    byte[] buf = new byte[1 << 16];
                    for (int n; (n = in.read(buf)) > 0; ) out.write(buf, 0, n);
                }
            } else {
                FarsiPdf.write(ctx, Files.readAllBytes(s.file.toPath()), out);
            }
        }
    }
}
