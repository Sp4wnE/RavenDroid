package dev.ravenfield.nativehost;

import org.junit.Test;
import java.util.*;
import static org.junit.Assert.*;

public class HardwareControlsTest {
    private static class Output implements GameControls.Sink {
        final List<String> events=new ArrayList<>();public boolean send(int k,int a,int b){events.add(k+","+a+","+b);return true;}
    }
    @Test public void keyboard_repeat_and_two_devices_keep_independent_ownership(){
        Output out=new Output();HardwareControls c=new HardwareControls(out);
        c.key(1,29,65,true);c.key(1,29,65,true);c.key(2,29,65,true);
        assertEquals(Arrays.asList("0,65,1"),out.events);
        c.remove(1);assertEquals(1,out.events.size());c.key(2,29,65,false);
        assertEquals("0,65,0",out.events.get(1));
    }
    @Test public void physical_modifier_sides_and_stick_keys_do_not_release_each_other(){
        Output out=new Output();HardwareControls c=new HardwareControls(out);
        c.key(1,59,16,true);c.key(1,60,16,true);c.key(1,59,16,false);assertEquals(1,out.events.size());
        c.key(1,60,16,false);assertEquals("0,16,0",out.events.get(1));
        c.key(1,51,87,true);c.stick(1,0,-1,0,0,0,0);c.stick(1,0,0,0,0,0,0);
        assertEquals(1,Collections.frequency(out.events,"0,87,1"));assertFalse(out.events.contains("0,87,0"));
        c.remove(1);assertTrue(out.events.contains("0,87,0"));
    }
    @Test public void trigger_release_disconnect_and_reset_clear_actions(){
        Output out=new Output();HardwareControls c=new HardwareControls(out);
        c.stick(7,1,0,1,0,1,1);assertTrue(out.events.contains("2,0,1"));assertTrue(out.events.contains("2,2,1"));
        c.remove(7);assertTrue(out.events.contains("0,68,0"));assertTrue(out.events.contains("2,0,0"));assertTrue(out.events.contains("2,2,0"));
        int before=out.events.size();c.tick(.016f,1,false);assertEquals(before,out.events.size());
        c.reset();assertEquals("3,0,0",out.events.get(out.events.size()-1));
    }
    @Test public void right_stick_integrates_elapsed_time_and_dead_zone_rejects_drift(){
        Output a=new Output(),b=new Output();HardwareControls fast=new HardwareControls(a),slow=new HardwareControls(b);
        fast.stick(1,0,0,1,0,0,0);slow.stick(1,0,0,1,0,0,0);
        for(int i=0;i<100;i++)fast.tick(.01f,1,false);for(int i=0;i<20;i++)slow.tick(.05f,1,false);
        int x1=a.events.stream().filter(e->e.startsWith("1,")).mapToInt(e->Integer.parseInt(e.split(",")[1])).sum();
        int x2=b.events.stream().filter(e->e.startsWith("1,")).mapToInt(e->Integer.parseInt(e.split(",")[1])).sum();
        assertTrue(Math.abs(x1-x2)<=1);assertEquals(720,x2);
        assertEquals(0,HardwareControls.axis(.1f,.15f),0);assertEquals(0,HardwareControls.axis(Float.NaN,.15f),0);
    }
    @Test public void touch_and_hardware_release_only_their_own_holds(){
        Output out=new Output();GameInputMixer mixer=new GameInputMixer(out);GameControls.Sink touch=mixer.source(0),hardware=mixer.source(1);
        touch.send(0,87,1);hardware.send(0,87,1);touch.send(3,0,0);
        assertEquals(Arrays.asList("0,87,1"),out.events);
        hardware.send(0,87,0);assertEquals("0,87,0",out.events.get(1));
        touch.send(2,0,1);hardware.send(2,0,1);hardware.send(3,0,0);
        assertEquals("2,0,1",out.events.get(2));touch.send(2,0,0);assertEquals("2,0,0",out.events.get(3));
        mixer.reset();assertEquals("3,0,0",out.events.get(4));
    }
    @Test public void failed_delivery_resets_ownership_and_allows_the_next_press(){
        Output out=new Output();boolean[] fail={true};
        HardwareControls c=new HardwareControls((kind,a,b)->{out.send(kind,a,b);if(kind==0&&fail[0]){fail[0]=false;return false;}return true;});
        c.key(1,51,87,true);assertEquals("3,0,0",out.events.get(1));
        c.key(1,51,87,true);c.key(1,51,87,false);
        assertEquals(Arrays.asList("0,87,1","3,0,0","0,87,1","0,87,0"),out.events);
    }
}
