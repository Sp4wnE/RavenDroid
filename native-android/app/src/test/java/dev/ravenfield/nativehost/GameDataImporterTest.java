package dev.ravenfield.nativehost;

import org.junit.Test;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.security.MessageDigest;
import java.util.Locale;
import java.util.zip.GZIPOutputStream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;
import static org.junit.Assert.*;

public final class GameDataImporterTest {
    private static byte[] data() throws Exception {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        bytes.write("UnityWebData1.0\0".getBytes(StandardCharsets.US_ASCII));
        bytes.write(new byte[] {20,0,0,0});
        for(int i=0;i<70000;i++)bytes.write(i&255);
        return bytes.toByteArray();
    }
    private static String hash(byte[] data) throws Exception {
        StringBuilder value=new StringBuilder();
        for(byte b:MessageDigest.getInstance("SHA-256").digest(data))value.append(String.format(Locale.ROOT,"%02x",b&255));
        return value.toString();
    }
    private static byte[] gzip(byte[] data) throws Exception {
        ByteArrayOutputStream out=new ByteArrayOutputStream();try(GZIPOutputStream zip=new GZIPOutputStream(out)){zip.write(data);}return out.toByteArray();
    }
    private static byte[] apk(byte[] data, boolean missing, boolean reordered) throws Exception {
        byte[] compressed=gzip(data);ByteArrayOutputStream out=new ByteArrayOutputStream();
        // Split inside the gzip header, exercising reads across ZIP entry boundaries.
        int[] boundaries={0,1,3,compressed.length};
        try(ZipOutputStream zip=new ZipOutputStream(out)){
            for(int i=0;i<3;i++){
                if(missing&&i==1)continue;
                int index=reordered&&i==1?2:reordered&&i==2?1:i;
                zip.putNextEntry(new ZipEntry(String.format(Locale.ROOT,"assets/game/data.%03d.bin",index)));
                zip.write(compressed,boundaries[i],boundaries[i+1]-boundaries[i]);zip.closeEntry();
            }
        }
        return out.toByteArray();
    }
    private static void check(byte[] source,byte[] expected) throws Exception {
        File directory=Files.createTempDirectory("raven-import").toFile();File target=new File(directory,"game.data");
        try{
            GameDataImporter.Profile profile=new GameDataImporter.Profile(expected.length,hash(expected),3);
            long[] last={0};
            GameDataImporter.importData(new ByteArrayInputStream(source),target,profile,(bytes,total)->{assertTrue(bytes>=last[0]);last[0]=bytes;});
            assertArrayEquals(expected,Files.readAllBytes(target.toPath()));assertEquals(expected.length,last[0]);
            assertTrue(GameDataImporter.matches(target,profile));assertFalse(new File(target+".partial").exists());
        }finally{target.delete();directory.delete();}
    }
    @Test public void importsRawDataAndSplitGzipApk() throws Exception {
        byte[] data=data();check(data,data);check(apk(data,false,false),data);
    }
    @Test public void importsStandaloneBundledGzip() throws Exception {
        byte[] data=data();check(gzip(data),data);
    }
    @Test public void concurrentBundlePreparationHasOneWriterAndReusesVerifiedData() throws Exception {
        byte[] data=data(),compressed=gzip(data);
        File directory=Files.createTempDirectory("raven-bundle-race").toFile();File target=new File(directory,"game.data");
        java.util.concurrent.ExecutorService workers=java.util.concurrent.Executors.newFixedThreadPool(2);
        java.util.concurrent.atomic.AtomicLong consumed=new java.util.concurrent.atomic.AtomicLong();
        GameDataImporter.Profile profile=new GameDataImporter.Profile(data.length,hash(data),3);
        try{
            java.util.concurrent.Callable<Void> task=()->{
                try(ByteArrayInputStream input=new ByteArrayInputStream(compressed){
                    @Override public synchronized int read(byte[] out,int off,int len){int n=super.read(out,off,len);if(n>0)consumed.addAndGet(n);return n;}
                }){GameDataImporter.importData(input,target,profile,(n,t)->{});}return null;
            };
            java.util.concurrent.Future<Void> first=workers.submit(task),second=workers.submit(task);
            first.get();second.get();
            assertTrue(GameDataImporter.matches(target,profile));assertEquals(compressed.length,consumed.get());
            assertFalse(new File(target+".partial").exists());
        }finally{workers.shutdownNow();target.setWritable(true);target.delete();directory.delete();}
    }
    @Test public void rejectsTruncatedBundledGzipAndKeepsEarlierData() throws Exception {
        byte[] data=data(),compressed=gzip(data);
        File directory=Files.createTempDirectory("raven-bundle").toFile();File target=new File(directory,"game.data");
        try{
            Files.write(target.toPath(),new byte[]{1,2,3});
            byte[] truncated=java.util.Arrays.copyOf(compressed,compressed.length-6);
            try{GameDataImporter.importData(new ByteArrayInputStream(truncated),target,new GameDataImporter.Profile(data.length,hash(data),3),(n,t)->{});fail("Truncated bundle accepted");}
            catch(IOException expected){}
            assertArrayEquals(new byte[]{1,2,3},Files.readAllBytes(target.toPath()));
            assertFalse(new File(target+".partial").exists());
        }finally{target.delete();directory.delete();}
    }
    @Test public void importsOuterZipContainingApkWithoutExtractingItsPaths() throws Exception {
        byte[] data=data();ByteArrayOutputStream output=new ByteArrayOutputStream();
        try(ZipOutputStream zip=new ZipOutputStream(output)){
            zip.putNextEntry(new ZipEntry("../../irrelevant.txt"));zip.write("ignore".getBytes(StandardCharsets.US_ASCII));zip.closeEntry();
            zip.putNextEntry(new ZipEntry("game.apk"));zip.write(apk(data,false,false));zip.closeEntry();
        }
        check(output.toByteArray(),data);
    }
    @Test public void rejectsCorruptionMissingAndReorderedPartsWithoutReplacingEarlierData() throws Exception {
        byte[] data=data();File directory=Files.createTempDirectory("raven-import").toFile();File target=new File(directory,"game.data");
        try{
            Files.write(target.toPath(),new byte[]{1,2,3});
            byte[] corrupt=data.clone();corrupt[100]^=1;
            for(byte[] source:new byte[][]{apk(corrupt,false,false),apk(data,true,false),apk(data,false,true)}){
                try{GameDataImporter.importData(new ByteArrayInputStream(source),target,new GameDataImporter.Profile(data.length,hash(data),3),(n,t)->{});fail("Invalid data accepted");}
                catch(IOException expected){}
                assertArrayEquals(new byte[]{1,2,3},Files.readAllBytes(target.toPath()));assertFalse(new File(target+".partial").exists());
            }
        }finally{target.delete();directory.delete();}
    }
    @Test public void rejectsNativeLabApkAndInvalidProfiles() throws Exception {
        byte[] data=data();ByteArrayOutputStream out=new ByteArrayOutputStream();
        try(ZipOutputStream zip=new ZipOutputStream(out)){zip.putNextEntry(new ZipEntry("assets/native/manifest.json"));zip.write("{}".getBytes(StandardCharsets.US_ASCII));zip.closeEntry();}
        File directory=Files.createTempDirectory("raven-import").toFile();File target=new File(directory,"game.data");
        try{
            try{GameDataImporter.importData(new ByteArrayInputStream(out.toByteArray()),target,new GameDataImporter.Profile(data.length,hash(data),3),(n,t)->{});fail("Native Lab accepted as game");}
            catch(IOException expected){assertTrue(expected.getMessage().contains("runtime APK"));}
            assertFalse(target.exists());
            try{new GameDataImporter.Profile(Long.MAX_VALUE,hash(data),3);fail("Oversized profile accepted");}catch(IOException expected){}
        }finally{target.delete();directory.delete();}
    }
}
