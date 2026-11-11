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
