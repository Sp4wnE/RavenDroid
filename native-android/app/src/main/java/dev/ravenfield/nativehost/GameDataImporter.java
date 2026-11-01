package dev.ravenfield.nativehost;

import java.io.BufferedInputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Locale;
import java.util.zip.GZIPInputStream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

/** Imports only reviewed Unity data into one fixed app-private file, never ZIP paths. */
final class GameDataImporter {
    private static final long MAX_DATA_BYTES = 1200L * 1024 * 1024;
    private static final long MAX_ARCHIVE_ENTRY_BYTES = 1536L * 1024 * 1024;
    interface Progress { void update(long bytes, long total); }
    static final class Profile {
        final long bytes; final String sha256; final int parts;
        Profile(long bytes, String hash, int parts) throws IOException {
            if (bytes < 20 || bytes > MAX_DATA_BYTES || hash == null || !hash.matches("[0-9a-f]{64}") || parts < 1 || parts > 64)
                throw new IOException("Invalid game data profile");
            this.bytes = bytes; this.sha256 = hash; this.parts = parts;
        }
    }
    private GameDataImporter() {}

    static synchronized void importData(InputStream input, File target, Profile profile, Progress progress) throws IOException {
        // Activity recreation must not start two writers for the shared .partial.
        if(matches(target,profile)){progress.update(profile.bytes,profile.bytes);return;}
        File partial = new File(target.getPath() + ".partial");
        try {
            if (partial.exists() && !partial.delete()) throw new IOException("Could not remove the previous partial import");
            if (!find(input, partial, profile, progress, 0))
                throw new IOException("Unity game data was not found. Select your existing game content APK, not the RavenDroid runtime APK.");
            if (!partial.setReadOnly()) throw new IOException("Could not mark game data read-only");
            Files.move(partial.toPath(), target.toPath(), StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
        } finally {
            if (partial.exists() && !partial.delete()) partial.deleteOnExit();
        }
    }

    static boolean matches(File file, Profile profile) throws IOException {
        if (!file.isFile() || file.length() != profile.bytes) return false;
        MessageDigest digest = sha();
        try (InputStream input = new FileInputStream(file)) {
            byte[] buffer = new byte[65536]; int count;
            while ((count = input.read(buffer)) != -1) digest.update(buffer, 0, count);
        }
        return hex(digest.digest()).equals(profile.sha256);
    }

    private static boolean find(InputStream source, File partial, Profile profile, Progress progress, int depth) throws IOException {
        if (depth > 3) throw new IOException("Archive nesting limit exceeded");
        BufferedInputStream input = new BufferedInputStream(source, 65536);
        input.mark(8); byte[] magic = new byte[4]; int read = 0;
        while (read < magic.length) { int count = input.read(magic, read, magic.length - read); if (count == -1) break; read += count; }
        input.reset(); if (read != 4) return false;
        if (magic[0] == 'U' && magic[1] == 'n' && magic[2] == 'i' && magic[3] == 't') {
            copyData(input, partial, profile, progress); return true;
        }
        if ((magic[0] & 255) == 0x1f && (magic[1] & 255) == 0x8b) {
            try (GZIPInputStream gzip = new GZIPInputStream(input, 65536)) { return find(gzip, partial, profile, progress, depth + 1); }
        }
        if (magic[0] != 'P' || magic[1] != 'K') return false;
        try (ZipInputStream zip = new ZipInputStream(input)) {
        Budget entries = new Budget(zip);
        ZipEntry entry; int entryCount = 0;
        while ((entry = zip.getNextEntry()) != null) {
            if (++entryCount > 20000 || entry.getName().length() > 4096) throw new IOException("Archive size limit exceeded");
            String name = entry.getName();
            if (name.equals("assets/native/manifest.json"))
                throw new IOException("This is the runtime APK. Select the game content APK or import from an installed prototype.");
            if (name.equals(partName(0))) {
                Parts parts = new Parts(zip, entries, profile.parts);
                try (GZIPInputStream gzip = new GZIPInputStream(parts, 65536)) {
                    copyData(gzip, partial, profile, progress);
                    drain(parts);
                }
                if (parts.index != profile.parts) throw new IOException("Missing game data part");
                return true;
            }
            if (name.startsWith("assets/game/data.") && name.endsWith(".bin"))
                throw new IOException("Missing or out-of-order game data parts");
            if (!entry.isDirectory() && (name.endsWith(".apk") || name.endsWith(".data")))
                if (find(entries, partial, profile, progress, depth + 1)) return true;
            drain(entries); zip.closeEntry();
        }
        return false;
        }
    }

    private static String partName(int index) { return String.format(Locale.ROOT, "assets/game/data.%03d.bin", index); }
    private static void drain(InputStream input) throws IOException { byte[] discard = new byte[65536]; while (input.read(discard) != -1) { /* bounded scan */ } }

    private static void copyData(InputStream input, File partial, Profile profile, Progress progress) throws IOException {
        MessageDigest digest = sha(); long total = 0, nextUpdate = 0;
        try (FileOutputStream output = new FileOutputStream(partial)) {
            byte[] buffer = new byte[65536]; int count;
            while ((count = input.read(buffer)) != -1) {
                if (count > profile.bytes - total) throw new IOException("Game data exceeds the expected size");
                digest.update(buffer, 0, count); output.write(buffer, 0, count); total += count;
                if (total >= nextUpdate) { progress.update(total, profile.bytes); nextUpdate = total + 16L*1024*1024; }
            }
            if (total != profile.bytes || !hex(digest.digest()).equals(profile.sha256))
                throw new IOException("Game data SHA-256/size verification failed; content does not match the reviewed build");
            output.getFD().sync();
        }
        progress.update(total, profile.bytes);
    }

    private static final class Budget extends InputStream {
        final InputStream source; long total;
        Budget(InputStream source) { this.source = source; }
        @Override public int read(byte[] out, int offset, int length) throws IOException {
            int count = source.read(out, offset, length);
            if (count > 0 && (total += count) > MAX_ARCHIVE_ENTRY_BYTES) throw new IOException("Archive read limit exceeded");
            return count;
        }
        @Override public int read() throws IOException { byte[] one = new byte[1]; return read(one,0,1) == -1 ? -1 : one[0] & 255; }
    }

    private static final class Parts extends InputStream {
        final ZipInputStream zip; final Budget input; final int count; int index;
        Parts(ZipInputStream zip, Budget input, int count) { this.zip = zip; this.input = input; this.count = count; }
        @Override public int read(byte[] out, int offset, int length) throws IOException {
            if (length == 0) return 0;
            while (index < count) {
                int value = input.read(out, offset, length);
                if (value != -1) return value;
                zip.closeEntry(); index++;
                if (index == count) return -1;
                ZipEntry next = zip.getNextEntry();
                if (next == null || !next.getName().equals(partName(index)))
                    throw new IOException("Missing or out-of-order game data part " + index);
            }
            return -1;
        }
        @Override public int read() throws IOException { byte[] one = new byte[1]; return read(one,0,1) == -1 ? -1 : one[0] & 255; }
    }

    private static MessageDigest sha() { try { return MessageDigest.getInstance("SHA-256"); } catch (NoSuchAlgorithmException error) { throw new AssertionError(error); } }
    private static String hex(byte[] bytes) { StringBuilder text = new StringBuilder(); for (byte b : bytes) text.append(String.format(Locale.ROOT,"%02x", b & 255)); return text.toString(); }
}
