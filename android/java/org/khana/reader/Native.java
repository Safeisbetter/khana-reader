package org.khana.reader;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.Map;

/** The app's Rust core (libkhana.so, src/ffi/jni.rs): the same request handler as the Mac window's. */
final class Native {
    static {
        System.loadLibrary("khana");
    }

    private Native() {}

    static native void init(byte[] dataDir, byte[] saveDir, byte[] webDir, byte[] modelsDir, byte[] onnxRuntime);
    static native byte[] token();
    static native byte[] handle(byte[] method, byte[] path, byte[] query, byte[] token, byte[] body);
    static native byte[] takeEvents();
    static native boolean busy();
    static native void shutdown();

    static byte[] utf8(String s) {
        return s == null ? null : s.getBytes(StandardCharsets.UTF_8);
    }

    static String text(byte[] b) {
        return b == null ? "" : new String(b, StandardCharsets.UTF_8);
    }

    /** A reply of Native.handle: status, headers (Content-Type first), body. */
    static final class Reply {
        final int status;
        final Map<String, String> headers = new LinkedHashMap<>();
        final byte[] body;

        Reply(byte[] r) {
            ByteBuffer b = ByteBuffer.wrap(r).order(ByteOrder.LITTLE_ENDIAN);
            status = b.getInt();
            int n = b.getInt();
            for (int i = 0; i < n; i++) {
                String k = string(b), v = string(b);
                headers.put(k, v);
            }
            body = new byte[b.remaining()];
            b.get(body);
            java.util.Arrays.fill(r, (byte) 0); // (the reply as it came from the core: its text is in `body` now)
        }

        private static String string(ByteBuffer b) {
            byte[] s = new byte[b.getInt()];
            b.get(s);
            return new String(s, StandardCharsets.UTF_8);
        }
    }
}
