package dev.ravenfield.nativehost;
import android.content.Context;
import android.graphics.*;
import android.view.*;

final class GameControlView extends View {
    private static final Typeface LABEL_FONT=Typeface.create("sans-serif-medium",Typeface.NORMAL);
    private final Paint paint=new Paint(Paint.ANTI_ALIAS_FLAG);private final GameControls.Sink sink;private final Runnable menu;
    private final java.util.Map<String,String> buttonLabels;
    private boolean vehicle;private ControlConfig footDraft,vehicleDraft;
    private boolean editing;private Runnable editDone;private int selected=-1,dragPointer=-1;private float dragX,dragY,downX,downY;private boolean moved;
    private GameControls controls;private boolean playing;private int toolbarPointer=-1,toolbarSide;
    private Runnable modeListener=()->{};private boolean externalCursor;private int cursorX,cursorY;
    void setModeListener(Runnable listener){modeListener=listener;}
    void cursor(boolean visible,int x,int y){externalCursor=visible;cursorX=x;cursorY=y;invalidate();}
    GameControlView(Context context,GameControls.Sink sink,Runnable menu){super(context);this.sink=sink;this.menu=menu;buttonLabels=ControlLabels.load(context);reload();setContentDescription(getContext().getString(R.string.ui_touch_movement_camera_and_action_buttons_t_0efc8));}
    void reload(){if(controls!=null)controls.cancel();controls=new GameControls(ControlPreferences.load(getContext(),vehicle),sink);invalidate();}
    void edit(Runnable done){releaseAll();footDraft=ControlPreferences.load(getContext(),false);vehicleDraft=ControlPreferences.load(getContext(),true);controls=new GameControls(vehicle?vehicleDraft:footDraft,sink);editing=true;editDone=done;selected=-1;invalidate();}
    boolean cancelEdit(){if(!editing)return false;finishEdit(false);return true;}
    private void finishEdit(boolean save){if(save){ControlPreferences.saveLayout(getContext(),footDraft);ControlPreferences.saveLayout(getContext(),vehicleDraft);}editing=false;reload();if(editDone!=null){Runnable done=editDone;editDone=null;done.run();}}
    private void switchProfile(){releaseAll();vehicle=!vehicle;selected=-1;dragPointer=-1;controls=new GameControls(editing?(vehicle?vehicleDraft:footDraft):ControlPreferences.load(getContext(),vehicle),sink);invalidate();}
    private void editOptions(){
        if(selected<-1||(vehicle&&selected<0))return;
        ControlConfig c=controls.config;android.widget.LinearLayout body=new android.widget.LinearLayout(getContext());body.setOrientation(android.widget.LinearLayout.VERTICAL);body.setPadding(28,12,28,12);
        if(selected<0){ControlPreferences.slider(getContext(),body,getContext().getString(R.string.ui_joystick_size_da33f),c.joySize,.6f,1.6f,v->{c.joySize=v;placeJoy(c,c.joyX,c.joyY);invalidate();});ControlPreferences.slider(getContext(),body,getContext().getString(R.string.ui_opacity_67e55),c.joyOpacity,.1f,1,v->{c.joyOpacity=v;invalidate();});}
        else{ControlLayout.Button b=c.buttons[selected];float base=ControlLayout.defaults(vehicle)[selected].r;ControlPreferences.slider(getContext(),body,getContext().getString(R.string.ui_button_size_cfbb1),b.r/base,20/base,100/base,v->{b.r=v*base;ControlLayout.place(b,b.x,b.y,c.scale);invalidate();});ControlPreferences.slider(getContext(),body,getContext().getString(R.string.ui_opacity_67e55),b.opacity,.1f,1,v->{b.opacity=v;invalidate();});}
        new android.app.AlertDialog.Builder(getContext()).setTitle(selected<0?"Joystick":buttonLabels.getOrDefault(c.buttons[selected].label,c.buttons[selected].label)).setView(body).setPositiveButton(getContext().getString(R.string.ui_ok_2dfaa),null).show();
    }
    private void placeJoy(ControlConfig c,float x,float y){float r=84*c.scale*c.joySize;c.joyX=ControlConfig.clamp(x,r,1280-r);c.joyY=ControlConfig.clamp(y,90+r,720-r);controls.joyX=c.joyX;controls.joyY=c.joyY;}
    private boolean editTouch(MotionEvent e){int a=e.getActionMasked(),i=e.getActionIndex();float px=x(e,i),py=y(e,i);ControlConfig c=controls.config;
        if(a==MotionEvent.ACTION_DOWN){
            if(py<78){if(px<200)finishEdit(true);else if(px>1080)finishEdit(false);else if(px>820){ControlConfig defaults=new ControlConfig(vehicle);for(int j=0;j<c.buttons.length;j++){ControlLayout.Button b=c.buttons[j],d=defaults.buttons[j];b.x=d.x;b.y=d.y;b.r=d.r;b.opacity=1;}c.joySize=1;c.joyOpacity=.62f;placeJoy(c,180,550);invalidate();}else if(px>540)editOptions();else if(px>210)switchProfile();return true;}
            ControlLayout.Button hit=c.hit(px,py);selected=-2;
            if(hit!=null){for(int j=0;j<c.buttons.length;j++)if(c.buttons[j]==hit)selected=j;dragX=px-hit.x;dragY=py-hit.y;}
            else if(!vehicle&&Math.hypot(px-c.joyX,py-c.joyY)<=84*c.scale*c.joySize){selected=-1;dragX=px-c.joyX;dragY=py-c.joyY;}
            if(selected>=-1){dragPointer=e.getPointerId(i);downX=px;downY=py;moved=false;}invalidate();
        }else if(a==MotionEvent.ACTION_MOVE&&dragPointer!=-1){i=e.findPointerIndex(dragPointer);if(i>=0){if(!moved&&Math.hypot(x(e,i)-downX,y(e,i)-downY)<6)return true;px=x(e,i)-dragX;py=y(e,i)-dragY;if(selected==-1)placeJoy(c,px,py);else ControlLayout.place(c.buttons[selected],px,py,c.scale);moved=true;invalidate();}}
        else if((a==MotionEvent.ACTION_UP||a==MotionEvent.ACTION_POINTER_UP)&&e.getPointerId(i)==dragPointer){dragPointer=-1;performClick();if(!moved)editOptions();}
        else if(a==MotionEvent.ACTION_CANCEL)dragPointer=-1;return true;
    }
    boolean playing(){return playing;}
    void setPlaying(boolean value){controls.cancel();playing=value;modeListener.run();invalidate();}
    void resumePlaying(boolean value){playing=value;modeListener.run();invalidate();}
    void releaseAll(){controls.cancel();toolbarPointer=-1;invalidate();}
    @Override protected void onMeasure(int w,int h){int[] size=TouchRouter.fit(MeasureSpec.getSize(w),MeasureSpec.getSize(h));setMeasuredDimension(size[0],size[1]);}
    private float x(MotionEvent e,int index){return e.getX(index)*1280/getWidth();}
    private float y(MotionEvent e,int index){return e.getY(index)*720/getHeight();}
    @Override public boolean onTouchEvent(MotionEvent event){
        if(getWidth()==0||getHeight()==0)return false;if(editing)return editTouch(event);int action=event.getActionMasked(),index=event.getActionIndex();
        if(action==MotionEvent.ACTION_DOWN){float x=x(event,index),y=y(event,index);
            if((y<78&&(x<270||x>1050))||(y>=80&&y<140&&x<270)){toolbarPointer=event.getPointerId(index);toolbarSide=y>=80?2:x<270?0:1;return true;}
            if(!playing)return false;
        }
        if(toolbarPointer!=-1){
            if((action==MotionEvent.ACTION_UP||action==MotionEvent.ACTION_POINTER_UP)&&event.getPointerId(index)==toolbarPointer){float x=x(event,index),y=y(event,index);toolbarPointer=-1;if(toolbarSide==2&&y>=80&&y<140&&x<270)switchProfile();else if(y<78){if(toolbarSide==0&&x<270)setPlaying(!playing);else if(toolbarSide==1&&x>1050)menu.run();}performClick();}
            else if(action==MotionEvent.ACTION_CANCEL){toolbarPointer=-1;controls.cancel();}return true;
        }
        if(!playing)return false;
        if(action==MotionEvent.ACTION_DOWN||action==MotionEvent.ACTION_POINTER_DOWN)controls.down(event.getPointerId(index),x(event,index),y(event,index));
        else if(action==MotionEvent.ACTION_MOVE){for(int i=0;i<event.getPointerCount();i++)controls.move(event.getPointerId(i),x(event,i),y(event,i));}
        else if(action==MotionEvent.ACTION_UP||action==MotionEvent.ACTION_POINTER_UP)controls.up(event.getPointerId(index));
        else if(action==MotionEvent.ACTION_CANCEL)controls.cancel();
        invalidate();return true;
    }
    @Override public boolean performClick(){super.performClick();return true;}
    private void text(Canvas c,String text,float x,float y,float size,int color){paint.setStyle(Paint.Style.FILL);paint.setColor(color);paint.setTextSize(size);paint.setTypeface(LABEL_FONT);paint.setTextAlign(Paint.Align.CENTER);c.drawText(text,x,y-(paint.ascent()+paint.descent())/2,paint);}
    private void pill(Canvas c,float left,float right,String label){paint.setColor(0xb51a2636);paint.setStyle(Paint.Style.FILL);c.drawRoundRect(left,16,right,64,14,14,paint);text(c,label,(left+right)/2,40,18,0xffeff5ff);}
    @Override protected void onDraw(Canvas canvas){
        super.onDraw(canvas);canvas.save();canvas.scale(getWidth()/1280f,getHeight()/720f);
        if(editing){pill(canvas,16,195,getContext().getString(R.string.ui_save_01b2b));pill(canvas,210,510,vehicle?getContext().getString(R.string.ui_vehicle_on_foot_e5ddf):getContext().getString(R.string.ui_on_foot_vehicle_fa3fd));pill(canvas,540,810,getContext().getString(R.string.ui_size_opacity_267f4));pill(canvas,825,1070,getContext().getString(R.string.ui_reset_4a8fc));pill(canvas,1085,1264,getContext().getString(R.string.ui_cancel_20763));text(canvas,getContext().getString(R.string.ui_drag_buttons_tap_to_adjust_8f616),640,108,17,0xffeff5ff);}
        else{pill(canvas,16,260,playing?getContext().getString(R.string.ui_touch_menus_a4ea9):getContext().getString(R.string.ui_enable_controls_580d7));canvas.save();canvas.translate(0,70);pill(canvas,16,260,vehicle?getContext().getString(R.string.ui_vehicle_on_foot_e5ddf):getContext().getString(R.string.ui_on_foot_vehicle_fa3fd));canvas.restore();pill(canvas,1060,1264,getContext().getString(R.string.ui_options_a13f4));}
        if(playing||editing){
            int alpha=Math.round(controls.config.opacity*255);float scale=controls.config.scale;
            for(ControlLayout.Button button:controls.config.buttons){boolean held=controls.held(button.action);paint.setStyle(Paint.Style.FILL);paint.setColor((Math.round(alpha*button.opacity)<<24)|(held?0x2a8bda:0x142030));canvas.drawCircle(button.x,button.y,button.r*scale,paint);
                paint.setStyle(Paint.Style.STROKE);paint.setStrokeWidth(editing&&selected>=0&&controls.config.buttons[selected]==button?4:held?3:1.5f);paint.setColor(editing&&selected>=0&&controls.config.buttons[selected]==button?0xff89caff:held?0xffb8e1ff:0x99dce8f4);paint.setAlpha(Math.round(alpha*button.opacity));canvas.drawCircle(button.x,button.y,button.r*scale,paint);
                String label=buttonLabels.getOrDefault(button.label,button.label);float size=button.action==ControlLayout.FIRE?22:15;
                paint.setTypeface(LABEL_FONT);paint.setTextSize(size);float width=paint.measureText(label),available=button.r*scale*1.75f;if(width>available)size*=available/width;
                text(canvas,label,button.x,button.y,size,(Math.round(alpha*button.opacity)<<24)|0xf1f6fc);
            }
            if(!vehicle){paint.setStyle(Paint.Style.FILL);paint.setColor((Math.round(controls.config.joyOpacity*128)<<24)|0x142030);canvas.drawCircle(controls.joyX,controls.joyY,84*scale*controls.config.joySize,paint);
            paint.setStyle(Paint.Style.STROKE);paint.setStrokeWidth(2);paint.setColor((Math.round(controls.config.joyOpacity*255)<<24)|0xdce8f4);canvas.drawCircle(controls.joyX,controls.joyY,84*scale*controls.config.joySize,paint);
            paint.setStyle(Paint.Style.FILL);paint.setColor((Math.round(controls.config.joyOpacity*255)<<24)|0xdce8f4);canvas.drawCircle(controls.joyX+controls.joyDx,controls.joyY+controls.joyDy,30*scale*controls.config.joySize,paint);}
        }
        if(externalCursor&&!playing&&!editing){paint.setStyle(Paint.Style.STROKE);paint.setColor(Color.WHITE);paint.setStrokeWidth(2);canvas.drawCircle(cursorX,cursorY,9,paint);canvas.drawLine(cursorX-14,cursorY,cursorX+14,cursorY,paint);canvas.drawLine(cursorX,cursorY-14,cursorX,cursorY+14,paint);}
        canvas.restore();
    }
    @Override protected void onDetachedFromWindow(){releaseAll();super.onDetachedFromWindow();}
}
