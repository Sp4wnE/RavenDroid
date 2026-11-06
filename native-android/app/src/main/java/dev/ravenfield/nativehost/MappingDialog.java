package dev.ravenfield.nativehost;

import android.app.AlertDialog;
import android.content.Context;
import android.view.InputDevice;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.widget.*;
import java.util.ArrayList;

/** Explicit per-device button capture and axis selection; never guesses an Xbox layout. */
final class MappingDialog extends AlertDialog {
    private final InputDevice device;private InputMapping mapping;
    private final LinearLayout rows;private final TextView status;private Integer waiting,capturedKey;
    static void show(Context context,Runnable done){
        ArrayList<InputDevice> devices=new ArrayList<>();
        for(int id:InputDevice.getDeviceIds()){InputDevice d=InputDevice.getDevice(id);if(d!=null&&!d.isVirtual()&&(HardwareInput.controller(d.getSources())||d.getKeyboardType()==InputDevice.KEYBOARD_TYPE_ALPHABETIC))devices.add(d);}
        if(devices.isEmpty()){new AlertDialog.Builder(context).setTitle(R.string.ui_mapping_title).setMessage(R.string.ui_mapping_connect).setPositiveButton(android.R.string.ok,(d,w)->{if(done!=null)done.run();}).setOnCancelListener(d->{if(done!=null)done.run();}).show();return;}
        String[] names=devices.stream().map(InputDevice::getName).toArray(String[]::new);
        new AlertDialog.Builder(context).setTitle(R.string.ui_mapping_device).setItems(names,(dialog,index)->new MappingDialog(context,devices.get(index),done).show()).setNegativeButton(android.R.string.cancel,(d,w)->{if(done!=null)done.run();}).setOnCancelListener(d->{if(done!=null)done.run();}).show();
    }
    private MappingDialog(Context context,InputDevice device,Runnable done){
        super(context);this.device=device;mapping=MappingPreferences.load(context,device);
        setTitle(context.getString(R.string.ui_mapping_title)+" · "+device.getName());
        LinearLayout body=new LinearLayout(context);body.setOrientation(LinearLayout.VERTICAL);body.setPadding(24,12,24,12);
        status=new TextView(context);status.setText(R.string.ui_mapping_hint);body.addView(status);
        Button bind=new Button(context);bind.setText(R.string.ui_mapping_bind);body.addView(bind);bind.setOnClickListener(view->new AlertDialog.Builder(context).setTitle(R.string.ui_mapping_action).setItems(InputActions.LABELS,(d,index)->{waiting=InputActions.VALUES[index];capturedKey=null;status.setText(context.getString(R.string.ui_mapping_pair,context.getString(R.string.ui_mapping_press),InputActions.label(waiting)));}).show());
        ControlPreferences.slider(context,body,context.getString(R.string.ui_mapping_deadzone),mapping.deadZone,.05f,.5f,v->{mapping.deadZone=v;persist();});
        ControlPreferences.slider(context,body,context.getString(R.string.ui_look_sensitivity_d2c60),mapping.sensitivity,.25f,3,v->{mapping.sensitivity=v;persist();});
        rows=new LinearLayout(context);rows.setOrientation(LinearLayout.VERTICAL);body.addView(rows);refresh();
        ScrollView scroll=new ScrollView(context);scroll.addView(body);setView(scroll);
        setButton(BUTTON_POSITIVE,context.getString(android.R.string.ok),(d,w)->{});
        setButton(BUTTON_NEUTRAL,context.getString(R.string.ui_reset_4a8fc),(d,w)->{MappingPreferences.clear(context,device);});
        setOnDismissListener(d->{waiting=null;if(done!=null)done.run();});
    }
    private void persist(){MappingPreferences.save(getContext(),device,mapping);}
    private void refresh(){
        rows.removeAllViews();String[] names=getContext().getResources().getStringArray(R.array.mapping_axes);
        if(HardwareInput.controller(device.getSources()))for(int role=0;role<mapping.axes.length;role++){
            final int index=role;Button row=new Button(getContext());String axis=mapping.axes[role]<0?"—":MotionEvent.axisToString(mapping.axes[role]);
            row.setText(getContext().getString(R.string.ui_mapping_pair,names[role],axis+(mapping.inverted[role]?" (−)":"")));rows.addView(row);row.setOnClickListener(v->chooseAxis(index,names[index]));
        }
        for(java.util.Map.Entry<Integer,Integer> entry:new java.util.TreeMap<>(mapping.keys).entrySet()){
            Button row=new Button(getContext());row.setText(getContext().getString(R.string.ui_mapping_arrow,KeyEvent.keyCodeToString(entry.getKey()),InputActions.label(entry.getValue())));rows.addView(row);
            row.setOnClickListener(v->new AlertDialog.Builder(getContext()).setTitle(R.string.ui_mapping_action).setItems(InputActions.LABELS,(d,i)->{mapping.bind(entry.getKey(),InputActions.VALUES[i]);persist();refresh();}).show());
        }
    }
    private void chooseAxis(int role,String name){
        ArrayList<Integer> axes=new ArrayList<>();axes.add(-1);
        for(InputDevice.MotionRange r:device.getMotionRanges())if(!axes.contains(r.getAxis()))axes.add(r.getAxis());
        String[] labels=axes.stream().map(axis->axis<0?"—":MotionEvent.axisToString(axis)).toArray(String[]::new);
        new AlertDialog.Builder(getContext()).setTitle(name).setSingleChoiceItems(labels,axes.indexOf(mapping.axes[role]),(dialog,which)->{
            mapping.axis(role,axes.get(which),mapping.inverted[role]);persist();refresh();dialog.dismiss();
            new AlertDialog.Builder(getContext()).setTitle(R.string.ui_mapping_invert).setSingleChoiceItems(new String[]{"+","−"},mapping.inverted[role]?1:0,(d,i)->{mapping.inverted[role]=i==1;persist();refresh();d.dismiss();}).show();
        }).setNegativeButton(android.R.string.cancel,null).show();
    }
    @Override public boolean dispatchKeyEvent(KeyEvent event){
        if(waiting!=null){
            if(event.getKeyCode()==KeyEvent.KEYCODE_BACK){if(event.getAction()==KeyEvent.ACTION_UP){waiting=null;status.setText(R.string.ui_mapping_hint);}return true;}
            if(event.getDeviceId()==device.getId()){
                if(event.getAction()==KeyEvent.ACTION_DOWN&&event.getRepeatCount()==0)capturedKey=event.getKeyCode();
                if(event.getAction()==KeyEvent.ACTION_UP&&capturedKey!=null&&capturedKey==event.getKeyCode()){mapping.bind(event.getKeyCode(),waiting);waiting=null;capturedKey=null;persist();refresh();status.setText(R.string.ui_mapping_hint);}return true;
            }
        }return super.dispatchKeyEvent(event);
    }
    @Override public boolean dispatchGenericMotionEvent(MotionEvent event){
        if(event.getDeviceId()==device.getId()&&HardwareInput.controller(event.getSource())){
            if(waiting==null){StringBuilder values=new StringBuilder(getContext().getString(R.string.ui_mapping_hint));for(InputDevice.MotionRange range:device.getMotionRanges()){float v=event.getAxisValue(range.getAxis());if(Math.abs(v)>.2f)values.append("\n").append(MotionEvent.axisToString(range.getAxis())).append(" ").append(String.format(java.util.Locale.ROOT,"%.2f",v));}status.setText(values);}
            return true;
        }return super.dispatchGenericMotionEvent(event);
    }
}
