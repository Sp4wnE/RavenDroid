package dev.ravenfield.nativehost;
import org.junit.Test;
import static org.junit.Assert.*;
public class TouchRouterTest{
    @Test public void letterboxed_surface_preserves_aspect_and_maps_view_coordinates(){
        assertArrayEquals(new int[]{1920,1080},TouchRouter.fit(2400,1080));
        assertArrayEquals(new int[]{1080,607},TouchRouter.fit(1080,2400));
        TouchRouter router=new TouchRouter();TouchRouter.Event down=router.down(7,960,540,1920,1080);
        assertEquals(640,down.x);assertEquals(360,down.y);assertEquals(0,down.kind);
        TouchRouter.Event up=router.up(7,2400,-4,1920,1080);assertEquals(1279,up.x);assertEquals(0,up.y);assertEquals(2,up.kind);assertEquals(-1,router.active());
    }
    @Test public void secondary_fingers_cannot_release_the_primary_pointer(){
        TouchRouter router=new TouchRouter();assertNotNull(router.down(4,100,200,1280,720));
        assertNull(router.down(9,500,400,1280,720));assertNull(router.up(9,500,400,1280,720));assertEquals(4,router.active());
        assertEquals(3,router.cancel().kind);assertEquals(-1,router.active());assertNull(router.cancel());
        assertNull(router.down(2,Float.NaN,0,1280,720));assertNull(router.down(2,1280,0,1280,720));
    }
}
