package dev.ravenfield.nativehost;
/** Labels resolved once per view, never in the native frame loop. */
final class ControlLabels {
 static java.util.Map<String,String> load(android.content.Context context){
  java.util.Map<String,String> labels=new java.util.HashMap<>();
  labels.put("FIRE",context.getString(R.string.control_fire));
  labels.put("AIM",context.getString(R.string.control_aim));
  labels.put("JUMP",context.getString(R.string.control_jump));
  labels.put("RELOAD",context.getString(R.string.control_reload));
  labels.put("CROUCH",context.getString(R.string.control_crouch));
  labels.put("USE",context.getString(R.string.control_use));
  labels.put("SPRINT",context.getString(R.string.control_sprint));
  labels.put("MAP",context.getString(R.string.control_map));
  labels.put("LOADOUT ↵",context.getString(R.string.control_loadout));
  labels.put("THROTTLE",context.getString(R.string.control_throttle));
  labels.put("BRAKE / REVERSE",context.getString(R.string.control_brake_reverse));
  labels.put("ENTER / EXIT",context.getString(R.string.control_enter_exit));
  labels.put("CAMERA",context.getString(R.string.control_camera));
  labels.put("COUNTERMEASURES",context.getString(R.string.control_countermeasures));
  labels.put("WEAPON 1",context.getString(R.string.control_weapon_1));
  labels.put("WEAPON 2",context.getString(R.string.control_weapon_2));
  labels.put("S1",context.getString(R.string.control_s1));
  labels.put("S2",context.getString(R.string.control_s2));
  labels.put("S3",context.getString(R.string.control_s3));
  labels.put("S4",context.getString(R.string.control_s4));
  labels.put("S5",context.getString(R.string.control_s5));
  labels.put("S6",context.getString(R.string.control_s6));
  labels.put("S7",context.getString(R.string.control_s7));
  return labels;
 }
}
