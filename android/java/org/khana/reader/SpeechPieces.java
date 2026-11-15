package org.khana.reader;

import java.util.ArrayList;
import java.util.List;
import java.util.regex.Pattern;

/** Sentence boundaries shared by native foreground and screen-off reading. Offsets are UTF-16, as in the page. */
final class SpeechPieces {
    static final class Piece {
        final String text;
        final int start, end;
        Piece(String source, int first, int last) { text = source.substring(first, last).trim(); start = first; end = last; }
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
    private static boolean asciiLetter(char c) { return c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z'; }
    private static void add(List<Piece> out, String text, int first, int end) {
        // Bound generation latency on an unusually long sentence without dropping letters.
        while (end - first > 600) {
            int cut = first + 600;
            while (cut > first && !Character.isWhitespace(text.charAt(cut))) cut--;
            if (cut == first) { cut = first + 600; if (Character.isHighSurrogate(text.charAt(cut - 1))) cut--; }
            out.add(new Piece(text, first, cut)); first = cut;
            while (first < end && Character.isWhitespace(text.charAt(first))) first++;
        }
        if (text.substring(first, end).codePoints().anyMatch(Character::isLetterOrDigit)) out.add(new Piece(text, first, end));
    }
}
