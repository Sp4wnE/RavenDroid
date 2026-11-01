package dev.ravenfield.nativehost;
import java.util.*;

/** Independent pointer ownership, reference-counted actions and distance-based look. */
final class GameControls {
    interface Sink{boolean send(int kind,int a,int b);}
    private static final class Finger{int role;float x,y,rx,ry;final float originX,originY;Set<Integer> keys=new HashSet<>();Finger(int role,float x,float y){this.role=role;this.x=x;this.y=y;originX=x;originY=y;}}
    private final Map<Integer,Finger> fingers=new HashMap<>();
    private final Map<Integer,Integer> held=new HashMap<>();
    final ControlConfig config;private final Sink sink;private boolean faulted,aimed,sprinting;private int joystick=-1;
    float joyX=180,joyY=550,joyDx,joyDy;
    GameControls(ControlConfig config,Sink sink){this.config=config;this.sink=sink;config.sanitize();joyX=config.joyX;joyY=config.joyY;}
    boolean held(int action){return held.getOrDefault(action,0)>0;}
    int pointers(){return fingers.size();}
    private void send(int kind,int a,int b){if(!sink.send(kind,a,b))faulted=true;}
    private void action(int action,boolean down){
        int old=held.getOrDefault(action,0),now=down?old+1:Math.max(0,old-1);
        if(now==0)held.remove(action);else held.put(action,now);
        if((old==0)==(now==0))return;
        send(action>=1000?2:0,action>=1000?action-1000:action,now>0?1:0);
    }
    void down(int id,float x,float y){
        if(id<0||fingers.containsKey(id)||!valid(x,y))return;
        ControlLayout.Button button=config.hit(x,y);
        int role=button!=null?button.action:(!config.vehicle&&((config.joyX==180&&config.joyY==550)?(x<350&&y>350):Math.hypot(x-config.joyX,y-config.joyY)<110*config.joySize*config.scale)&&joystick==-1?ControlLayout.JOYSTICK:ControlLayout.LOOK);
        Finger finger=new Finger(role,x,y);fingers.put(id,finger);
        if(role==ControlLayout.JOYSTICK){joystick=id;joyX=config.joyX;joyY=config.joyY;joyDx=joyDy=0;}
        else if(role==ControlLayout.AIM&&config.aimToggle){aimed=!aimed;action(role,aimed);}
        else if(role==16&&config.sprintToggle){sprinting=!sprinting;action(role,sprinting);}
        else if(role>=0){action(role,true);}
        recover();
    }
    void move(int id,float x,float y){
        Finger f=fingers.get(id);if(f==null||!Float.isFinite(x)||!Float.isFinite(y))return;
        if(f.role==ControlLayout.JOYSTICK)joystick(f,x,y);
        else if(f.role==ControlLayout.LOOK||f.role==ControlLayout.FIRE||config.vehicle){
            // Slide between steering arrows without lifting; a second finger can
            // continue holding throttle. All vehicle buttons also permit looking.
            if(config.vehicle&&(f.role==65||f.role==68)){
                ControlLayout.Button target=config.hit(x,y);
                if(target!=null&&(target.action==65||target.action==68)&&target.action!=f.role){
                    action(f.role,false);f.role=target.action;action(f.role,true);
                    f.x=x;f.y=y;recover();return;
                }
            }
            float sensitivity=config.sensitivity*(held(ControlLayout.AIM)?config.aimSensitivity:1f);
            f.rx+=(x-f.x)*sensitivity;f.ry+=(y-f.y)*sensitivity*(config.invertY?-1:1);
            int dx=(int)f.rx,dy=(int)f.ry;f.rx-=dx;f.ry-=dy;
            if(dx!=0||dy!=0)send(1,Math.max(-4096,Math.min(4096,dx)),Math.max(-4096,Math.min(4096,dy)));
        }f.x=x;f.y=y;recover();
    }
    private void joystick(Finger f,float x,float y){
        float radius=84*config.scale*config.joySize,dx=(x-f.originX)/radius,dy=(y-f.originY)/radius;
        float distance=(float)Math.hypot(dx,dy);if(distance>1){dx/=distance;dy/=distance;}
        joyDx=dx*radius;joyDy=dy*radius;
        Set<Integer> wanted=new HashSet<>();
        if(distance>=config.deadZone){
            // Entry/release hysteresis prevents key chatter near a diagonal.
            if(dx>(f.keys.contains(68)?.18f:.30f))wanted.add(68);
            if(dx<-(f.keys.contains(65)?.18f:.30f))wanted.add(65);
            if(dy>(f.keys.contains(83)?.18f:.30f))wanted.add(83);
            if(dy<-(f.keys.contains(87)?.18f:.30f))wanted.add(87);
        }
        for(int key:f.keys)if(!wanted.contains(key))action(key,false);
        for(int key:wanted)if(!f.keys.contains(key))action(key,true);
        f.keys=wanted;
    }
    void up(int id){
        Finger f=fingers.remove(id);if(f==null)return;
        for(int key:f.keys)action(key,false);
        if(f.role==ControlLayout.JOYSTICK){joystick=-1;joyDx=joyDy=0;}
        else if(f.role>=0&&!(f.role==ControlLayout.AIM&&config.aimToggle)&&!(f.role==16&&config.sprintToggle))action(f.role,false);
        recover();
    }
    void cancel(){fingers.clear();held.clear();joystick=-1;aimed=false;sprinting=false;joyDx=joyDy=0;faulted=false;sink.send(3,0,0);}
    private void recover(){if(faulted)cancel();}
    private boolean valid(float x,float y){return Float.isFinite(x)&&Float.isFinite(y)&&x>=0&&x<1280&&y>=0&&y<720;}
}
