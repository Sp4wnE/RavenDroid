package dev.ravenfield.nativehost;
final class ControlConfig {
    float sensitivity=1f,aimSensitivity=.55f,deadZone=.18f,scale=1f,opacity=.62f;
    boolean invertY,aimToggle=true,sprintToggle=true;
    final boolean vehicle;
    final ControlLayout.Button[] buttons;
    ControlConfig(){this(false);}
    ControlConfig(boolean vehicle){this.vehicle=vehicle;buttons=ControlLayout.copyDefaults(vehicle);}
    String layoutPrefix(){return vehicle?"vehicle.hud1.":"hud1.";}
    float joyX=180,joyY=550,joySize=1,joyOpacity=.62f;
    ControlLayout.Button hit(float x,float y){for(ControlLayout.Button b:buttons)if(b.contains(x,y,scale))return b;return null;}
    void sanitize(){sensitivity=clamp(sensitivity,.25f,3);aimSensitivity=clamp(aimSensitivity,.25f,1);deadZone=clamp(deadZone,.1f,.35f);scale=clamp(scale,.85f,1.3f);opacity=clamp(opacity,.25f,1);}
    static float clamp(float x,float min,float max){return Float.isFinite(x)?Math.max(min,Math.min(max,x)):min;}
}
