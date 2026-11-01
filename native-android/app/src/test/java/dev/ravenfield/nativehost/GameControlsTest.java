package dev.ravenfield.nativehost;
import org.junit.Test;
import java.util.*;
import static org.junit.Assert.*;

public class GameControlsTest {
    @Test public void vehicle_held_throttle_can_drag_look_and_steering_can_slide(){
        Output out=new Output();GameControls c=new GameControls(new ControlConfig(true),out);
        c.down(1,1120,480);c.move(1,1140,470);assertTrue(c.held(87));assertTrue(out.events.contains("1,20,-10"));
        c.down(2,170,560);c.move(2,310,560);assertFalse(c.held(65));assertTrue(c.held(68));assertTrue(c.held(87));
        c.up(1);assertFalse(c.held(87));assertTrue(c.held(68));c.up(2);assertFalse(c.held(68));
        c.down(3,1120,620);c.move(3,1100,630);assertTrue(c.held(83));c.cancel();assertFalse(c.held(83));
    }
    @Test public void crouch_is_control_and_loadout_is_enter_in_both_profiles(){
        Output out=new Output();GameControls c=new GameControls(new ControlConfig(),out);
        c.down(1,1000,596);assertTrue(c.held(17));assertFalse(c.held(67));c.up(1);assertFalse(c.held(17));
        for(boolean vehicle:new boolean[]{false,true}){c=new GameControls(new ControlConfig(vehicle),out);c.down(2,640,160);assertTrue(c.held(13));c.up(2);assertFalse(c.held(13));}
    }
    @Test public void floating_joystick_starts_neutral_even_at_the_screen_edge(){
        Output out=new Output();GameControls c=new GameControls(new ControlConfig(),out);c.down(3,300,700);
        assertFalse(c.held(83));assertFalse(c.held(87));assertTrue(c.joyY<=575);c.move(3,300,610);assertTrue(c.held(87));
        c.up(3);assertFalse(c.held(87));
    }
    @Test public void sprint_toggle_does_not_require_a_third_held_finger(){
        Output out=new Output();GameControls c=new GameControls(new ControlConfig(),out);c.down(1,420,570);c.up(1);assertTrue(c.held(16));
        c.down(2,420,570);c.up(2);assertFalse(c.held(16));
    }
    @Test public void custom_layout_moves_hit_area_and_preserves_defaults(){
        ControlConfig config=new ControlConfig();ControlLayout.Button fire=config.buttons[0];fire.r=65;ControlLayout.place(fire,600,300,1);fire.opacity=.3f;
        Output out=new Output();GameControls c=new GameControls(config,out);c.down(1,600,300);assertTrue(c.held(1000));c.up(1);c.down(2,1120,440);assertFalse(c.held(1000));
        assertEquals(1120,ControlLayout.BUTTONS[0].x,0);assertEquals(52,ControlLayout.BUTTONS[0].r,0);
        ControlLayout.place(fire,-500,2000,1.3f);assertTrue(fire.x>=fire.r*1.3f);assertTrue(fire.y+fire.r*1.3f<=720);
    }
    @Test public void vehicle_gas_steering_look_and_seats_are_independent(){
        Output out=new Output();GameControls c=new GameControls(new ControlConfig(true),out);
        c.down(1,1120,480);c.down(2,170,560);assertTrue(c.held(87));assertTrue(c.held(65));
        c.down(3,700,300);c.move(3,710,305);assertTrue(out.events.contains("1,10,5"));
        c.up(2);assertTrue(c.held(87));assertFalse(c.held(65));
        c.down(4,400,660);assertTrue(c.held(112));assertTrue(out.events.contains("0,112,1"));c.up(4);assertFalse(c.held(112));
        c.down(5,760,550);assertTrue(c.held(88));c.cancel();assertFalse(c.held(87));assertFalse(c.held(88));assertEquals("3,0,0",out.events.get(out.events.size()-1));
        c=new GameControls(new ControlConfig(true),out);c.down(9,180,700);c.move(9,180,500);assertFalse(c.held(87));
    }
    @Test public void vehicle_layout_is_separate_bounded_and_does_not_overlap(){
        ControlConfig foot=new ControlConfig(),car=new ControlConfig(true);car.buttons[0].x=230;assertEquals(1120,foot.buttons[0].x,0);assertNotEquals(foot.layoutPrefix(),car.layoutPrefix());
        java.util.Set<Integer> actions=new java.util.HashSet<>();
        for(float scale:new float[]{.85f,1f,1.3f})for(int i=0;i<ControlLayout.VEHICLE_BUTTONS.length;i++){
            ControlLayout.Button a=ControlLayout.VEHICLE_BUTTONS[i];assertTrue(a.x-a.r*scale>=0);assertTrue(a.x+a.r*scale<=1280);assertTrue(a.y-a.r*scale>=90);assertTrue(a.y+a.r*scale<=720);if(scale==1)assertTrue(actions.add(a.action));
            for(int j=i+1;j<ControlLayout.VEHICLE_BUTTONS.length;j++){ControlLayout.Button b=ControlLayout.VEHICLE_BUTTONS[j];assertTrue(a.label+" overlaps "+b.label,Math.hypot(a.x-b.x,a.y-b.y)>=(a.r+b.r)*scale);}
        }
    }
    private static final class Output implements GameControls.Sink{final List<String> events=new ArrayList<>();boolean accept=true;public boolean send(int k,int a,int b){events.add(k+","+a+","+b);return accept;}}
    @Test public void movement_look_and_fire_have_independent_pointer_ownership(){
        Output out=new Output();GameControls c=new GameControls(new ControlConfig(),out);
        c.down(7,180,550);c.move(7,180,460);assertTrue(c.held(87));
        c.down(42,800,320);c.move(42,810,325);assertTrue(out.events.contains("1,10,5"));
        c.down(19,1120,440);assertTrue(c.held(1000));
        c.move(19,1140,430);assertTrue(out.events.contains("1,20,-10"));
        c.up(42);assertTrue(c.held(87));assertTrue(c.held(1000));
        c.up(7);assertFalse(c.held(87));assertTrue(c.held(1000));c.up(19);assertFalse(c.held(1000));assertEquals(0,c.pointers());
    }
    @Test public void shared_fire_does_not_release_when_only_one_finger_lifts(){
        Output out=new Output();GameControls c=new GameControls(new ControlConfig(),out);
        c.down(1,1120,440);c.down(9,1120,440);c.up(1);assertTrue(c.held(1000));
        assertEquals(1,Collections.frequency(out.events,"2,0,1"));assertEquals(0,Collections.frequency(out.events,"2,0,0"));
        c.up(9);assertEquals(1,Collections.frequency(out.events,"2,0,0"));
    }
    @Test public void aim_toggle_and_cancel_release_every_action(){
        Output out=new Output();GameControls c=new GameControls(new ControlConfig(),out);
        c.down(1,1000,500);c.up(1);assertTrue(c.held(1002));
        c.down(2,1000,500);c.up(2);assertFalse(c.held(1002));
        c.down(3,1000,500);c.up(3);c.down(4,180,550);c.move(4,240,490);c.down(5,1120,440);
        c.cancel();assertFalse(c.held(1000));assertFalse(c.held(1002));assertFalse(c.held(87));assertEquals(0,c.pointers());assertEquals("3,0,0",out.events.get(out.events.size()-1));
        int count=out.events.size();c.move(4,100,100);c.up(5);assertEquals(count,out.events.size());
    }
    @Test public void fractional_look_is_distance_based_and_aim_sensitivity_is_separate(){
        Output out=new Output();ControlConfig config=new ControlConfig();config.sensitivity=.5f;config.aimSensitivity=.5f;
        GameControls c=new GameControls(config,out);c.down(1,800,300);
        c.move(1,801,300);c.move(1,802,300);assertTrue(out.events.contains("1,1,0"));
        c.down(2,1000,500);c.up(2);c.move(1,810,300);assertEquals("1,2,0",out.events.get(out.events.size()-1));
        config.invertY=true;c.move(1,810,308);assertEquals("1,0,-2",out.events.get(out.events.size()-1));
    }
    @Test public void lost_queue_or_bad_coordinates_cannot_latch_keys(){
        Output out=new Output();GameControls c=new GameControls(new ControlConfig(),out);
        c.down(1,Float.NaN,1);c.down(2,1280,0);assertEquals(0,c.pointers());
        c.down(3,180,550);out.accept=false;c.move(3,180,440);assertEquals(0,c.pointers());assertFalse(c.held(87));assertEquals("3,0,0",out.events.get(out.events.size()-1));
    }
    @Test public void controls_stay_inside_surface_and_button_targets_do_not_overlap(){
        for(float scale:new float[]{.85f,1f,1.3f})for(int i=0;i<ControlLayout.BUTTONS.length;i++){
            ControlLayout.Button a=ControlLayout.BUTTONS[i];assertTrue(a.x-a.r*scale>=0);assertTrue(a.x+a.r*scale<=1280);assertTrue(a.y-a.r*scale>=0);assertTrue(a.y+a.r*scale<=720);
            assertSame(a,ControlLayout.hit(a.x,a.y,scale));
            for(int j=i+1;j<ControlLayout.BUTTONS.length;j++){ControlLayout.Button b=ControlLayout.BUTTONS[j];assertTrue(a.label+" overlaps "+b.label,Math.hypot(a.x-b.x,a.y-b.y)>=(a.r+b.r)*scale);}
        }
    }
}
