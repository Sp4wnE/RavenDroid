package dev.ravenfield.nativehost;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;

/** Only used for trusted assets inside the signed APK. Never accepts file-picker code. */
final class VerifiedArtifact {
    static final long MAX_BYTES = 768L * 1024 * 1024;
    private VerifiedArtifact() {}

    static boolean matches(File file, String sha256, long length) throws IOException {
        validate(sha256, length);
        if (!file.isFile() || file.length() != length) return false;
        MessageDigest digest = sha256();
        try (InputStream input = new FileInputStream(file)) {
            byte[] buffer = new byte[65536]; int count;
            while ((count = input.read(buffer)) != -1) digest.update(buffer, 0, count);
        }
        return hex(digest.digest()).equals(sha256);
    }

    static void copy(InputStream trustedAsset, File destination, String expectedHash, long length) throws IOException {
        validate(expectedHash, length);
        File temporary = new File(destination.getPath() + ".partial");
        MessageDigest digest = sha256();
        try {
            long total = 0;
            try (FileOutputStream output = new FileOutputStream(temporary)) {
                byte[] buffer = new byte[65536]; int count;
                while ((count = trustedAsset.read(buffer)) != -1) {
                    if (count > length - total) throw new IOException("AOT size does not match the manifest");
                    total += count; digest.update(buffer, 0, count); output.write(buffer, 0, count);
                }
                if (total != length || !hex(digest.digest()).equals(expectedHash))
                    throw new IOException("AOT SHA-256/size verification failed");
                output.getFD().sync();
            }
            if (!temporary.setReadOnly()) throw new IOException("Could not mark the AOT file read-only");
            Files.move(temporary.toPath(), destination.toPath(), StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
        } finally {
            if (temporary.exists() && !temporary.delete()) temporary.deleteOnExit();
        }
    }

    private static void validate(String hash, long length) throws IOException {
        if (hash == null || !hash.matches("[0-9a-f]{64}") || length <= 0 || length > MAX_BYTES)
            throw new IOException("Invalid AOT manifest information");
    }
    private static MessageDigest sha256() {
        try { return MessageDigest.getInstance("SHA-256"); }
        catch (NoSuchAlgorithmException error) { throw new AssertionError(error); }
    }
    private static String hex(byte[] bytes) {
        StringBuilder value = new StringBuilder();
        for (byte b : bytes) value.append(String.format(java.util.Locale.ROOT, "%02x", b & 255));
        return value.toString();
    }
}
