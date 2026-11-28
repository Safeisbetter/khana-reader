package org.khana.reader;

import android.content.Context;
import android.media.MediaCodec;
import android.media.MediaExtractor;
import android.media.MediaFormat;
import android.util.Log;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;

/**
 * A voice sample in any format the phone can play (M4A, MP3, AAC, FLAC…) -> 16-bit PCM WAV at its
 * own rate and channels, for «صدای من» (the Mac does this with afconvert; the Rust core then makes it
 * mono 24 kHz). Decoded by Android's own MediaCodec, from a private temporary file.
 */
final class AudioDecode {
    private static final long MAX_SECONDS = 120;
    private static final int MAX_PCM_BYTES = 24 * 1024 * 1024;
    private static final long DECODE_TIMEOUT_MS = 15_000;
    private static final int MAX_ITERATIONS = 50_000;

    private AudioDecode() {}

    private static final class WipingBytes extends ByteArrayOutputStream {
        void wipe() {
            java.util.Arrays.fill(buf, (byte) 0);
            reset();
        }
    }

    private static void wipeAndDelete(File f) {
        if (f == null || !f.isFile()) return;
        try (java.io.RandomAccessFile raf = new java.io.RandomAccessFile(f, "rw")) {
            long len = raf.length();
            byte[] zeros = new byte[8192];
            raf.seek(0);
            long rem = len;
            while (rem > 0) {
                int w = (int) Math.min(zeros.length, rem);
                raf.write(zeros, 0, w);
                rem -= w;
            }
            raf.getFD().sync();
        } catch (Exception ignored) {
        } finally {
            //noinspection ResultOfMethodCallIgnored
            f.delete();
        }
    }

    static byte[] toWav(Context ctx, byte[] data) {
        File tmp = null;
        MediaExtractor ex = new MediaExtractor();
        MediaCodec codec = null;
        WipingBytes pcm = new WipingBytes();
        try {
            // («khana-»: if the app is stopped by force while decoding, the core's sweep of the cache at the next start, and «forget
            // everything», remove this copy of the reader's voice too: app::sweep_temp takes every «khana-» file there)
            tmp = File.createTempFile("khana-voice", ".in", ctx.getCacheDir());
            try (FileOutputStream o = new FileOutputStream(tmp)) {
                o.write(data);
            }
            ex.setDataSource(tmp.getPath());
            int track = -1;
            MediaFormat fmt = null;
            String audioMime = null;
            for (int i = 0; i < ex.getTrackCount(); i++) {
                MediaFormat f = ex.getTrackFormat(i);
                String mime = f.getString(MediaFormat.KEY_MIME);
                if (mime != null && mime.startsWith("audio/")) {
                    track = i;
                    fmt = f;
                    audioMime = mime;
                    break;
                }
            }
            if (track < 0) return null;
            ex.selectTrack(track);
            codec = MediaCodec.createDecoderByType(audioMime);
            codec.configure(fmt, null, null, 0);
            codec.start();
            int rate = fmt.getInteger(MediaFormat.KEY_SAMPLE_RATE), channels = fmt.getInteger(MediaFormat.KEY_CHANNEL_COUNT);
            if (rate < 8000 || rate > 192_000 || channels < 1 || channels > 8) return null;
            MediaCodec.BufferInfo info = new MediaCodec.BufferInfo();
            boolean inputDone = false, outputDone = false;
            long deadline = android.os.SystemClock.elapsedRealtime() + DECODE_TIMEOUT_MS;
            int iters = 0;
            while (!outputDone) {
                if (++iters > MAX_ITERATIONS || android.os.SystemClock.elapsedRealtime() > deadline) return null;
                if (!inputDone) {
                    int in = codec.dequeueInputBuffer(10_000);
                    if (in >= 0) {
                        ByteBuffer b = codec.getInputBuffer(in);
                        int n = b == null ? -1 : ex.readSampleData(b, 0);
                        if (n < 0) {
                            codec.queueInputBuffer(in, 0, 0, 0, MediaCodec.BUFFER_FLAG_END_OF_STREAM);
                            inputDone = true;
                        } else {
                            codec.queueInputBuffer(in, 0, n, ex.getSampleTime(), 0);
                            ex.advance();
                        }
                    }
                }
                int out = codec.dequeueOutputBuffer(info, 10_000);
                if (out == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED) {
                    MediaFormat of = codec.getOutputFormat();
                    rate = of.getInteger(MediaFormat.KEY_SAMPLE_RATE);
                    channels = of.getInteger(MediaFormat.KEY_CHANNEL_COUNT);
                    if (rate < 8000 || rate > 192_000 || channels < 1 || channels > 8) return null;
                } else if (out >= 0) {
                    ByteBuffer b = codec.getOutputBuffer(out);
                    if (b != null && info.size > 0) {
                        int take = Math.min(info.size, Math.max(0, MAX_PCM_BYTES - pcm.size()));
                        if (take > 0) {
                            byte[] chunk = new byte[take];
                            b.position(info.offset);
                            b.get(chunk);
                            pcm.write(chunk, 0, chunk.length);
                            java.util.Arrays.fill(chunk, (byte) 0);
                        }
                    }
                    codec.releaseOutputBuffer(out, false);
                    if ((info.flags & MediaCodec.BUFFER_FLAG_END_OF_STREAM) != 0) outputDone = true;
                    long maxByTime = Math.min(MAX_PCM_BYTES, MAX_SECONDS * (long) rate * (long) channels * 2L);
                    if (pcm.size() >= maxByTime) outputDone = true;
                }
            }
            byte[] raw = pcm.toByteArray();
            try {
                return wav(raw, rate, channels);
            } finally {
                java.util.Arrays.fill(raw, (byte) 0);
            }
        } catch (Exception e) {
            Log.w("Khana", "decoding the voice sample failed: " + e.getClass().getSimpleName());
            return null;
        } finally {
            pcm.wipe();
            if (codec != null) {
                try {
                    codec.stop();
                } catch (Exception ignored) {
                }
                codec.release();
            }
            ex.release();
            wipeAndDelete(tmp);
        }
    }

    private static byte[] wav(byte[] pcm, int rate, int channels) {
        ByteBuffer h = ByteBuffer.allocate(44 + pcm.length).order(ByteOrder.LITTLE_ENDIAN);
        h.put("RIFF".getBytes()).putInt(36 + pcm.length).put("WAVE".getBytes());
        h.put("fmt ".getBytes()).putInt(16).putShort((short) 1).putShort((short) channels).putInt(rate)
                .putInt(rate * channels * 2).putShort((short) (channels * 2)).putShort((short) 16);
        h.put("data".getBytes()).putInt(pcm.length).put(pcm);
        return h.array();
    }
}
