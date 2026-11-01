package dev.ravenfield.nativehost;
import org.junit.Test;
import static org.junit.Assert.*;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.Locale;

public class NativeFrameTest{
    private static String hash(byte[] data)throws Exception{
        StringBuilder value=new StringBuilder();for(byte item:MessageDigest.getInstance("SHA-256").digest(data))value.append(String.format(Locale.ROOT,"%02x",item&255));return value.toString();
    }
    @Test public void verifies_capture_and_preserves_rgb_row_order()throws Exception{
        byte[] header="P6\n1 2\n255\n".getBytes(StandardCharsets.US_ASCII);
        byte[] bytes=java.util.Arrays.copyOf(header,header.length+6);
        bytes[header.length]=(byte)255;bytes[header.length+4]=(byte)128;
        NativeFrame frame=NativeFrame.decode(bytes,hash(bytes));
        assertEquals(1,frame.width);assertEquals(2,frame.height);assertArrayEquals(new int[]{0xffff0000,0xff008000},frame.pixels);
        try{NativeFrame.decode(bytes,"stale-report");fail();}catch(IOException expected){}
    }
    @Test public void rejects_truncated_and_overflowing_dimensions()throws Exception{
        for(String value:new String[]{"P6\n8192 8192\n255\n","P6\n2147483647 2147483647\n255\n","P6\n0 0\n255\n","P6\n1 1\n255\nxx"}){
            byte[] bytes=value.getBytes(StandardCharsets.US_ASCII);
            try{NativeFrame.decode(bytes,hash(bytes));fail(value);}catch(IOException expected){}
        }
    }
}
