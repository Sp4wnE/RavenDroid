package dev.ravenfield.nativehost;
import android.app.AlertDialog;
import android.content.*;
import android.widget.*;

final class ControlPreferences {
    static ControlConfig load(Context context){return load(context,false);}
    static ControlConfig load(Context context,boolean vehicle){
        SharedPreferences p=context.getSharedPreferences("controls",Context.MODE_PRIVATE);ControlConfig c=new ControlConfig(vehicle);
        c.sensitivity=p.getFloat("sensitivity",1);c.aimSensitivity=p.getFloat("aimSensitivity",.55f);c.scale=p.getFloat("scale",1);c.opacity=p.getFloat("opacity",.62f);c.invertY=p.getBoolean("invertY",false);c.aimToggle=p.getBoolean("aimToggle",true);c.sprintToggle=p.getBoolean("sprintToggle",true);c.sanitize();
        try{String extra=p.getString(c.layoutPrefix()+"extra","[]");if(extra.length()<=2048){org.json.JSONArray a=new org.json.JSONArray(extra);for(int i=0;i<Math.min(32,a.length());i++)c.add(a.optInt(i,-1));}}catch(org.json.JSONException ignored){}
        for(ControlLayout.Button b:c.buttons){String k=c.layoutPrefix()+b.action+".";
            // Preserve the user's old crouch position after correcting C to Ctrl.
            if(!vehicle&&b.action==17&&!p.contains(k+"x"))k="hud1.67.";
            b.r=ControlConfig.clamp(p.getFloat(k+"r",b.r),20,100);b.opacity=ControlConfig.clamp(p.getFloat(k+"alpha",1),.1f,1);ControlLayout.place(b,p.getFloat(k+"x",b.x),p.getFloat(k+"y",b.y),c.scale);}
        c.joySize=ControlConfig.clamp(p.getFloat("hud1.joy.size",1),.6f,1.6f);c.joyOpacity=ControlConfig.clamp(p.getFloat("hud1.joy.alpha",.62f),.1f,1);float radius=84*c.scale*c.joySize;
        c.joyX=ControlConfig.clamp(p.getFloat("hud1.joy.x",180),radius,1280-radius);c.joyY=ControlConfig.clamp(p.getFloat("hud1.joy.y",550),90+radius,720-radius);return c;
    }
    static void show(Context context,Runnable changed){
        ControlConfig c=load(context);LinearLayout column=new LinearLayout(context);column.setOrientation(LinearLayout.VERTICAL);column.setPadding(32,12,32,12);
        slider(context,column,context.getString(R.string.ui_look_sensitivity_d2c60),c.sensitivity,.25f,3,value->c.sensitivity=value);
        slider(context,column,context.getString(R.string.ui_aim_sensitivity_0f32a),c.aimSensitivity,.25f,1,value->c.aimSensitivity=value);
        slider(context,column,context.getString(R.string.ui_button_size_cfbb1),c.scale,.85f,1.3f,value->c.scale=value);
        slider(context,column,context.getString(R.string.ui_control_visibility_1cc69),c.opacity,.25f,1,value->c.opacity=value);
        CheckBox invert=new CheckBox(context);invert.setText(context.getString(R.string.ui_invert_vertical_look_e0108));invert.setChecked(c.invertY);column.addView(invert);
        CheckBox aim=new CheckBox(context);aim.setText(context.getString(R.string.ui_tap_to_toggle_aim_d697e));aim.setChecked(c.aimToggle);column.addView(aim);
        CheckBox sprint=new CheckBox(context);sprint.setText(context.getString(R.string.ui_tap_to_toggle_sprint_f1186));sprint.setChecked(c.sprintToggle);column.addView(sprint);
        TextView hint=new TextView(context);hint.setText(context.getString(R.string.ui_left_thumb_movement_right_area_look_hold_a_0ef50));column.addView(hint);
        ScrollView scroll=new ScrollView(context);scroll.addView(column);
        new AlertDialog.Builder(context).setTitle(context.getString(R.string.ui_touch_controls_c6ad8)).setView(scroll).setNegativeButton(context.getString(R.string.ui_cancel_14d24),null).setPositiveButton(context.getString(R.string.ui_save_b8094),(dialog,which)->{
            context.getSharedPreferences("controls",Context.MODE_PRIVATE).edit().putFloat("sensitivity",c.sensitivity).putFloat("aimSensitivity",c.aimSensitivity).putFloat("scale",c.scale).putFloat("opacity",c.opacity).putBoolean("invertY",invert.isChecked()).putBoolean("aimToggle",aim.isChecked()).putBoolean("sprintToggle",sprint.isChecked()).apply();if(changed!=null)changed.run();
        }).show();
    }
    static void saveLayout(Context context,ControlConfig c){SharedPreferences.Editor e=context.getSharedPreferences("controls",Context.MODE_PRIVATE).edit();org.json.JSONArray extras=new org.json.JSONArray();for(int i=0;i<c.buttons.length;i++){ControlLayout.Button b=c.buttons[i];if(i>=ControlLayout.defaults(c.vehicle).length)extras.put(b.action);String k=c.layoutPrefix()+b.action+".";e.putFloat(k+"x",b.x).putFloat(k+"y",b.y).putFloat(k+"r",b.r).putFloat(k+"alpha",b.opacity);}e.putString(c.layoutPrefix()+"extra",extras.toString());if(!c.vehicle)e.putFloat("hud1.joy.x",c.joyX).putFloat("hud1.joy.y",c.joyY).putFloat("hud1.joy.size",c.joySize).putFloat("hud1.joy.alpha",c.joyOpacity);e.apply();}
    interface Change{void set(float value);}
    static void slider(Context context,LinearLayout root,String name,float value,float min,float max,Change change){
        TextView label=new TextView(context);root.addView(label);SeekBar bar=new SeekBar(context);bar.setMax(100);bar.setProgress(Math.round((value-min)*100/(max-min)));root.addView(bar);
        label.setText(name+" · "+Math.round(value*100)+"%");bar.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener(){public void onStartTrackingTouch(SeekBar b){}public void onStopTrackingTouch(SeekBar b){}public void onProgressChanged(SeekBar b,int p,boolean user){float v=min+(max-min)*p/100;change.set(v);label.setText(name+" · "+Math.round(v*100)+"%");}});
    }
}
