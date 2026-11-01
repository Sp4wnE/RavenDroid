package dev.ravenfield.nativehost;

/** One physical finger maps to the guest's primary mouse pointer. */
final class TouchRouter {
    static final class Event {final int kind,x,y;Event(int kind,int x,int y){this.kind=kind;this.x=x;this.y=y;}}
    private int active=-1,lastX,lastY;
    int active(){return active;}
    static int[] fit(int width,int height){
        if(width<1||height<1)return new int[]{1,1};
        if((long)width*720>(long)height*1280)return new int[]{Math.max(1,(int)((long)height*1280/720)),height};
        return new int[]{width,Math.max(1,(int)((long)width*720/1280))};
    }
    private Event map(int kind,float x,float y,int width,int height){
        if(width<1||height<1||!Float.isFinite(x)||!Float.isFinite(y))return null;
        lastX=(int)Math.max(0,Math.min(1279,Math.floor((double)x*1280/width)));
        lastY=(int)Math.max(0,Math.min(719,Math.floor((double)y*720/height)));
        return new Event(kind,lastX,lastY);
    }
    Event down(int id,float x,float y,int width,int height){
        if(active!=-1||id<0||x<0||y<0||x>=width||y>=height)return null;
        Event event=map(0,x,y,width,height);if(event!=null)active=id;return event;
    }
    Event move(int id,float x,float y,int width,int height){return id==active&&active!=-1?map(1,x,y,width,height):null;}
    Event up(int id,float x,float y,int width,int height){
        if(id!=active||active==-1)return null;Event event=map(2,x,y,width,height);active=-1;return event==null?new Event(3,lastX,lastY):event;
    }
    Event cancel(){if(active==-1)return null;active=-1;return new Event(3,lastX,lastY);}
}
