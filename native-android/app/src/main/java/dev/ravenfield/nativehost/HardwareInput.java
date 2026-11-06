package dev.ravenfield.nativehost;

import android.content.Context;
import android.hardware.input.InputManager;
import android.os.Handler;
import android.os.Looper;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.widget.Toast;
import java.util.HashMap;
import java.util.Map;

/** Android event adapter. Device actions are emulated through the existing game ABI. */
final class HardwareInput implements InputManager.InputDeviceListener {
    interface Host {
        boolean ready(); boolean playing(); void mode(boolean playing); void pause();
        void pointer(int kind,int x,int y); void cursor(boolean visible,int x,int y);
    }
    private final Context context;private final View surface;private final Host host;
    private final HardwareControls controls;private final InputManager manager;private final GameControls.Sink mouseSink;
    private final Handler handler=new Handler(Looper.getMainLooper());
    private final Map<Integer,float[]> menuSticks=new HashMap<>();
    private final Map<Integer,InputMapping> mappings=new HashMap<>();
    private InputMapping mapping(InputDevice device){int id=device==null?-1:device.getId();return mappings.computeIfAbsent(id,ignored->MappingPreferences.load(context,device));}
    private final java.util.Set<String> menuButtons=new java.util.HashSet<>();
    private float cursorX=640,cursorY=360;private int weapon=1;private boolean active;
    private long previous;private ControlConfig preferences;
    HardwareInput(Context context,View surface,GameControls.Sink sink,Host host) {
        this.context=context;this.surface=surface;this.host=host;this.mouseSink=sink;controls=new HardwareControls(sink);
        manager=(InputManager)context.getSystemService(Context.INPUT_SERVICE);preferences=ControlPreferences.load(context,false);
        surface.setFocusableInTouchMode(true);
        surface.setOnCapturedPointerListener((view,event)->mouse(event,true));
    }
    void start(){if(active)return;active=true;previous=SystemClock.uptimeMillis();preferences=ControlPreferences.load(context,false);manager.registerInputDeviceListener(this,handler);handler.post(tick);}
    void stop(){if(!active)return;active=false;handler.removeCallbacks(tick);manager.unregisterInputDeviceListener(this);reset();}
    void reset(){controls.reset();mappings.clear();menuSticks.clear();menuButtons.clear();remainderX=remainderY=0;host.pointer(3,(int)cursorX,(int)cursorY);host.cursor(false,0,0);if(surface.hasPointerCapture())surface.releasePointerCapture();}
    private final Runnable tick=new Runnable(){public void run(){
        if(!active)return;long now=SystemClock.uptimeMillis();float dt=Math.min(.05f,(now-previous)/1000f);previous=now;
        if(host.ready()) {
            if(host.playing())controls.tick(dt,preferences.sensitivity,preferences.invertY);
            else {float x=0,y=0;for(float[] axes:menuSticks.values()){x+=axes[0];y+=axes[1];}if(x!=0||y!=0)moveCursor(x*650*dt,y*650*dt);}
        }
        handler.postDelayed(this,16);
    }};
    private void moveCursor(float dx,float dy){cursorX=Math.max(0,Math.min(1279,cursorX+dx));cursorY=Math.max(0,Math.min(719,cursorY+dy));host.pointer(1,(int)cursorX,(int)cursorY);host.cursor(true,(int)cursorX,(int)cursorY);}
    private void menuButton(int device,int key,boolean down){
        boolean before=!menuButtons.isEmpty();String owner=device+":"+key;
        if(down)menuButtons.add(owner);else menuButtons.remove(owner);
        if(before!=!menuButtons.isEmpty())host.pointer(down?0:2,(int)cursorX,(int)cursorY);
        host.cursor(true,(int)cursorX,(int)cursorY);
    }
    private void mode(){boolean playing=!host.playing();reset();host.mode(playing);preferences=ControlPreferences.load(context,false);Toast.makeText(context,playing?R.string.ui_hardware_gameplay:R.string.ui_hardware_menus,Toast.LENGTH_SHORT).show();}
    static boolean controller(int source){return (source&InputDevice.SOURCE_GAMEPAD)==InputDevice.SOURCE_GAMEPAD||(source&InputDevice.SOURCE_JOYSTICK)==InputDevice.SOURCE_JOYSTICK;}
    static boolean mouseSource(int source){return (source&InputDevice.SOURCE_MOUSE)==InputDevice.SOURCE_MOUSE;}
    boolean key(KeyEvent event){
        if(!active||!host.ready()||event.getAction()==KeyEvent.ACTION_MULTIPLE)return false;
        int key=event.getKeyCode(),device=event.getDeviceId();boolean down=event.getAction()==KeyEvent.ACTION_DOWN;
        boolean gamepad=controller(event.getSource())||(event.getDevice()!=null&&controller(event.getDevice().getSources()));
        int action=mapping(event.getDevice()).action(key,gamepad?gamepadKey(key):key==KeyEvent.KEYCODE_F12?InputActions.MODE:key==KeyEvent.KEYCODE_ESCAPE?InputActions.PAUSE:keyboardKey(key));
        if(action==InputActions.MODE){if(down&&event.getRepeatCount()==0)mode();return true;}
        if(action==InputActions.PAUSE) {
            if(down&&event.getRepeatCount()==0){reset();host.pause();}return true;
        }
        if(gamepad) {
            if(!host.playing()) {
                if(action==32){menuButton(device,key,down);return true;}
                if(key==KeyEvent.KEYCODE_DPAD_LEFT||key==KeyEvent.KEYCODE_DPAD_RIGHT||key==KeyEvent.KEYCODE_DPAD_UP||key==KeyEvent.KEYCODE_DPAD_DOWN){
                    float[] axes=menuSticks.computeIfAbsent(device,ignored->new float[2]);
                    if(key==KeyEvent.KEYCODE_DPAD_LEFT||key==KeyEvent.KEYCODE_DPAD_RIGHT)axes[0]=down?(key==KeyEvent.KEYCODE_DPAD_LEFT?-1:1):0;
                    else axes[1]=down?(key==KeyEvent.KEYCODE_DPAD_UP?-1:1):0;return true;
                }
                return key>=KeyEvent.KEYCODE_BUTTON_A&&key<=KeyEvent.KEYCODE_BUTTON_MODE;
            }
        }
        if(action==InputActions.PREVIOUS||action==InputActions.NEXT){if(down&&event.getRepeatCount()==0)cycle(action==InputActions.NEXT?1:-1);return true;}
        if(action<0)return mapping(event.getDevice()).keys.containsKey(key);controls.key(device,key,action,down);return true;
    }
    static int keyboardKey(int key){
        if(key>=KeyEvent.KEYCODE_A&&key<=KeyEvent.KEYCODE_Z)return 65+key-KeyEvent.KEYCODE_A;
        if(key>=KeyEvent.KEYCODE_0&&key<=KeyEvent.KEYCODE_9)return 48+key-KeyEvent.KEYCODE_0;
        if(key>=KeyEvent.KEYCODE_F1&&key<=KeyEvent.KEYCODE_F11)return 112+key-KeyEvent.KEYCODE_F1;
        switch(key){case KeyEvent.KEYCODE_ENTER:case KeyEvent.KEYCODE_NUMPAD_ENTER:return 13;
            case KeyEvent.KEYCODE_SPACE:return 32;case KeyEvent.KEYCODE_TAB:return 9;case KeyEvent.KEYCODE_DEL:return 8;
            case KeyEvent.KEYCODE_SHIFT_LEFT:case KeyEvent.KEYCODE_SHIFT_RIGHT:return 16;
            case KeyEvent.KEYCODE_CTRL_LEFT:case KeyEvent.KEYCODE_CTRL_RIGHT:return 17;
            case KeyEvent.KEYCODE_ALT_LEFT:case KeyEvent.KEYCODE_ALT_RIGHT:return 18;
            case KeyEvent.KEYCODE_DPAD_LEFT:return 37;case KeyEvent.KEYCODE_DPAD_UP:return 38;case KeyEvent.KEYCODE_DPAD_RIGHT:return 39;case KeyEvent.KEYCODE_DPAD_DOWN:return 40;
            default:return -1;}
    }
    static int gamepadKey(int key){switch(key){
        case KeyEvent.KEYCODE_BUTTON_SELECT:case KeyEvent.KEYCODE_BUTTON_MODE:return InputActions.MODE;
        case KeyEvent.KEYCODE_BUTTON_START:case KeyEvent.KEYCODE_BACK:return InputActions.PAUSE;
        case KeyEvent.KEYCODE_BUTTON_L1:return InputActions.PREVIOUS;case KeyEvent.KEYCODE_BUTTON_R1:return InputActions.NEXT;
        case KeyEvent.KEYCODE_BUTTON_A:return 32;case KeyEvent.KEYCODE_BUTTON_B:return 17;
        case KeyEvent.KEYCODE_BUTTON_X:return 82;case KeyEvent.KEYCODE_BUTTON_Y:return 13;
        case KeyEvent.KEYCODE_BUTTON_L2:return 1002;case KeyEvent.KEYCODE_BUTTON_R2:return 1000;
        case KeyEvent.KEYCODE_BUTTON_THUMBL:return 16;case KeyEvent.KEYCODE_BUTTON_THUMBR:return 70;
        case KeyEvent.KEYCODE_DPAD_UP:return 77;case KeyEvent.KEYCODE_DPAD_DOWN:return 88;
        case KeyEvent.KEYCODE_DPAD_LEFT:return 81;case KeyEvent.KEYCODE_DPAD_RIGHT:return 69;default:return -1;
    }}
    private void cycle(int direction){weapon=(weapon-1+direction+5)%5+1;controls.key(Integer.MIN_VALUE,1,48+weapon,true);controls.key(Integer.MIN_VALUE,1,48+weapon,false);}
    private static float axis(MotionEvent e,int axis,boolean trigger){
        InputDevice device=e.getDevice();InputDevice.MotionRange range=device==null?null:device.getMotionRange(axis,e.getSource());
        if(range==null)return 0;float value=e.getAxisValue(axis);
        if(trigger)return Math.max(0,Math.min(1,(value-range.getMin())/Math.max(.001f,range.getRange())));
        return HardwareControls.axis(value,Math.max(.15f,Math.min(.5f,range.getFlat())));
    }
    private static float mappedAxis(MotionEvent event,InputMapping m,int role){
        int axis=m.axes[role];if(axis<0)return 0;
        boolean trigger=role==InputMapping.AIM||role==InputMapping.FIRE;
        // Preserve the previous automatic fallback only for an unmodified role.
        InputDevice device=event.getDevice();
        if(device!=null&&device.getMotionRange(axis,event.getSource())==null){
            if(role==InputMapping.LOOK_X&&axis==MotionEvent.AXIS_Z)axis=MotionEvent.AXIS_RX;
            else if(role==InputMapping.LOOK_Y&&axis==MotionEvent.AXIS_RZ)axis=MotionEvent.AXIS_RY;
            else if(role==InputMapping.AIM&&axis==MotionEvent.AXIS_LTRIGGER)axis=MotionEvent.AXIS_BRAKE;
            else if(role==InputMapping.FIRE&&axis==MotionEvent.AXIS_RTRIGGER)axis=MotionEvent.AXIS_GAS;
        }
        if(device==null||device.getMotionRange(axis,event.getSource())==null)return 0;
        float value=axis(event,axis,trigger);
        if(trigger)return m.inverted[role]?1-value:value;
        // axis() already applies the device flat; user dead zone is applied to raw range.
        InputDevice.MotionRange range=device==null?null:device.getMotionRange(axis,event.getSource());
        return m.value(role,range==null?0:HardwareControls.axis(event.getAxisValue(axis),Math.max(m.deadZone,Math.min(.5f,range.getFlat()))));
    }
    private void mappedButton(int device,int owner,int physical,int fallback,boolean down,InputMapping m){int action=m.action(physical,fallback);if(action>=0&&action<InputActions.MODE)controls.key(device,owner,action,down);}
    boolean motion(MotionEvent event){
        if(!active||!host.ready())return false;
        if(mouseSource(event.getSource()))return mouse(event,false);
        if(!controller(event.getSource())||event.getActionMasked()!=MotionEvent.ACTION_MOVE)return false;
        int device=event.getDeviceId();InputMapping m=mapping(event.getDevice());float x=mappedAxis(event,m,InputMapping.MOVE_X),y=mappedAxis(event,m,InputMapping.MOVE_Y);
        float hatX=mappedAxis(event,m,InputMapping.HAT_X),hatY=mappedAxis(event,m,InputMapping.HAT_Y);
        if(!host.playing()){menuSticks.put(device,new float[]{x!=0?x:hatX,y!=0?y:hatY});return true;}
        mappedButton(device,-7,KeyEvent.KEYCODE_DPAD_LEFT,81,hatX<-.25f,m);mappedButton(device,-8,KeyEvent.KEYCODE_DPAD_RIGHT,69,hatX>.25f,m);mappedButton(device,-9,KeyEvent.KEYCODE_DPAD_UP,77,hatY<-.25f,m);mappedButton(device,-10,KeyEvent.KEYCODE_DPAD_DOWN,88,hatY>.25f,m);
        controls.key(device,-1,65,x<-.25f);controls.key(device,-2,68,x>.25f);controls.key(device,-3,87,y<-.25f);controls.key(device,-4,83,y>.25f);
        mappedButton(device,-5,KeyEvent.KEYCODE_BUTTON_L2,1002,mappedAxis(event,m,InputMapping.AIM)>.25f,m);mappedButton(device,-6,KeyEvent.KEYCODE_BUTTON_R2,1000,mappedAxis(event,m,InputMapping.FIRE)>.25f,m);
        controls.look(device,mappedAxis(event,m,InputMapping.LOOK_X)*m.sensitivity,mappedAxis(event,m,InputMapping.LOOK_Y)*m.sensitivity);return true;
    }
    boolean mouse(MotionEvent event,boolean captured){
        if(!active||!host.ready())return false;int action=event.getActionMasked(),device=event.getDeviceId();
        if(action==MotionEvent.ACTION_CANCEL){reset();return true;}
        if(action==MotionEvent.ACTION_SCROLL){if(host.playing()){float scroll=event.getAxisValue(MotionEvent.AXIS_VSCROLL);if(scroll!=0)cycle(scroll>0?-1:1);}return true;}
        if(host.playing()){
            if(!captured){surface.requestFocus();surface.requestPointerCapture();}
            if(captured&&action==MotionEvent.ACTION_MOVE){float dx=event.getX()*preferences.sensitivity,dy=event.getY()*preferences.sensitivity*(preferences.invertY?-1:1);if(Float.isFinite(dx)&&Float.isFinite(dy))mouseLook(dx,dy);}
            controls.key(device,10001,1000,(event.getButtonState()&MotionEvent.BUTTON_PRIMARY)!=0);
            controls.key(device,10002,1002,(event.getButtonState()&MotionEvent.BUTTON_SECONDARY)!=0);
        }else{
            int[] location=new int[2];surface.getLocationOnScreen(location);
            float x=(event.getRawX()-location[0])*1280/Math.max(1,surface.getWidth()),y=(event.getRawY()-location[1])*720/Math.max(1,surface.getHeight());
            if((x<0||x>=1280||y<0||y>=720)&&!menuButtons.contains(device+":10001"))return false;
            x=Math.max(0,Math.min(1279,x));y=Math.max(0,Math.min(719,y));
            cursorX=x;cursorY=y;host.pointer(1,(int)x,(int)y);host.cursor(false,0,0);
            menuButton(device,10001,(event.getButtonState()&MotionEvent.BUTTON_PRIMARY)!=0);
        }return true;
    }
    private float remainderX,remainderY;
    private void mouseLook(float dx,float dy){remainderX+=dx;remainderY+=dy;int x=(int)remainderX,y=(int)remainderY;remainderX-=x;remainderY-=y;if((x!=0||y!=0)&&!mouseSink.send(1,Math.max(-4096,Math.min(4096,x)),Math.max(-4096,Math.min(4096,y))))reset();}
    @Override public void onInputDeviceAdded(int id){}
    @Override public void onInputDeviceChanged(int id){onInputDeviceRemoved(id);}
    @Override public void onInputDeviceRemoved(int id){controls.remove(id);mappings.remove(id);menuSticks.remove(id);boolean held=!menuButtons.isEmpty();menuButtons.removeIf(owner->owner.startsWith(id+":"));if(held&&menuButtons.isEmpty())host.pointer(2,(int)cursorX,(int)cursorY);}
}
