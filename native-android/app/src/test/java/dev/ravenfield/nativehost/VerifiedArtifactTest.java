package dev.ravenfield.nativehost;

import org.junit.Test;
import java.io.ByteArrayInputStream;
import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import static org.junit.Assert.*;

public final class VerifiedArtifactTest {
    private static final byte[] ABC = "abc".getBytes(StandardCharsets.US_ASCII);
    private static final String HASH = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    @Test public void copiesVerifiedAssetAndChecksCachedBytes() throws Exception {
        File directory = Files.createTempDirectory("raven-aot-test").toFile();
        File target = new File(directory, "trusted.cwasm");
        try {
            VerifiedArtifact.copy(new ByteArrayInputStream(ABC), target, HASH, 3);
            assertTrue(VerifiedArtifact.matches(target, HASH, 3));
            assertFalse(new File(target + ".partial").exists());
            assertFalse(VerifiedArtifact.matches(target, HASH, 4));
        } finally { target.delete(); directory.delete(); }
    }

    @Test public void badHashOrLengthNeverReplacesPreviousArtifact() throws Exception {
        File directory = Files.createTempDirectory("raven-aot-test").toFile();
        File target = new File(directory, "trusted.cwasm");
        try {
            Files.write(target.toPath(), ABC);
            byte[][] invalid = { "abd".getBytes(StandardCharsets.US_ASCII), new byte[] {1, 2}, new byte[] {1, 2, 3, 4} };
            for (byte[] input : invalid) {
                try { VerifiedArtifact.copy(new ByteArrayInputStream(input), target, HASH, 3); fail("Unverified code accepted"); }
                catch (IOException expected) { /* hash or length mismatch */ }
                assertArrayEquals(ABC, Files.readAllBytes(target.toPath()));
                assertFalse(new File(target + ".partial").exists());
            }
        } finally { target.delete(); directory.delete(); }
    }

    @Test public void corruptCachedArtifactIsNotReused() throws Exception {
        File target = File.createTempFile("raven-aot-test", ".cwasm");
        try {
            Files.write(target.toPath(), "abd".getBytes(StandardCharsets.US_ASCII));
            assertFalse(VerifiedArtifact.matches(target, HASH, 3));
        } finally { target.delete(); }
    }

    @Test public void rejectsMalformedOrOversizedManifestBeforeCopying() throws Exception {
        File directory = Files.createTempDirectory("raven-aot-test").toFile();
        File target = new File(directory, "trusted.cwasm");
        try {
            for (long length : new long[] {0, -1, VerifiedArtifact.MAX_BYTES + 1}) {
                try { VerifiedArtifact.copy(new ByteArrayInputStream(ABC), target, HASH, length); fail("Invalid size accepted"); }
                catch (IOException expected) { /* no output file may be created */ }
            }
            try { VerifiedArtifact.copy(new ByteArrayInputStream(ABC), target, "not-a-sha256", 3); fail("Invalid hash accepted"); }
            catch (IOException expected) { /* malformed manifest */ }
            assertFalse(target.exists());
            assertFalse(new File(target + ".partial").exists());
        } finally { target.delete(); directory.delete(); }
    }
}
