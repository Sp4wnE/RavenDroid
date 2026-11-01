package dev.ravenfield.nativehost;

import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

/** Explicit local full-size integration probe; never part of normal unit tests. */
public final class BundledDataProbe {
    public static void main(String[] args) throws Exception {
        if(args.length!=4)throw new IllegalArgumentException("APK, test directory, reviewed bytes, SHA-256 required");
        File directory=new File(args[1]),target=new File(directory,"game.data");
        if(target.exists()||new File(target+".partial").exists())throw new IOException("Probe target must be empty");
        Files.createDirectories(directory.toPath());
        GameDataImporter.Profile profile=new GameDataImporter.Profile(Long.parseLong(args[2]),args[3],15);
        long started=System.nanoTime();
        long[] last={0},next={0};
        try{
            try(ZipFile apk=new ZipFile(args[0])){
                int parts=0;while(apk.getEntry(String.format(java.util.Locale.ROOT,"assets/bundled/game.data.%03d.chunk",parts))!=null)parts++;
                try(InputStream input=new BundledDataStream(parts,index->apk.getInputStream(apk.getEntry(String.format(java.util.Locale.ROOT,"assets/bundled/game.data.%03d.chunk",index))))){
                    GameDataImporter.importData(input,target,profile,(bytes,total)->{
                        if(bytes<last[0])throw new AssertionError("Progress regressed");
                        last[0]=bytes;
                        if(bytes>=next[0]){System.out.println("Prepared "+bytes+" / "+total);next[0]=bytes+128L*1024*1024;}
                    });
                }
            }
            long prepareMs=(System.nanoTime()-started)/1_000_000;
            if(last[0]!=profile.bytes||!GameDataImporter.matches(target,profile)||new File(target+".partial").exists())
                throw new AssertionError("Prepared data verification failed");
            long reuseStarted=System.nanoTime();
            try(InputStream unread=new InputStream(){@Override public int read()throws IOException{throw new IOException("Existing data was imported twice");}}){
                GameDataImporter.importData(unread,target,profile,(bytes,total)->{});
            }
            System.out.println("{\"fresh_import_passed\":true,\"existing_data_reused\":true,\"bytes\":"+profile.bytes+
                    ",\"prepare_ms\":"+prepareMs+",\"reuse_verify_ms\":"+(System.nanoTime()-reuseStarted)/1_000_000+
                    ",\"max_heap_bytes\":"+Runtime.getRuntime().maxMemory()+"}");
        }finally{
            target.setWritable(true);Files.deleteIfExists(target.toPath());
            File partial=new File(target+".partial");partial.setWritable(true);Files.deleteIfExists(partial.toPath());
        }
    }
}
