package dev.ravenfield.nativehost;

import org.junit.Test;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import static org.junit.Assert.*;

public final class NativeCrashSummaryTest {
    private static byte[] varint(long value) {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        do {
            int part = (int) (value & 127); value >>>= 7;
            out.write(part | (value == 0 ? 0 : 128));
        } while (value != 0);
        return out.toByteArray();
    }
    private static byte[] concat(byte[]... parts) {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        for (byte[] part : parts) out.write(part, 0, part.length);
        return out.toByteArray();
    }
    private static byte[] number(int field, long value) { return concat(varint(field * 8L), varint(value)); }
    private static byte[] message(int field, byte[] value) { return concat(varint(field * 8L + 2), varint(value.length), value); }
    private static byte[] text(int field, String value) { return message(field, value.getBytes(StandardCharsets.UTF_8)); }

    @Test public void decodesAbortSignalAndOnlyCrashingThreadBacktrace() throws Exception {
        byte[] signal = concat(number(1, 6), text(2, "SIGABRT"), number(3, -6), text(4, "SI_TKILL"));
        byte[] frame = concat(number(1, 0x1234), text(4, "wasmtime_test"), number(5, 12), text(6, "libravenfield_native_host.so"), text(8, "abc123"));
        byte[] worker = concat(number(1, 42), text(2, "RavenNativeProbe"), message(4, frame));
        byte[] other = concat(number(1, 24), text(2, "private-other-thread"), message(4, text(6, "not-the-crashing-frame")));
        // Reordered fields and unknown memory/log payloads must not affect decoding.
        byte[] tombstone = concat(message(16, concat(number(1, 24), message(2, other))),
            message(16, concat(number(1, 42), message(2, worker))),
            message(17, new byte[] {0, 1, 2, 3}), number(6, 42), message(10, signal),
            text(14, "example abort cause"), message(15, text(1, "cause detail")), number(22, 16384));
        String result = NativeCrashSummary.parse(tombstone);
        assertTrue(result.contains("signal=6 SIGABRT code=-6 SI_TKILL"));
        assertTrue(result.contains("Abort message: example abort cause"));
        assertTrue(result.contains("Cause: cause detail"));
        assertTrue(result.contains("page_size=16384"));
        assertTrue(result.contains("tid=42 RavenNativeProbe"));
        assertTrue(result.contains("pc 1234 libravenfield_native_host.so (wasmtime_test+12) BuildId=abc123"));
        assertFalse(result.contains("private-other-thread"));
        assertFalse(result.contains("not-the-crashing-frame"));
    }

    @Test public void rejectsTruncatedAndOverflowingInputs() {
        byte[][] invalid = { {0}, {(byte) 0x80}, {0x72, 5, 1}, {0x35, 1},
            {0x30, (byte)0xff,(byte)0xff,(byte)0xff,(byte)0xff,(byte)0xff,
                (byte)0xff,(byte)0xff,(byte)0xff,(byte)0xff,2},
            {0x72, (byte)0xff,(byte)0xff,(byte)0xff,(byte)0xff, 0x7f} };
        for (byte[] input : invalid) {
            try { NativeCrashSummary.parse(input); fail("Malformed input accepted"); }
            catch (IOException expected) { /* report unavailable instead of crashing the launcher */ }
        }
    }

    @Test public void skipsUnknownWireFieldsAndBoundsTextOutput() throws Exception {
        String large = new String(new char[12000]).replace('\0', 'x');
        byte[] input = concat(text(14, large), varint(100 * 8 + 1), new byte[8],
            varint(101 * 8 + 5), new byte[4], number(102, 999));
        String result = NativeCrashSummary.parse(input);
        assertTrue(result.contains("Abort message: "));
        assertTrue(result.length() < 8300);
    }

    @Test public void handlesMissingOptionalFields() throws Exception {
        assertTrue(NativeCrashSummary.parse(new byte[0]).contains("No supported crash fields found."));
        assertTrue(NativeCrashSummary.parse(message(10, new byte[0])).contains("signal=0"));
    }
}
