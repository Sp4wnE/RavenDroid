package dev.ravenfield.nativehost;

import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.InputStream;

/** Desktop harness for the exact Android streaming importer; no Android APIs. */
public final class ImportGame {
    public static void main(String[] args) throws Exception {
        if (args.length != 4) throw new IllegalArgumentException("Usage: ImportGame <authorized.apk> <output.data> <expectedBytes> <expectedSha256>");
        GameDataImporter.Profile profile = new GameDataImporter.Profile(Long.parseLong(args[2]), args[3], 15);
        File output = new File(args[1]);
        if (!GameDataImporter.matches(output, profile)) {
            try (InputStream source = new FileInputStream(args[0])) {
                GameDataImporter.importData(source, output, profile, (done, total) -> {});
            }
        }
        if (!GameDataImporter.matches(output, profile)) throw new IOException("Imported hash mismatch");
        System.out.println("Verified game data: " + output.length() + " bytes");
    }
}
