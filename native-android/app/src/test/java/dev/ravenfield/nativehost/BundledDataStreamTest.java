package dev.ravenfield.nativehost;
import org.junit.Test;
import java.io.*;
import static org.junit.Assert.*;
public final class BundledDataStreamTest {
    @Test public void joinsPartsLazilyAndHandlesEmptyPartsAndZeroLengthReads()throws Exception{
        int[] opened={0},closed={0};byte[][] parts={{1,2},{},{3,4,5}};
        try(BundledDataStream input=new BundledDataStream(parts.length,index->{opened[0]++;return new ByteArrayInputStream(parts[index]){@Override public void close(){closed[0]++;}};})){
            assertEquals(0,opened[0]);assertEquals(0,input.read(new byte[1],0,0));assertEquals(0,opened[0]);
            assertArrayEquals(new byte[]{1,2,3,4,5},input.readAllBytes());assertEquals(-1,input.read());assertEquals(3,closed[0]);
        }
    }
    @Test public void missingPartFailsAndCloseDoesNotOpenRemainingAssets()throws Exception{
        int[] opened={0};BundledDataStream input=new BundledDataStream(2,index->{opened[0]++;if(index==1)throw new IOException("missing");return new ByteArrayInputStream(new byte[]{1});});
        assertEquals(1,input.read());try{input.read();fail("Missing part accepted");}catch(IOException expected){}
        input.close();assertEquals(2,opened[0]);try{input.read();fail("Closed stream accepted");}catch(IOException expected){}
        try{new BundledDataStream(65,index->null);fail("Unbounded parts accepted");}catch(IOException expected){}
    }
}
