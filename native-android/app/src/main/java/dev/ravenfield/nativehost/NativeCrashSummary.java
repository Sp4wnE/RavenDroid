package dev.ravenfield.nativehost;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

/** Reads only the diagnostic subset of Android's Tombstone protobuf, without logs or memory dumps.
 * Schema: https://android.googlesource.com/platform/system/core/+/main/debuggerd/proto/tombstone.proto
 * This is an original bounded wire-format reader; unknown fields are skipped.
 */
final class NativeCrashSummary {
    private NativeCrashSummary() {}

    static String parse(byte[] bytes) throws IOException {
        if (bytes.length > 8 * 1024 * 1024) throw new IOException("Native crash report exceeds the 8 MiB limit");
        Wire wire = new Wire(bytes, 0, bytes.length);
        long tid = 0, pageSize = 0;
        String abort = "", signal = "";
        List<ThreadInfo> threads = new ArrayList<>();
        List<String> causes = new ArrayList<>();
        while (wire.next()) {
            switch (wire.field) {
                case 6: tid = wire.number(); break;
                case 10: signal = signal(wire.child()); break;
                case 14: abort = wire.text(8192); break;
                case 15:
                    Wire cause = wire.child();
                    while (cause.next()) {
                        if (cause.field == 1 && causes.size() < 3) causes.add(cause.text(2048));
                        else cause.skip();
                    }
                    break;
                case 16:
                    if (threads.size() < 128) threads.add(threadEntry(wire.child()));
                    else wire.skip();
                    break;
                case 22: pageSize = wire.number(); break;
                default: wire.skip();
            }
        }
        StringBuilder result = new StringBuilder("Android native tombstone:\n");
        if (!signal.isEmpty()) result.append(signal).append('\n');
        if (pageSize != 0) result.append("page_size=").append(pageSize).append('\n');
        if (!abort.isEmpty()) result.append("Abort message: ").append(abort).append('\n');
        for (String cause : causes) result.append("Cause: ").append(cause).append('\n');
        for (ThreadInfo thread : threads) {
            if (thread.id == tid) {
                result.append("Crashing thread tid=").append(tid).append(" ").append(thread.name).append('\n');
                for (String frame : thread.frames) result.append(frame).append('\n');
                break;
            }
        }
        if (result.length() == "Android native tombstone:\n".length()) result.append("No supported crash fields found.\n");
        return result.toString();
    }

    private static String signal(Wire wire) throws IOException {
        long number = 0, code = 0;
        String name = "", codeName = "";
        while (wire.next()) {
            switch (wire.field) {
                case 1: number = wire.number(); break;
                case 2: name = wire.text(128); break;
                case 3: code = wire.number(); break;
                case 4: codeName = wire.text(128); break;
                default: wire.skip();
            }
        }
        return "signal=" + number + " " + name + " code=" + (int) code + " " + codeName;
    }

    private static final class ThreadInfo {
        long id;
        String name = "";
        final List<String> frames = new ArrayList<>();
    }

    private static ThreadInfo threadEntry(Wire entry) throws IOException {
        ThreadInfo info = new ThreadInfo();
        while (entry.next()) {
            if (entry.field == 1) info.id = entry.number();
            else if (entry.field == 2) {
                Wire thread = entry.child();
                while (thread.next()) {
                    if (thread.field == 1) info.id = thread.number();
                    else if (thread.field == 2) info.name = thread.text(256);
                    else if (thread.field == 4 && info.frames.size() < 32) info.frames.add(frame(thread.child(), info.frames.size()));
                    else thread.skip();
                }
            } else entry.skip();
        }
        return info;
    }

    private static String frame(Wire wire, int index) throws IOException {
        long pc = 0, offset = 0;
        String function = "", file = "", buildId = "";
        while (wire.next()) {
            switch (wire.field) {
                case 1: pc = wire.number(); break;
                case 4: function = wire.text(1024); break;
                case 5: offset = wire.number(); break;
                case 6: file = wire.text(1024); break;
                case 8: buildId = wire.text(128); break;
                default: wire.skip();
            }
        }
        return "#" + index + " pc " + Long.toUnsignedString(pc, 16) + " " + file
                + (function.isEmpty() ? "" : " (" + function + "+" + offset + ")")
                + (buildId.isEmpty() ? "" : " BuildId=" + buildId);
    }

    private static final class Wire {
        private final byte[] data;
        private final int end;
        private int position;
        private int field, type;
        private boolean pending;
        Wire(byte[] data, int start, int end) { this.data = data; this.position = start; this.end = end; }

        boolean next() throws IOException {
            if (pending) throw new IOException("Unread protobuf field");
            if (position == end) return false;
            long key = varint();
            if (key < 8 || key > 0xffffffffL) throw new IOException("Invalid protobuf field key");
            field = (int) (key >>> 3); type = (int) (key & 7); pending = true;
            return true;
        }
        long number() throws IOException { require(0); pending = false; return varint(); }
        Wire child() throws IOException {
            require(2); int length = length(); int start = position; position += length; pending = false;
            return new Wire(data, start, position);
        }
        String text(int maximum) throws IOException {
            Wire value = child();
            int length = Math.min(value.end - value.position, maximum);
            return new String(data, value.position, length, StandardCharsets.UTF_8);
        }
        void skip() throws IOException {
            if (!pending) throw new IOException("Truncated protobuf field");
            switch (type) {
                case 0: varint(); break;
                case 1: advance(8); break;
                case 2: advance(length()); break;
                case 5: advance(4); break;
                default: throw new IOException("Desteklenmeyen protobuf wire type " + type);
            }
            pending = false;
        }
        private void require(int expected) throws IOException {
            if (!pending || type != expected) throw new IOException("Beklenmeyen protobuf wire type");
        }
        private int length() throws IOException {
            long value = varint();
            if (value < 0 || value > end - position) throw new IOException("Truncated or overflowing protobuf field");
            return (int) value;
        }
        private void advance(int count) throws IOException {
            if (count > end - position) throw new IOException("Truncated protobuf field");
            position += count;
        }
        private long varint() throws IOException {
            long value = 0;
            for (int i = 0; i < 10; i++) {
                if (position >= end) throw new IOException("Eksik protobuf varint");
                int next = data[position++] & 255;
                if (i == 9 && next > 1) throw new IOException("Overflowing protobuf varint");
                value |= ((long) (next & 127)) << (i * 7);
                if ((next & 128) == 0) return value;
            }
            throw new IOException("Invalid protobuf varint");
        }
    }
}
