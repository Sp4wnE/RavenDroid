package dev.ravenfield.nativehost;

import java.io.IOException;
import java.io.InputStream;

/** One gzip stream split into small assets; only one asset is open at a time. */
final class BundledDataStream extends InputStream {
    interface Opener { InputStream open(int index) throws IOException; }
    private final int count;private final Opener opener;private final byte[] single=new byte[1];
    private int index;private InputStream current;private boolean closed;
    BundledDataStream(int count,Opener opener)throws IOException{
        if(count<1||count>64)throw new IOException("Invalid bundled data part count");
        this.count=count;this.opener=opener;
    }
    @Override public int read(byte[] out,int offset,int length)throws IOException{
        if(offset<0||length<0||offset>out.length-length)throw new IndexOutOfBoundsException();
        if(closed)throw new IOException("Bundled stream closed");
        if(length==0)return 0;
        while(index<count){
            if(current==null){current=opener.open(index);if(current==null)throw new IOException("Missing bundled data part");}
            int read=current.read(out,offset,length);
            if(read!=-1)return read;
            current.close();current=null;index++;
        }
        return -1;
    }
    @Override public int read()throws IOException{return read(single,0,1)==-1?-1:single[0]&255;}
    @Override public void close()throws IOException{closed=true;if(current!=null){InputStream old=current;current=null;old.close();}}
}
