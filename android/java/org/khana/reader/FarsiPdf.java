package org.khana.reader;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.pdf.PdfDocument;
import android.text.Layout;
import android.text.StaticLayout;
import android.text.TextDirectionHeuristics;
import android.text.TextPaint;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;

/**
 * The translated PDF on Android, as native/typeset.c lays it out on the Mac with CoreText: A4
 * pages, the app's font (Vazirmatn), each block right to left with Android's own Farsi shaping
 * (letters joined) and line breaking; paragraphs justified, 1.4 line height, the page number in
 * Persian digits at the foot. Each block is its own layout, cut across pages line by line.
 *
 * The input is the Rust core's records (src/ffi/typeset.rs): a kind byte ('T' the title, 'S' the
 * line under it, 'H' a page's heading, 'P' a paragraph) and UTF-8 text, separated by 0x1E. Only
 * the app's own text comes here (a book's translation), never a document.
 */
final class FarsiPdf {
    private static final int W = 595, H = 842;
    private static final float LEFT = 56, WIDTH = 483, TOP = 842 - 786, BOTTOM = 842 - 70;
    private static final int INK = Color.BLACK, GREY = Color.rgb(115, 115, 115);

    private FarsiPdf() {}

    static void write(Context ctx, byte[] recs, OutputStream out) throws java.io.IOException {
        Typeface font;
        try {
            font = Typeface.createFromAsset(ctx.getAssets(), "fonts/Vazirmatn.ttf");
        } catch (RuntimeException e) {
            font = Typeface.DEFAULT;
        }
        TextPaint title = paint(font, 18, INK), small = paint(font, 9, GREY), text = paint(font, 12, INK);
        PdfDocument pdf = new PdfDocument();
        Pages p = new Pages(pdf, small);
        int i = 0;
        while (i < recs.length && p.number < 20000) {
            int j = i;
            while (j < recs.length && recs[j] != 0x1E) j++;
            if (j > i + 1) {
                char kind = (char) recs[i];
                String body = new String(recs, i + 1, j - i - 1, StandardCharsets.UTF_8);
                TextPaint tp = kind == 'T' ? title : kind == 'P' ? text : small;
                Layout.Alignment align = kind == 'T' || kind == 'S' ? Layout.Alignment.ALIGN_CENTER : Layout.Alignment.ALIGN_NORMAL;
                float before = kind == 'H' ? 14 : 0, after = kind == 'S' ? 18 : kind == 'P' ? 8 : 4;
                StaticLayout l = StaticLayout.Builder.obtain(body, 0, body.length(), tp, (int) WIDTH)
                        .setAlignment(align)
                        .setTextDirection(TextDirectionHeuristics.RTL)
                        .setLineSpacing(0, 1.4f)
                        .setIncludePad(false)
                        .setBreakStrategy(Layout.BREAK_STRATEGY_HIGH_QUALITY)
                        .setHyphenationFrequency(Layout.HYPHENATION_FREQUENCY_NONE)
                        .setJustificationMode(kind == 'P' ? Layout.JUSTIFICATION_MODE_INTER_WORD : Layout.JUSTIFICATION_MODE_NONE)
                        .build();
                if (p.page != null && p.y > TOP) p.y += before;
                p.draw(l);
                p.y += after;
            }
            i = j + 1;
        }
        p.finish();
        try {
            pdf.writeTo(out);
        } finally {
            pdf.close();
        }
    }

    private static TextPaint paint(Typeface font, float size, int color) {
        TextPaint t = new TextPaint(TextPaint.ANTI_ALIAS_FLAG);
        t.setTypeface(font);
        t.setTextSize(size);
        t.setColor(color);
        t.setFontVariationSettings("'wght' 400"); // (Vazirmatn's regular weight, if it is the variable font)
        return t;
    }

    /** The pages being filled: the open page and how far down it is. */
    private static final class Pages {
        final PdfDocument pdf;
        final TextPaint numbers;
        PdfDocument.Page page;
        int number;
        float y;

        Pages(PdfDocument pdf, TextPaint numbers) {
            this.pdf = pdf;
            this.numbers = numbers;
        }

        void newPage() {
            finishPage();
            number++;
            page = pdf.startPage(new PdfDocument.PageInfo.Builder(W, H, number).create());
            y = TOP;
        }

        /** Draws the layout from y on, line by line, onto as many pages as it needs. */
        void draw(StaticLayout l) {
            int line = 0, n = l.getLineCount();
            while (line < n && number < 20000) {
                if (page == null || BOTTOM - y < 20) newPage();
                // the lines that fit on this page from `line` on
                int last = line;
                float top = l.getLineTop(line);
                while (last < n && y + (l.getLineBottom(last) - top) <= BOTTOM) last++;
                if (last == line) {
                    // (not even one line fits: the next page; a line taller than a page is cut)
                    if (y == TOP) last = line + 1;
                    else {
                        newPage();
                        continue;
                    }
                }
                float height = l.getLineBottom(last - 1) - top;
                Canvas c = page.getCanvas();
                c.save();
                c.clipRect(LEFT, y, LEFT + WIDTH, y + height);
                c.translate(LEFT, y - top);
                l.draw(c);
                c.restore();
                y += height;
                line = last;
                if (line < n) y = BOTTOM; // (the rest on the next page)
            }
        }

        void finishPage() {
            if (page == null) return;
            String num = persian(number);
            float w = numbers.measureText(num);
            page.getCanvas().drawText(num, (W - w) / 2, H - 40, numbers);
            pdf.finishPage(page);
            page = null;
        }

        void finish() {
            finishPage();
        }
    }

    static String persian(int n) {
        String s = Integer.toString(n);
        StringBuilder b = new StringBuilder();
        for (char c : s.toCharArray()) b.append((char) (0x06F0 + (c - '0')));
        return b.toString();
    }
}
