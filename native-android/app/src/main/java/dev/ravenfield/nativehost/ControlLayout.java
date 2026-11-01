package dev.ravenfield.nativehost;
/** Coordinates are in the same 1280x720 space as the actual game surface. */
final class ControlLayout {
    static final int FIRE=1000,AIM=1002,LOOK=-1,JOYSTICK=-2;
    static final class Button {
        final String label;final int action;float x,y,r,opacity=1;
        Button(String label,int action,float x,float y,float r){this.label=label;this.action=action;this.x=x;this.y=y;this.r=r;}
        boolean contains(float px,float py,float scale){return (px-x)*(px-x)+(py-y)*(py-y)<=r*r*scale*scale;}
    }
    static final Button[] BUTTONS={
        new Button("FIRE",FIRE,1120,440,52),new Button("AIM",AIM,1000,500,36),
        new Button("JUMP",32,1220,555,36),new Button("RELOAD",82,1130,585,36),
        new Button("CROUCH",17,1000,596,32),new Button("USE",70,875,540,32),
        new Button("SPRINT",16,420,570,34),new Button("MAP",77,980,160,34),
        new Button("1",49,500,660,34),new Button("2",50,595,660,34),
        new Button("3",51,690,660,34),new Button("4",52,785,660,34),new Button("5",53,880,660,34),
        new Button("LOADOUT ↵",13,640,160,42)};
    static final Button[] VEHICLE_BUTTONS={
        new Button("◀",65,170,560,48),new Button("▶",68,310,560,48),
        new Button("THROTTLE",87,1120,480,50),new Button("BRAKE / REVERSE",83,1120,620,44),
        new Button("FIRE",FIRE,1010,360,46),new Button("AIM",AIM,900,440,34),
        new Button("ENTER / EXIT",70,900,550,36),new Button("CAMERA",67,780,440,34),
        new Button("COUNTERMEASURES",88,760,550,32),new Button("RELOAD",82,1220,360,32),
        new Button("MAP",77,980,160,34),new Button("WEAPON 1",49,1120,260,30),new Button("WEAPON 2",50,1210,260,30),
        new Button("S1",112,400,660,26),new Button("S2",113,480,660,26),new Button("S3",114,560,660,26),
        new Button("S4",115,640,660,26),new Button("S5",116,720,660,26),new Button("S6",117,800,660,26),new Button("S7",118,880,660,26),
        new Button("LOADOUT ↵",13,640,160,42)};
    static Button[] defaults(boolean vehicle){return vehicle?VEHICLE_BUTTONS:BUTTONS;}
    static Button[] copyDefaults(boolean vehicle){Button[] source=defaults(vehicle),out=new Button[source.length];for(int i=0;i<out.length;i++){Button b=source[i];out[i]=new Button(b.label,b.action,b.x,b.y,b.r);}return out;}
    static Button[] copyDefaults(){return copyDefaults(false);}
    static void place(Button b,float x,float y,float scale){float r=b.r*scale;b.x=ControlConfig.clamp(x,r,1280-r);b.y=ControlConfig.clamp(y,90+r,720-r);}
    static Button hit(float x,float y,float scale){for(Button button:BUTTONS)if(button.contains(x,y,scale))return button;return null;}
}
