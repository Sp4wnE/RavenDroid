package dev.ravenfield.nativehost;
import org.junit.Test;
import static org.junit.Assert.*;

public class InputMappingTest {
    @Test public void mappings_are_independent_and_reject_invalid_guest_actions(){
        InputMapping a=new InputMapping(),b=new InputMapping();a.bind(96,82);assertEquals(82,a.action(96,32));assertEquals(32,b.action(96,32));
        a.bind(96,9999);assertEquals(82,a.action(96,32));a.bind(96,InputActions.NONE);assertEquals(-1,a.action(96,32));
    }
    @Test public void retroid_axes_can_swap_movement_and_look_with_inversion(){
        InputMapping m=new InputMapping();m.axis(InputMapping.MOVE_X,11,false);m.axis(InputMapping.LOOK_X,0,true);
        assertEquals(11,m.axes[InputMapping.MOVE_X]);assertEquals(0,m.axes[InputMapping.LOOK_X]);assertEquals(-.75f,m.value(InputMapping.LOOK_X,.75f),0);
        m.axis(2,-1,false);assertEquals(-1,m.axes[2]);m.axis(2,100,false);assertEquals(-1,m.axes[2]);
    }
    @Test public void custom_buttons_are_bounded_and_default_buttons_cannot_be_removed(){
        ControlConfig c=new ControlConfig();int count=c.buttons.length;assertTrue(c.add(81));assertFalse(c.add(81));assertFalse(c.add(InputActions.MODE));assertFalse(c.add(-1));
        assertFalse(c.remove(0));assertTrue(c.remove(count));assertEquals(count,c.buttons.length);
        for(int action:InputActions.touchValues())c.add(action);assertEquals(count+32,c.buttons.length);
    }
    @Test public void extra_touch_button_uses_existing_pointer_ownership(){
        ControlConfig c=new ControlConfig();assertTrue(c.add(81));java.util.List<String> out=new java.util.ArrayList<>();GameControls controls=new GameControls(c,(k,a,b)->{out.add(k+":"+a+":"+b);return true;});
        controls.down(1,640,360);assertTrue(controls.held(81));controls.move(1,650,360);controls.up(1);assertFalse(controls.held(81));assertEquals(java.util.Arrays.asList("0:81:1","0:81:0"),out);
    }
    @Test public void report_categories_do_not_treat_every_native_exit_as_memory_shortage(){
        assertEquals(LaunchFailure.Kind.GRAPHICS,LaunchFailure.classify("EGL robustness extension unavailable"));
        assertEquals(LaunchFailure.Kind.MEMORY,LaunchFailure.classify("reason=3 status=0"));
        assertEquals(LaunchFailure.Kind.STORAGE,LaunchFailure.classify("Not enough storage for the AOT file"));
        assertEquals(LaunchFailure.Kind.RUNTIME,LaunchFailure.classify("UnsatisfiedLinkError"));
        assertEquals(LaunchFailure.Kind.UNKNOWN,LaunchFailure.classify("reason=5 status=11"));
        assertEquals(LaunchFailure.Kind.UNKNOWN,LaunchFailure.classify("backend=GLES/EGL shader_program_failures=[] first_blocker=unknown"));
    }
}
