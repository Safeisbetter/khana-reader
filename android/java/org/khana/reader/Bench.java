package org.khana.reader;

import android.util.Log;

import java.util.concurrent.atomic.AtomicInteger;

/**
 * A speed test for a developer, started over USB only (MainActivity.maybeBench: USB debugging must be on):
 * `adb shell am start -n org.khana.reader/.MainActivity --ez khana_bench true`. It makes the same twelve sentences
 * one at a time, then three at a time, as the reading page asks for them, and writes to the system log (tag KhanaBench)
 * how many seconds of speech were made per second, and where the work went. Nothing is stored or sent.
 */
final class Bench {
    private static final String[] LINES = {
        "امروز هوا آفتابی است و ما به پارک می‌رویم.", "کتابخانه‌ی شهر هر روز از ساعت نه صبح باز است.",
        "او سال‌ها در دانشگاه تهران تاریخ درس می‌داد.", "لطفاً پنجره را ببندید چون باد سردی می‌آید.",
        "قطار ساعت هفت و نیم از ایستگاه حرکت می‌کند.", "نویسنده در این فصل درباره‌ی زندگی کارگران سخن می‌گوید.",
        "آیا می‌دانید نزدیک‌ترین داروخانه کجاست؟", "مادرم برای شام خورش سبزی و برنج پخته بود.",
        "پس از باران، رنگین‌کمان بزرگی در آسمان پیدا شد.", "این پژوهش نشان می‌دهد که خواندن کتاب حافظه را بهتر می‌کند.",
        "دولت تصمیم گرفت بودجه‌ی آموزش و بهداشت را افزایش دهد.", "بچه‌ها در حیاط مدرسه با توپ بازی می‌کردند.",
    };

    private Bench() {}

    private static Native.Reply call(String method, String path, String body) {
        return new Native.Reply(Native.handle(Native.utf8(method), Native.utf8(path), Native.utf8(""), Native.token(), Native.utf8(body)));
    }

    private static double say(String text) {
        Native.Reply r = call("POST", "/api/tts", "{\"text\":\"" + text + "\",\"voice\":\"khabari\",\"speed\":1.0}");
        return r.status == 200 ? Math.max(0, r.body.length - 44) / 2.0 / 24000.0 : -1;
    }

    private static Native.Reply callq(String method, String path, String query, byte[] body) {
        return new Native.Reply(Native.handle(Native.utf8(method), Native.utf8(path), Native.utf8(query), Native.token(), body));
    }

    private static String field(String json, String key) {
        java.util.regex.Matcher m = java.util.regex.Pattern.compile("\"" + key + "\"\\s*:\\s*\"?([^\",}]*)").matcher(json);
        return m.find() ? m.group(1) : "";
    }

    /**
     * «فایل‌های من», checked on the device (`--es khana_test library`): a short text is opened, made into an MP3 as the save window
     * does, and the file must then be in the app's own folder, readable through the app, and gone after it is deleted.
     */
    static void library() {
        try {
            String text = "این یک آزمایش کوتاه است. فایل باید داخل برنامه بماند.";
            String doc = Native.text(callq("POST", "/api/open", "name=test.txt", Native.utf8(text)).body);
            String id = field(doc, "id");
            String job = Native.text(callq("POST", "/api/mp3", "", Native.utf8("{\"doc\":\"" + id + "\",\"first\":1,\"last\":1,\"voice\":\"khabari\",\"speed\":1.0,\"bitrate\":64}")).body);
            String jid = field(job, "id");
            String st = "";
            for (int k = 0; k < 240; k++) {
                Thread.sleep(500);
                st = Native.text(callq("GET", "/api/job", "id=" + jid, null).body);
                if (!field(st, "status").equals("running")) break;
            }
            String events = Native.text(Native.takeEvents()).replaceAll("\t.*/", " <app folder>/");
            String list = Native.text(callq("GET", "/api/library", "", null).body);
            String name = field(list, "name");
            Native.Reply file = callq("GET", "/api/library_file", "name=" + java.net.URLEncoder.encode(name, "UTF-8"), null);
            // (only the test's own file is ever deleted: never a file of the reader's, should the test's file be missing)
            Native.Reply del = name.startsWith("test - ") ? callq("POST", "/api/library_delete", "name=" + java.net.URLEncoder.encode(name, "UTF-8"), Native.utf8("{}"))
                    : callq("GET", "/api/library", "", null);
            String after = Native.text(callq("GET", "/api/library", "", null).body);
            Log.i("KhanaBench", "library: document opened " + !id.isEmpty() + "; job " + field(st, "status") + " " + field(st, "error") + "; core told the shell: " + events.trim()
                    + "; listed " + list.length() + " chars, first file «" + name + "»; read back " + file.status + " " + file.body.length + " bytes; deleted " + del.status
                    + "; list after: " + after);
        } catch (Throwable e) {
            Log.e("KhanaBench", "library test failed", e);
        }
    }

    /**
     * An MP3 export timed part by part (`--es khana_test mp3 --es khana_plan 5n,3n,3f`): each entry is a number of pieces made at once
     * and n (normal: every piece listened back to) or f (faster). The text is this test's own twelve sentences, three times.
     * Nothing is kept: the sound file of each run is deleted from the app's folder at once.
     */
    static void mp3(android.content.Context ctx, String plan) {
        try {
            StringBuilder sb = new StringBuilder();
            for (int k = 0; k < 3; k++) for (String l : LINES) sb.append(l).append(k == 0 && l.endsWith("است.") ? "\n\n" : " ");
            String doc = Native.text(callq("POST", "/api/open", "name=test%20-%20bench.txt", Native.utf8(sb.toString())).body);
            String id = field(doc, "id");
            say("سلام.");
            for (String step : plan.split(",")) {
                String w = step.substring(0, step.length() - 1);
                boolean fast = step.endsWith("f");
                android.system.Os.setenv("KHANA_WORKERS", w, true);
                call("POST", "/api/profile", "");
                long t0 = System.nanoTime();
                JobService.start(ctx); // (as a real export: the service that keeps it going with the screen off)
                String job = Native.text(callq("POST", "/api/mp3", "", Native.utf8("{\"doc\":\"" + id + "\",\"first\":1,\"last\":1,\"voice\":\"khabari\",\"speed\":1.0,\"bitrate\":64,\"fast\":" + fast + "}")).body);
                String jid = field(job, "id");
                String st = "";
                for (int k = 0; k < 7200; k++) {
                    Thread.sleep(500);
                    st = Native.text(callq("GET", "/api/job", "id=" + jid, null).body);
                    if (!field(st, "status").equals("running")) break;
                }
                double took = (System.nanoTime() - t0) / 1e9;
                String cpuset = new String(java.nio.file.Files.readAllBytes(java.nio.file.Paths.get("/proc/self/cpuset"))).trim();
                String prof =Native.text(call("GET", "/api/profile", null).body);
                Native.takeEvents();
                String list = Native.text(callq("GET", "/api/library", "", null).body);
                java.util.regex.Matcher m = java.util.regex.Pattern.compile("\"name\"\\s*:\\s*\"(test - bench[^\"]*)\"").matcher(list);
                while (m.find()) callq("POST", "/api/library_delete", "name=" + java.net.URLEncoder.encode(m.group(1), "UTF-8"), Native.utf8("{}"));
                Log.i("KhanaBench", "mp3 " + w + " at once, " + (fast ? "faster" : "normal") + ": " + field(st, "status") + " " + field(st, "error")
                        + "; job " + st + "; took " + Math.round(took) + " s; cores group at the end " + cpuset + "; " + prof);
            }
            android.system.Os.unsetenv("KHANA_WORKERS");
            Log.i("KhanaBench", "mp3 done");
        } catch (Throwable e) {
            Log.e("KhanaBench", "mp3 test failed", e);
        }
    }

    static void run() {
        try {
            Log.i("KhanaBench", "start; cores " + Runtime.getRuntime().availableProcessors());
            long t = System.nanoTime();
            double first = say("سلام.");
            Log.i("KhanaBench", "first sentence (the models load): " + (System.nanoTime() - t) / 1e9 + " s, speech " + first + " s");
            for (int conc : new int[] {1, 3}) {
                call("POST", "/api/profile", "");
                final AtomicInteger next = new AtomicInteger();
                final double[] speech = new double[1];
                final double[] longest = new double[1];
                Thread[] th = new Thread[conc];
                long t0 = System.nanoTime();
                for (int k = 0; k < conc; k++) {
                    th[k] = new Thread(() -> {
                        for (int i; (i = next.getAndIncrement()) < LINES.length; ) {
                            long s = System.nanoTime();
                            double d = say(LINES[i]);
                            double w = (System.nanoTime() - s) / 1e9;
                            synchronized (speech) { speech[0] += Math.max(d, 0); longest[0] = Math.max(longest[0], w); }
                        }
                    }, "khana-bench-" + k);
                    th[k].start();
                }
                for (Thread x : th) x.join();
                double took = (System.nanoTime() - t0) / 1e9;
                Log.i("KhanaBench", conc + " at a time: " + Math.round(speech[0]) + " s of speech in " + Math.round(took) + " s = "
                        + Math.round(100 * speech[0] / took) / 100.0 + " x real time; longest wait " + Math.round(10 * longest[0]) / 10.0 + " s; "
                        + Native.text(call("GET", "/api/profile", null).body));
            }
            Log.i("KhanaBench", "done");
        } catch (Throwable e) {
            Log.e("KhanaBench", "failed", e);
        }
    }
}
