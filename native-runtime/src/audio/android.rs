//! AAudio output. Stream lifetime and close belong to the VM owner, callbacks only mix.
use super::*;
use std::{ffi::c_void,sync::atomic::AtomicI32};
#[link(name="aaudio")]unsafe extern "C"{
 fn AAudio_createStreamBuilder(b:*mut *mut c_void)->i32;fn AAudioStreamBuilder_delete(b:*mut c_void)->i32;
 fn AAudioStreamBuilder_setSampleRate(b:*mut c_void,v:i32);fn AAudioStreamBuilder_setChannelCount(b:*mut c_void,v:i32);fn AAudioStreamBuilder_setFormat(b:*mut c_void,v:i32);fn AAudioStreamBuilder_setSharingMode(b:*mut c_void,v:i32);fn AAudioStreamBuilder_setPerformanceMode(b:*mut c_void,v:i32);fn AAudioStreamBuilder_setUsage(b:*mut c_void,v:i32);
 fn AAudioStreamBuilder_setDataCallback(b:*mut c_void,cb:unsafe extern "C" fn(*mut c_void,*mut c_void,*mut c_void,i32)->i32,user:*mut c_void);
 fn AAudioStreamBuilder_setErrorCallback(b:*mut c_void,cb:unsafe extern "C" fn(*mut c_void,*mut c_void,i32),user:*mut c_void);
 fn AAudioStreamBuilder_openStream(b:*mut c_void,s:*mut *mut c_void)->i32;
 fn AAudioStream_getSampleRate(s:*mut c_void)->i32;fn AAudioStream_getChannelCount(s:*mut c_void)->i32;fn AAudioStream_getFormat(s:*mut c_void)->i32;
 fn AAudioStream_requestStart(s:*mut c_void)->i32;fn AAudioStream_requestStop(s:*mut c_void)->i32;fn AAudioStream_close(s:*mut c_void)->i32;
}
struct Callback{mixer:Arc<Mutex<Mixer>>,muted:Arc<AtomicBool>,underruns:Arc<AtomicU64>,error:Arc<AtomicI32>}
pub(super) struct Stream{raw:*mut c_void,_callback:Box<Callback>}
unsafe extern "C" fn failed(_stream:*mut c_void,user:*mut c_void,error:i32){if !user.is_null(){unsafe{&*(user as *const Callback)}.error.store(error,Ordering::Relaxed);}}
unsafe extern "C" fn render(_stream:*mut c_void,user:*mut c_void,data:*mut c_void,frames:i32)->i32{
 if data.is_null()||user.is_null()||frames<=0{return 1;}
 let c=unsafe{&*(user as *const Callback)};let out=unsafe{std::slice::from_raw_parts_mut(data as *mut f32,frames as usize*2)};
 if let Ok(mut m)=c.mixer.try_lock(){m.render(out);}else{out.fill(0.);c.underruns.fetch_add(1,Ordering::Relaxed);}
 if c.muted.load(Ordering::Relaxed){out.fill(0.);}0
}
impl Stream{
 pub fn open(mixer:Arc<Mutex<Mixer>>,muted:Arc<AtomicBool>,underruns:Arc<AtomicU64>,error:Arc<AtomicI32>)->anyhow::Result<Self>{unsafe{
  let mut b=std::ptr::null_mut();anyhow::ensure!(AAudio_createStreamBuilder(&mut b)==0,"AAudio builder creation failed");
  let mut callback=Box::new(Callback{mixer,muted,underruns,error});AAudioStreamBuilder_setSampleRate(b,48000);AAudioStreamBuilder_setChannelCount(b,2);AAudioStreamBuilder_setFormat(b,2);AAudioStreamBuilder_setSharingMode(b,1);AAudioStreamBuilder_setPerformanceMode(b,12);AAudioStreamBuilder_setUsage(b,14);AAudioStreamBuilder_setDataCallback(b,render,(&mut *callback as *mut Callback).cast());
  AAudioStreamBuilder_setErrorCallback(b,failed,(&mut *callback as *mut Callback).cast());
  let mut raw=std::ptr::null_mut();let result=AAudioStreamBuilder_openStream(b,&mut raw);AAudioStreamBuilder_delete(b);anyhow::ensure!(result==0,"AAudio open failed: {result}");
  let stream=Self{raw,_callback:callback};anyhow::ensure!(AAudioStream_getChannelCount(raw)==2&&AAudioStream_getFormat(raw)==2,"AAudio did not provide stereo float");let rate=AAudioStream_getSampleRate(raw);anyhow::ensure!(rate>0,"Invalid AAudio sample rate");stream._callback.mixer.lock().unwrap().rate=rate as u32;
  let result=AAudioStream_requestStart(raw);anyhow::ensure!(result==0,"AAudio start failed: {result}");Ok(stream)
 }}
}
impl Drop for Stream{fn drop(&mut self){unsafe{AAudioStream_requestStop(self.raw);AAudioStream_close(self.raw);}}}
