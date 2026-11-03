package dev.ravenfield.nativehost;

import java.util.HashMap;
import java.util.Map;

/** Pure input ownership and frame-rate-independent stick look, shared by USB/Bluetooth devices. */
final class HardwareControls {
    private final GameControls.Sink sink;
    private final Map<String,Integer> held=new HashMap<>();
    private final Map<Integer,float[]> sticks=new HashMap<>();
    HardwareControls(GameControls.Sink sink) { this.sink=sink; }
    private void emit(int action,boolean down) { if(!sink.send(action>=1000?2:0,action>=1000?action-1000:action,down?1:0))reset(); }
    void key(int device,int physical,int action,boolean down) {
        String owner=device+":"+physical;
        Integer old=held.get(owner);
        if (old!=null && (!down || old!=action)) {
            held.remove(owner); if (!held.containsValue(old)) emit(old,false);
        }
        if (down && !held.containsKey(owner)) { boolean active=held.containsValue(action);held.put(owner,action);if(!active)emit(action,true); }
    }
    static float axis(float value,float deadZone) {
        if(!Float.isFinite(value))return 0;
        float magnitude=Math.min(1,Math.abs(value));
        return magnitude<=deadZone?0:Math.copySign((magnitude-deadZone)/(1-deadZone),value);
    }
    void stick(int device,float x,float y,float rx,float ry,float leftTrigger,float rightTrigger) {
        key(device,-1,65,x<-.25f);key(device,-2,68,x>.25f);key(device,-3,87,y<-.25f);key(device,-4,83,y>.25f);
        key(device,-5,1002,leftTrigger>.25f);key(device,-6,1000,rightTrigger>.25f);
        float[] state=sticks.computeIfAbsent(device,ignored->new float[4]);state[0]=rx;state[1]=ry;
    }
    void tick(float seconds,float sensitivity,boolean invertY) {
        seconds=Math.max(0,Math.min(.05f,seconds));
        int dx=0,dy=0;
        for(float[] state:sticks.values()) {
            state[2]+=state[0]*720*seconds*sensitivity;state[3]+=state[1]*720*seconds*sensitivity*(invertY?-1:1);
            int x=(int)state[2],y=(int)state[3];state[2]-=x;state[3]-=y;dx+=x;dy+=y;
        }
        if((dx!=0||dy!=0)&&!sink.send(1,Math.max(-4096,Math.min(4096,dx)),Math.max(-4096,Math.min(4096,dy))))reset();
    }
    void remove(int device) {
        String prefix=device+":";
        for(String owner:new java.util.ArrayList<>(held.keySet()))if(owner.startsWith(prefix)) {
            int action=held.remove(owner);if(!held.containsValue(action))emit(action,false);
        }
        sticks.remove(device);
    }
    void reset() { held.clear();sticks.clear();sink.send(3,0,0); }
}
