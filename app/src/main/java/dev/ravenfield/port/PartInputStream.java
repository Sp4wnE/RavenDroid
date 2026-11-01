package dev.ravenfield.port;

import java.io.IOException;
import java.io.InputStream;

/** Opens only one data part at a time; never joins game data into a Java byte array. */
public final class PartInputStream extends InputStream {
    public interface Opener { InputStream open(int index) throws IOException; }
    private final Opener opener;
    private final int count;
    private int next;
    private InputStream current;
    private boolean closed;

    public PartInputStream(int count, Opener opener) {
        if (count < 1) throw new IllegalArgumentException("No data parts");
        this.count = count;
        this.opener = opener;
    }

    @Override public int read(byte[] b, int off, int len) throws IOException {
        if (closed) throw new IOException("Stream is closed");
        if (off < 0 || len < 0 || off > b.length - len) throw new IndexOutOfBoundsException();
        if (len == 0) return 0;
        while (true) {
            if (current == null) {
                if (next == count) return -1;
                current = opener.open(next++);
            }
            int n = current.read(b, off, len);
            if (n != -1) return n;
            current.close();
            current = null;
        }
    }

    @Override public int read() throws IOException {
        byte[] one = new byte[1];
        return read(one, 0, 1) == -1 ? -1 : one[0] & 255;
    }

    @Override public void close() throws IOException {
        closed = true;
        if (current != null) { current.close(); current = null; }
    }
}
