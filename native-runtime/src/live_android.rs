//! JNI control messages never access a VM or GL object from the UI thread.
use super::{session::{Pointer,registry},diagnostics::Progress};
use jni::{JNIEnv,objects::{JClass,JObject,JString},sys::{jlong,jint,jboolean,jstring}};
use std::path::Path;
#[unsafe(no_mangle)]pub extern "system" fn Java_dev_ravenfield_nativehost_GameActivity_createSessionNative(_env:JNIEnv,_class:JClass)->jlong{registry::create() as jlong}
#[unsafe(no_mangle)]pub extern "system" fn Java_dev_ravenfield_nativehost_GameActivity_audioMutedNative(_env:JNIEnv,_class:JClass,id:jlong,muted:jboolean){if let Some(control)=registry::get(id as u64){control.audio_muted.store(muted!=0,std::sync::atomic::Ordering::Relaxed);}}
#[unsafe(no_mangle)]pub extern "system" fn Java_dev_ravenfield_nativehost_GameActivity_stopSessionNative(_env:JNIEnv,_class:JClass,id:jlong){if let Some(control)=registry::get(id as u64){control.stop();}}
#[unsafe(no_mangle)]pub extern "system" fn Java_dev_ravenfield_nativehost_GameActivity_releaseSessionNative(_env:JNIEnv,_class:JClass,id:jlong){registry::release(id as u64);}
#[unsafe(no_mangle)]pub extern "system" fn Java_dev_ravenfield_nativehost_GameActivity_pointerNative(_env:JNIEnv,_class:JClass,id:jlong,kind:jint,x:jint,y:jint)->jboolean{
    registry::get(id as u64).is_some_and(|control|control.push(Pointer{kind,x,y})) as jboolean
}
#[unsafe(no_mangle)]pub extern "system" fn Java_dev_ravenfield_nativehost_GameActivity_gameInputNative(_env:JNIEnv,_class:JClass,id:jlong,kind:jint,a:jint,b:jint)->jboolean{
    use super::session::GameEvent;
    let Some(event)=GameEvent::from_wire(kind,a,b)else{return 0;};
    registry::get(id as u64).is_some_and(|control|control.push_game(event)) as jboolean
}
#[unsafe(no_mangle)]pub extern "system" fn Java_dev_ravenfield_nativehost_MainActivity_isNativeBusy(_env:JNIEnv,_class:JClass)->jboolean{registry::busy() as jboolean}
#[unsafe(no_mangle)]pub extern "system" fn Java_dev_ravenfield_nativehost_GameActivity_statusNative(env:JNIEnv,_class:JClass,id:jlong)->jstring{
    let text=registry::get(id as u64).map(|control|control.status()).unwrap_or_else(||serde_json::json!({"stopped":true})).to_string();
    env.new_string(text).map(|value|value.into_raw()).unwrap_or(std::ptr::null_mut())
}
#[unsafe(no_mangle)]pub extern "system" fn Java_dev_ravenfield_nativehost_GameActivity_runLiveNative(mut env:JNIEnv,_class:JClass,id:jlong,surface:JObject,path:JString,log:JString,data:JString,width:jint,height:jint)->jstring{
    let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||->anyhow::Result<String>{
        let _execution=registry::Execution::enter(Some(id as u64))?;
        let control=registry::get(id as u64).ok_or_else(||anyhow::anyhow!("Live session no longer exists"))?;
        if control.stopped(){return Ok(serde_json::json!({"stage":"live-session-stopped","first_blocker":null}).to_string());}
        let path:String=env.get_string(&path)?.into();let log:String=env.get_string(&log)?.into();let data:String=env.get_string(&data)?.into();
        let progress=Progress::at(Path::new(&log));progress.record("live-jni-enter");
        let window=super::graphics::NativeWindow::from_surface(&env,&surface)?;
        anyhow::ensure!(super::viewport::supported(width,height),"Unsupported launcher resolution");
        let options=super::ProbeOptions{render_size:(width,height),guest_eh:super::android_profile::GUEST_EH,inline_eager_codegen:super::android_profile::INLINE_EAGER,epoch_guard:super::android_profile::EPOCH_GUARD,save_state:Some(Path::new(&data).with_file_name("native-saves.bin")),android_gles:true,android_window:Some(window),control:Some(control),headless_events:true,native_audio:true,startup_fuel:Some(50_000_000_000),frame_fuel:Some(50_000_000_000),..Default::default()};
        let report=super::probe_impl(&path,None,true,&progress,Some(&data),options)?;
        progress.record("live-jni-returned");Ok(serde_json::to_string_pretty(&report)?)
    }));
    let text=match result{Ok(Ok(text))=>text,Ok(Err(error))=>serde_json::json!({"error":format!("{error:#}"),"playable":false}).to_string(),Err(_)=>serde_json::json!({"error":"Rust panic in live native session","playable":false}).to_string()};
    env.new_string(text).map(|value|value.into_raw()).unwrap_or(std::ptr::null_mut())
}
