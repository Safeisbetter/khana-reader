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

    private AudioDecode() {}

    static byte[] toWav(Context ctx, byte[] data) {
        File tmp = null;
        MediaExtractor ex = new MediaExtractor();
        MediaCodec codec = null;
        try {
            tmp = File.createTempFile("voice", ".in", ctx.getCacheDir());
            try (FileOutputStream o = new FileOutputStream(tmp)) {
                o.write(data);
            }
            ex.setDataSource(tmp.getPath());
            int track = -1;
            MediaFormat fmt = null;
            for (int i = 0; i < ex.getTrackCount(); i++) {
                MediaFormat f = ex.getTrackFormat(i);
                String mime = f.getString(MediaFormat.KEY_MIME);
                if (mime != null && mime.startsWith("audio/")) {
                    track = i;
                    fmt = f;
                    break;
                }
            }
            if (track < 0) return null;
            ex.selectTrack(track);
            codec = MediaCodec.createDecoderByType(fmt.getString(MediaFormat.KEY_MIME));
            codec.configure(fmt, null, null, 0);
            codec.start();
            int rate = fmt.getInteger(MediaFormat.KEY_SAMPLE_RATE), channels = fmt.getInteger(MediaFormat.KEY_CHANNEL_COUNT);
            ByteArrayOutputStream pcm = new ByteArrayOutputStream();
            MediaCodec.BufferInfo info = new MediaCodec.BufferInfo();
            boolean inputDone = false, outputDone = false;
            while (!outputDone) {
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
                } else if (out >= 0) {
                    ByteBuffer b = codec.getOutputBuffer(out);
                    if (b != null && info.size > 0) {
                        byte[] chunk = new byte[info.size];
                        b.position(info.offset);
                        b.get(chunk);
                        pcm.write(chunk, 0, chunk.length);
                    }
                    codec.releaseOutputBuffer(out, false);
                    if ((info.flags & MediaCodec.BUFFER_FLAG_END_OF_STREAM) != 0) outputDone = true;
                    if (pcm.size() > MAX_SECONDS * rate * channels * 2) outputDone = true;
                }
            }
            return wav(pcm.toByteArray(), rate, channels);
        } catch (Exception e) {
            Log.w("Khana", "decoding the voice sample: " + e);
            return null;
        } finally {
            if (codec != null) {
                try {
                    codec.stop();
                } catch (Exception ignored) {
                }
                codec.release();
            }
            ex.release();
            if (tmp != null) //noinspection ResultOfMethodCallIgnored
                tmp.delete();
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
