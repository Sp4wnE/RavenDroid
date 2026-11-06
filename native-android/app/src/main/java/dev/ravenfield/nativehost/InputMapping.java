package dev.ravenfield.nativehost;

import java.util.HashMap;
import java.util.Map;

/** Per-device mapping data; no dependency on an Android event loop. */
final class InputMapping {
    static final int MOVE_X=0,MOVE_Y=1,LOOK_X=2,LOOK_Y=3,AIM=4,FIRE=5,HAT_X=6,HAT_Y=7;
    // Android AXIS_X/Y/Z/RZ/LTRIGGER/RTRIGGER/HAT_X/HAT_Y.
    final int[] axes={0,1,11,14,17,18,15,16};
    final boolean[] inverted=new boolean[8];
    final Map<Integer,Integer> keys=new HashMap<>();
    float deadZone=.18f,sensitivity=1;
    int action(int physical,int fallback){return keys.getOrDefault(physical,fallback);}
    void bind(int physical,int action){if(InputActions.valid(action,true))keys.put(physical,action);}
    void axis(int role,int axis,boolean invert){if(role>=0&&role<axes.length&&axis>=-1&&axis<=63){axes[role]=axis;inverted[role]=invert;}}
    float value(int role,float value){return inverted[role]?-value:value;}
    void sanitize(){deadZone=ControlConfig.clamp(deadZone,.05f,.5f);sensitivity=ControlConfig.clamp(sensitivity,.25f,3);}
}
