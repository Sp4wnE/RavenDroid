package dev.ravenfield.nativehost;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;

/** Decode only the bounded P6 capture emitted by our native renderer. */
final class NativeFrame {
    final int width, height;
    final int[] pixels;
    private NativeFrame(int width,int height,int[] pixels){this.width=width;this.height=height;this.pixels=pixels;}
    static NativeFrame decode(byte[] bytes,String expectedHash)throws IOException{
        if(bytes.length>16*1024*1024)throw new IOException("Native image exceeds the size limit");
        try{
            StringBuilder hash=new StringBuilder();
            for(byte value:MessageDigest.getInstance("SHA-256").digest(bytes))hash.append(String.format(java.util.Locale.ROOT,"%02x",value&255));
            if(!hash.toString().equals(expectedHash))throw new IOException("Image does not match this test report");
        }catch(NoSuchAlgorithmException error){throw new IOException(error);}
        int end=0,lines=0;
        while(end<Math.min(bytes.length,80)&&lines<3){if(bytes[end++]=='\n')lines++;}
        if(lines!=3)throw new IOException("Invalid native image header");
        String[] header=new String(bytes,0,end,StandardCharsets.US_ASCII).split("\n");
        if(header.length!=3||!header[0].equals("P6")||!header[2].equals("255"))throw new IOException("Invalid native image format");
        String[] dimensions=header[1].split(" ");
        if(dimensions.length!=2)throw new IOException("Invalid image dimensions");
        int width,height;
        try{width=Integer.parseInt(dimensions[0]);height=Integer.parseInt(dimensions[1]);}
        catch(NumberFormatException error){throw new IOException("Invalid image dimensions",error);}
        long count=(long)width*height;
        if(width<1||width>8192||height<1||height>8192||count*3!=bytes.length-end)throw new IOException("Incomplete or invalid native image");
        int[] pixels=new int[(int)count];
        for(int index=0;index<pixels.length;index++){
            int offset=end+index*3;
            pixels[index]=0xff000000|((bytes[offset]&255)<<16)|((bytes[offset+1]&255)<<8)|(bytes[offset+2]&255);
        }
        return new NativeFrame(width,height,pixels);
    }
}
