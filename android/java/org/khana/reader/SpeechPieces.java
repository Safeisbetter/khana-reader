package org.khana.reader;

import java.util.ArrayList;
import java.util.List;
import java.util.regex.Pattern;

/** Sentence and natural-clause boundaries shared by native foreground and screen-off reading. Offsets are UTF-16. */
final class SpeechPieces {
    static final class Piece {
        final String text;
        final int start, end;
        Piece(String source, int first, int last) { text = source.substring(first, last).trim(); start = first; end = last; }
    }
    private static final Pattern INITIAL = Pattern.compile("(?:^|\\s)(?:(?:Mr|Mrs|Ms|Dr|Prof|Sr|Jr|St|vs|e\\.g|i\\.e)\\.|(?:[A-Z]\\.)+)$", Pattern.CASE_INSENSITIVE);
    private static final String CLAUSE_BREAKS = "،,؛;:—–";

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
            while (end < text.length() && ".!?؟…\"'’”»)]".indexOf(text.charAt(end)) >= 0) end++;
            while (end < text.length() && Character.isWhitespace(text.charAt(end))) end++;
            add(out, text, first, end);
            first = end; i = end - 1;
        }
        if (first < text.length()) add(out, text, first, text.length());
        return out;
    }

    /** Splits long sentences (>140 chars, or >72 chars during fast startup) at natural Persian clause punctuation
     *  so synthesis stays fast and pipelines evenly without cutting mid-phrase (which causes skipped words). */
    static List<Piece> clauses(String text, Piece p, boolean isFirst) {
        return clauses(text, p, isFirst, isFirst);
    }

    static List<Piece> clauses(String text, Piece p, boolean isFirst, boolean fastStart) {
        if (isFirst) {
            List<Piece> h = head(text, p);
            if (h.size() > 1) {
                List<Piece> out = new ArrayList<>();
                out.add(h.get(0));
                out.addAll(clauses(text, h.get(1), false, fastStart));
                return out;
            }
        }
        int maxLen = fastStart ? 72 : 140;
        int minSpan = fastStart ? 30 : 48;
        int aheadMax = fastStart ? 96 : 185;
        List<Piece> out = new ArrayList<>();
        if (p.end - p.start <= maxLen) {
            out.add(p);
            return out;
        }
        int cur = p.start;
        while (p.end - cur > maxLen) {
            int limit = Math.min(p.end - 18, cur + maxLen);
            int minCut = Math.min(limit, cur + minSpan);
            int cut = -1;
            // Prefer the last natural clause punctuation within [minCut, limit]
            for (int i = limit; i >= minCut; i--) {
                if (CLAUSE_BREAKS.indexOf(text.charAt(i)) >= 0) {
                    cut = i + 1;
                    break;
                }
            }
            // If no punctuation before maxLen, look ahead slightly before forcing a word cut
            if (cut < 0) {
                int ahead = Math.min(p.end - 14, cur + aheadMax);
                for (int i = limit + 1; i <= ahead; i++) {
                    if (CLAUSE_BREAKS.indexOf(text.charAt(i)) >= 0) {
                        cut = i + 1;
                        break;
                    }
                }
            }
            if (cut < 0) {
                cut = limit;
                while (cut > minCut && !Character.isWhitespace(text.charAt(cut))) cut--;
                if (cut <= minCut) cut = limit;
            }
            while (cut < p.end && Character.isWhitespace(text.charAt(cut))) cut++;
            if (cut <= cur || cut >= p.end) break;
            if (hasAlnum(text, cur, cut)) out.add(new Piece(text, cur, cut));
            cur = cut;
        }
        if (cur < p.end && hasAlnum(text, cur, p.end)) out.add(new Piece(text, cur, p.end));
        if (out.isEmpty()) out.add(p);
        return out;
    }

    /** The first piece of a reading, cut early when long so spoken audio starts in ~1-2s after pressing play. */
    static List<Piece> head(String text, Piece p) {
        List<Piece> out = new ArrayList<>();
        if (p.end - p.start > 42) {
            int maxFirst = Math.min(p.end - 14, p.start + 58);
            for (int i = p.start + 12; i <= maxFirst; i++) {
                if (CLAUSE_BREAKS.indexOf(text.charAt(i)) < 0) continue;
                int cut = i + 1;
                while (cut < p.end && Character.isWhitespace(text.charAt(cut))) cut++;
                if (hasAlnum(text, p.start, cut) && hasAlnum(text, cut, p.end)) {
                    out.add(new Piece(text, p.start, cut));
                    out.add(new Piece(text, cut, p.end));
                    return out;
                }
            }
            if (p.end - p.start > 68) {
                int cut = Math.min(p.end - 18, p.start + 46);
                while (cut > p.start + 20 && !Character.isWhitespace(text.charAt(cut))) cut--;
                while (cut < p.end && Character.isWhitespace(text.charAt(cut))) cut++;
                if (cut > p.start + 20 && cut < p.end && hasAlnum(text, p.start, cut) && hasAlnum(text, cut, p.end)) {
                    out.add(new Piece(text, p.start, cut));
                    out.add(new Piece(text, cut, p.end));
                    return out;
                }
            }
        }
        out.add(p);
        return out;
    }

    private static boolean hasAlnum(String s, int a, int b) {
        return s.substring(a, b).codePoints().anyMatch(Character::isLetterOrDigit);
    }

    private static boolean asciiLetter(char c) { return c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z'; }

    private static void add(List<Piece> out, String text, int first, int end) {
        int start = first;
        while (end - start > 600) {
            int cut = start + 600;
            while (cut > start && !Character.isWhitespace(text.charAt(cut))) cut--;
            if (cut == start) { cut = start + 600; if (Character.isHighSurrogate(text.charAt(cut - 1))) cut--; }
            if (hasAlnum(text, start, cut)) out.add(new Piece(text, start, cut));
            start = cut;
            while (start < end && Character.isWhitespace(text.charAt(start))) start++;
        }
        if (hasAlnum(text, start, end)) out.add(new Piece(text, start, end));
    }
}
