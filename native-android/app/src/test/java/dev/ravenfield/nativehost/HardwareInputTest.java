package dev.ravenfield.nativehost;
import android.view.InputDevice;
import android.view.KeyEvent;
import org.junit.Test;
import static org.junit.Assert.*;

public class HardwareInputTest {
    @Test public void android_key_codes_map_to_the_guest_keyboard_abi(){
        assertEquals(87,HardwareInput.keyboardKey(KeyEvent.KEYCODE_W));
        assertEquals(66,HardwareInput.keyboardKey(KeyEvent.KEYCODE_B));
        assertEquals(53,HardwareInput.keyboardKey(KeyEvent.KEYCODE_5));
        assertEquals(13,HardwareInput.keyboardKey(KeyEvent.KEYCODE_NUMPAD_ENTER));
        assertEquals(17,HardwareInput.keyboardKey(KeyEvent.KEYCODE_CTRL_RIGHT));
        assertEquals(37,HardwareInput.keyboardKey(KeyEvent.KEYCODE_DPAD_LEFT));
        assertEquals(-1,HardwareInput.keyboardKey(KeyEvent.KEYCODE_VOLUME_UP));
    }
    @Test public void joystick_mouse_and_keyboard_sources_are_distinguished(){
        assertTrue(HardwareInput.controller(InputDevice.SOURCE_GAMEPAD));
        assertTrue(HardwareInput.controller(InputDevice.SOURCE_JOYSTICK));
        assertFalse(HardwareInput.controller(InputDevice.SOURCE_KEYBOARD));
        assertTrue(HardwareInput.mouseSource(InputDevice.SOURCE_MOUSE));
        assertFalse(HardwareInput.mouseSource(InputDevice.SOURCE_TOUCHSCREEN));
    }
}
