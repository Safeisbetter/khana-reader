package org.khana.reader;

import java.util.ArrayList;
import java.util.List;
import java.util.regex.Pattern;

/** Sentence boundaries shared by native foreground and screen-off reading. Offsets are UTF-16, as in the page. */
final class SpeechPieces {
    // Keep Android's streaming TTS requests short enough that one slow generation cannot
    // leave the player silent for several seconds after a short comma-delimited lead-in.
    private static final int MAX_PIECE = 80;
    static final class Piece {
        final String text;
        final int start, end;
        final boolean continuation;
        Piece(String source, int first, int last) { this(source, first, last, false); }
        Piece(String source, int first, int last, boolean continuation) {
            text = source.substring(first, last).trim(); start = first; end = last; this.continuation = continuation;
        }
    }
    private static final Pattern INITIAL = Pattern.compile("(?:^|\\s)(?:(?:Mr|Mrs|Ms|Dr|Prof|Sr|Jr|St|vs|e\\.g|i\\.e)\\.|(?:[A-Z]\\.)+)$", Pattern.CASE_INSENSITIVE);
    static List<Piece> split(String text) {
        List<Piece> out = new ArrayList<>();
        int first = 0;
        for (int i = 0; i < text.length(); i++) {
            char c = text.charAt(i);
            if (".!?؟…".indexOf(c) < 0) continue;
            if (c == '.') {
                char before = i > 0 ? text.charAt(i - 1) : ' ', after = i + 1 < text.length() ? text.charAt(i + 1) : ' ';
                if (Character.isDigit(before) && Character.isDigit(after)) continue;
                if (asciiLetter(before) && asciiLetter(after)) continue;
                if (INITIAL.matcher(text.substring(first, i + 1)).find()) continue;
            }
            int end = i + 1;
            while (end < text.length() && ".!?؟…\"'’”») ]".replace(" ", "").indexOf(text.charAt(end)) >= 0) end++;
            while (end < text.length() && Character.isWhitespace(text.charAt(end))) end++;
            add(out, text, first, end);
            first = end; i = end - 1;
        }
        if (first < text.length()) add(out, text, first, text.length());
        return out;
    }
    /** Keep the first clip after a jump or page turn short enough to prepare while the old page ends. */
    static List<Piece> head(String text, Piece p) {
        List<Piece> out = new ArrayList<>();
        if (p.end - p.start > 60) {
            int minCut = p.start + 30;
            int targetCut = Math.min(p.end - 15, p.start + 50);
            for (int i = minCut; i < targetCut; i++) {
                if ("،,؛;:".indexOf(text.charAt(i)) < 0) continue;
                int cut = i + 1;
                while (cut < p.end && Character.isWhitespace(text.charAt(cut))) cut++;
                if (cut - p.start <= 55) {
                    out.add(new Piece(text, p.start, cut));
                    out.add(new Piece(text, cut, p.end, true));
                    return out;
                }
            }
            int cut = targetCut;
            while (cut > minCut && !Character.isWhitespace(text.charAt(cut - 1))) cut--;
            if (cut <= minCut) {
                cut = targetCut;
                while (cut < p.end - 15 && !Character.isWhitespace(text.charAt(cut))) cut++;
            }
            if (cut > minCut && cut < p.end) {
                out.add(new Piece(text, p.start, cut));
                out.add(new Piece(text, cut, p.end, true));
                return out;
            }
        }
        out.add(p);
        return out;
    }
    private static boolean asciiLetter(char c) { return c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z'; }
    private static void add(List<Piece> out, String text, int first, int end) {
        // Keep one TTS request short enough to prepare while the previous piece is heard.
        // Prefer clause punctuation; otherwise use a word boundary, preserving every source offset.
        int initial = first;
        while (end - first > MAX_PIECE) {
            int limit = first + MAX_PIECE;
            int cut = -1;
            for (int i = first + MAX_PIECE / 2; i < limit; i++) {
                if ("،,؛;:".indexOf(text.charAt(i)) >= 0) cut = i + 1;
            }
            if (cut < 0) {
                cut = limit;
                while (cut > first && !Character.isWhitespace(text.charAt(cut - 1))) cut--;
                if (cut == first) { cut = limit; if (Character.isHighSurrogate(text.charAt(cut - 1))) cut--; }
            }
            out.add(new Piece(text, first, cut, first > initial)); first = cut;
            while (first < end && Character.isWhitespace(text.charAt(first))) first++;
        }
        if (text.substring(first, end).codePoints().anyMatch(Character::isLetterOrDigit)) out.add(new Piece(text, first, end, first > initial));
    }
}
