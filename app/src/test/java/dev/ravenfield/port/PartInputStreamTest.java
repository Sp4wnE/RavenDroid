package dev.ravenfield.port;

import org.junit.Test;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.zip.GZIPInputStream;
import java.util.zip.GZIPOutputStream;
import static org.junit.Assert.*;

public class PartInputStreamTest {
    @Test public void decompressesGzipAcrossArbitraryPartBoundaries() throws Exception {
        byte[] expected = new byte[10000];
        for (int i = 0; i < expected.length; i++) expected[i] = (byte) (i * 31);
        ByteArrayOutputStream compressed = new ByteArrayOutputStream();
        try (GZIPOutputStream gzip = new GZIPOutputStream(compressed)) { gzip.write(expected); }
        byte[] bytes = compressed.toByteArray();
        AtomicInteger open = new AtomicInteger(), peak = new AtomicInteger();
        PartInputStream joined = new PartInputStream(bytes.length, index -> {
            peak.set(Math.max(peak.get(), open.incrementAndGet()));
            return new ByteArrayInputStream(new byte[]{bytes[index]}) {
                @Override public void close() { open.decrementAndGet(); }
            };
        });
        try (GZIPInputStream gzip = new GZIPInputStream(joined)) {
            assertArrayEquals(expected, gzip.readAllBytes());
        }
        assertEquals(1, peak.get());
        assertEquals(0, open.get());
    }

    @Test public void closingEarlyDoesNotOpenRemainingParts() throws Exception {
        AtomicInteger opened = new AtomicInteger(), closed = new AtomicInteger();
        PartInputStream stream = new PartInputStream(15, index -> {
            opened.incrementAndGet();
            return new ByteArrayInputStream(new byte[]{1, 2, 3}) {
                @Override public void close() { closed.incrementAndGet(); }
            };
        });
        assertEquals(1, stream.read());
        stream.close();
        assertEquals(1, opened.get());
        assertEquals(1, closed.get());
        assertThrows(IOException.class, stream::read);
    }

    @Test public void emptyPartAndUnsignedBytesAreHandled() throws Exception {
        byte[][] parts = {{}, {(byte) 255}, {}, {4}};
        try (InputStream stream = new PartInputStream(parts.length, i -> new ByteArrayInputStream(parts[i]))) {
            assertEquals(0, stream.read(new byte[0]));
            assertEquals(255, stream.read());
            assertEquals(4, stream.read());
            assertEquals(-1, stream.read());
        }
    }

    @Test public void missingPartFailsInsteadOfSilentlyTruncating() throws Exception {
        try (InputStream stream = new PartInputStream(2, i -> {
            if (i == 1) throw new IOException("Missing part");
            return new ByteArrayInputStream(new byte[]{1});
        })) {
            assertEquals(1, stream.read());
            assertThrows(IOException.class, stream::read);
        }
    }
}
