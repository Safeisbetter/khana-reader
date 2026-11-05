package org.khana.reader;

import android.content.Context;
import android.content.res.AssetManager;
import java.io.BufferedReader;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;

/**
 * The reader page and the models are in the APK (assets/app, listed in assets/app.files); the
 * speech, recognition and translation libraries read them as files, so they are copied once into
 * the app's private storage (again only after the app is updated). A copy is made beside the old
 * one and put in its place when complete, so an interrupted start leaves nothing half made.
 */
final class Assets {
    private Assets() {}

    static void unpack(Context ctx, File dest, String version) throws IOException {
        File mark = new File(dest, ".version");
        if (mark.isFile() && version.equals(new String(Files.readAllBytes(mark.toPath()), StandardCharsets.UTF_8))) return;
        File tmp = new File(dest.getPath() + ".new");
        delete(tmp);
        AssetManager am = ctx.getAssets();
        byte[] buf = new byte[1 << 20];
        try (BufferedReader list = new BufferedReader(new InputStreamReader(am.open("app.files"), StandardCharsets.UTF_8))) {
            for (String line; (line = list.readLine()) != null; ) {
                if (line.isEmpty()) continue;
                int tab = line.indexOf('\t');
                String rel = tab < 0 ? line : line.substring(0, tab);
                if (rel.startsWith("/") || rel.contains("..")) throw new IOException("bad asset name " + rel);
                File out = new File(tmp, rel);
                //noinspection ResultOfMethodCallIgnored
                out.getParentFile().mkdirs();
                long want = tab < 0 ? -1 : Long.parseLong(line.substring(tab + 1));
                long got = 0;
                try (InputStream in = am.open("app/" + rel, AssetManager.ACCESS_STREAMING); OutputStream o = new FileOutputStream(out)) {
                    for (int n; (n = in.read(buf)) > 0; ) {
                        o.write(buf, 0, n);
                        got += n;
                    }
                }
                if (want >= 0 && got != want) throw new IOException(rel + ": " + got + " of " + want + " bytes");
            }
        }
        try (OutputStream o = new FileOutputStream(new File(tmp, ".version"))) {
            o.write(version.getBytes(StandardCharsets.UTF_8));
        }
        delete(dest);
        if (!tmp.renameTo(dest)) throw new IOException("cannot put the app's files in place");
    }

    private static void delete(File f) {
        File[] kids = f.listFiles();
        if (kids != null) for (File k : kids) delete(k);
        //noinspection ResultOfMethodCallIgnored
        f.delete();
    }
}
