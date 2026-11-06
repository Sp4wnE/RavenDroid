package dev.ravenfield.nativehost;

import android.content.Context;
import android.content.SharedPreferences;
import android.view.InputDevice;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;

final class MappingPreferences {
    static String prefix(InputDevice device){
        String descriptor=device==null?"fallback":device.getDescriptor();
        try{byte[] hash=MessageDigest.getInstance("SHA-256").digest(descriptor.getBytes(StandardCharsets.UTF_8));StringBuilder out=new StringBuilder("device.");for(byte b:hash)out.append(String.format(java.util.Locale.ROOT,"%02x",b&255));return out+".";}
        catch(java.security.NoSuchAlgorithmException e){throw new AssertionError(e);}
    }
    static SharedPreferences store(Context context){return context.getSharedPreferences("hardware-mappings",Context.MODE_PRIVATE);}
    static InputMapping load(Context context,InputDevice device){
        SharedPreferences p=store(context);String prefix=prefix(device);InputMapping m=new InputMapping();
        for(int key=0;key<=android.view.KeyEvent.getMaxKeyCode();key++)if(p.contains(prefix+"key."+key))m.bind(key,p.getInt(prefix+"key."+key,-1));
        for(int role=0;role<m.axes.length;role++)m.axis(role,p.getInt(prefix+"axis."+role,m.axes[role]),p.getBoolean(prefix+"invert."+role,false));
        m.deadZone=p.getFloat(prefix+"deadZone",m.deadZone);m.sensitivity=p.getFloat(prefix+"sensitivity",1);m.sanitize();return m;
    }
    static void save(Context context,InputDevice device,InputMapping m){
        String prefix=prefix(device);SharedPreferences.Editor edit=store(context).edit();
        m.sanitize();edit.putFloat(prefix+"deadZone",m.deadZone).putFloat(prefix+"sensitivity",m.sensitivity);
        for(java.util.Map.Entry<Integer,Integer> e:m.keys.entrySet())edit.putInt(prefix+"key."+e.getKey(),e.getValue());
        for(int i=0;i<m.axes.length;i++)edit.putInt(prefix+"axis."+i,m.axes[i]).putBoolean(prefix+"invert."+i,m.inverted[i]);edit.apply();
    }
    static void clear(Context context,InputDevice device){SharedPreferences p=store(context);String prefix=prefix(device);SharedPreferences.Editor edit=p.edit();for(String key:p.getAll().keySet())if(key.startsWith(prefix))edit.remove(key);edit.apply();}
}
