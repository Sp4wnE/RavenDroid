//! A real Wasmtime host for the pinned Unity ABI. Unsupported services trap.
//! Successful linking is not a playable game or a rendering implementation.
use anyhow::{Context, Result};
use serde::Serialize;
use std::{collections::{BTreeMap,BTreeSet}, time::{Instant,SystemTime,UNIX_EPOCH}};
mod diagnostics;
mod filesystem;
mod io_bridge;
mod environment;
mod saves;
mod graphics;
mod audio;
mod scheduler;
mod execution_guard;
mod performance;
mod exceptions;
mod calendar;
mod pointer;
mod session;
mod game_input;
mod services;
#[cfg(target_os="android")]mod live_android;
#[cfg(target_os="android")]mod android_performance;
#[cfg(target_os="android")]mod android_profile;
mod input;
#[cfg(all(test,feature="compiler"))]mod runtime_tests;
use wasmtime::{Caller, Config, Engine, ExternType, Global, Instance, Linker, Memory,
    Module, Ref, Store, Table, Val};

const DYNAMIC_TOP_PTR: usize = 7_988_528;
const STACK_BASE: i32 = 7_988_544;
const STACK_MAX: i32 = STACK_BASE + 5_242_880;
const MAX_HEAP_BYTES: usize = 2_147_418_112;

#[derive(Default)]
struct Host {
    memory: Option<Memory>,
    table: Option<Table>,
    calls: Vec<(String,u64)>,
    unsupported: Option<String>,
    logs: Vec<String>,
    started: Option<Instant>,
    clock_started: Option<Instant>,
    set_temp_ret: Option<std::rc::Rc<wasmtime::TypedFunc<i32,()>>>,
    tls: BTreeMap<u32,u32>,
    next_tls: u32,
    mappings: BTreeMap<usize,usize>,
    once: BTreeSet<usize>,
    filesystem: Option<filesystem::AssetFs>,
    headless_events: bool,
    headless_audio: bool,
    audio:Option<audio::State>,
    unavailable_services: BTreeSet<String>,
    environment: environment::GuestEnvironment,
    graphics: Option<graphics::Graphics>,
    scheduler:scheduler::Scheduler,
    exceptions:exceptions::Exceptions,
    calendar:calendar::Calendar,
    input:input::Input,
    pointer:pointer::Input,
    control:Option<std::sync::Arc<session::Control>>,
    live:bool,
    profile_imports:bool,
    epoch_guard:bool,
    import_timings:BTreeMap<String,(u64,u128)>,
    trace_gl_calls:bool,
    game_input:game_input::State,
    scripted_game:Vec<(u32,session::GameEvent)>,
    capture_frames:BTreeMap<u32,std::path::PathBuf>,
}

#[derive(Clone, Default)]
pub struct ProbeOptions {
    /// CLI research profile: separate AOT; 10ms epochs, startup60s/callback30s guard.
    pub epoch_guard:bool,
    /// Developer-only guest catch experiment; requires a matching local AOT.
    pub guest_eh:bool,
    /// CLI-only diagnostic: report event devices as unavailable, never as registered.
    pub headless_events: bool,
    /// Explicit host-owned snapshot path. Guest paths cannot select this file.
    pub save_state: Option<std::path::PathBuf>,
    pub desktop_gles: bool,
    pub desktop_keyboard:bool,
    pub space_at_frame:Option<u32>,
    pub tap_at_frame:Option<u32>,
    pub tap_x:i32,
    pub tap_y:i32,
    pub tap_sequence:Vec<(u32,i32,i32)>,
    pub game_sequence:Vec<(u32,i32,i32,i32)>,
    pub capture_frames:Vec<(u32,std::path::PathBuf)>,
    pub(crate) control:Option<std::sync::Arc<session::Control>>,
    #[cfg(target_os="android")]pub(crate) android_window:Option<std::sync::Arc<graphics::NativeWindow>>,
    pub android_gles: bool,
    pub headless_audio: bool,
    pub native_audio:bool,
    pub audio_wav:Option<std::path::PathBuf>,
    pub frame_limit:u32,
    pub startup_fuel:Option<u64>,
    pub frame_fuel:Option<u64>,
    /// Developer-only read-only diagnostics for the pinned guest list globals.
    pub trace_guest_lists:bool,
    pub capture:Option<std::path::PathBuf>,
    /// Opt-in timing adds two clock reads per non-invoke service callback.
    pub profile_imports:bool,
    /// Experimental AOT profile; requires its separately compiled artifact.
    pub inline_eager_codegen:bool,
    /// Expensive formatted last-call traces are disabled in normal gameplay.
    pub trace_gl_calls:bool,
}

const EVENT_CALLBACKS: &[&str] = &[
    "blur", "dblclick", "devicemotion", "deviceorientation", "focus",
    "fullscreenchange", "gamepadconnected", "gamepaddisconnected", "keydown",
    "keypress", "keyup", "mousedown", "mousemove", "mouseup", "touchcancel",
    "touchend", "touchmove", "touchstart", "wheel",
];

// Exact no-op functions in the pinned non-shared-memory JS framework.
const SINGLE_THREAD_NOOPS:&[&str]=&[
    "_emscripten_has_threading_support", // The pinned JS returns 0, unconditionally.
    "_pthread_cond_destroy","_pthread_cond_init","_pthread_cond_timedwait","_pthread_cond_wait",
    "_pthread_mutex_destroy","_pthread_mutex_init","_pthread_mutexattr_destroy","_pthread_mutexattr_init",
    "_pthread_mutexattr_setprotocol","_pthread_mutexattr_settype",
];

#[derive(Serialize)]
pub struct Report {
    pub execution_guard:&'static str,
    pub runtime: &'static str,
    pub architecture: &'static str,
    pub diagnostic_version: &'static str,
    pub compiler_included: bool,
    pub guest_exception_boundaries:bool,
    pub inline_eager_codegen:bool,
    pub compile_ms: u128,
    pub deserialize_ms: u128,
    pub instantiate_ms: u128,
    pub imports: usize,
    pub memory_bytes: usize,
    pub stage: String,
    pub completed_constructors: usize,
    pub first_blocker: Option<String>,
    pub allocator_roundtrip: bool,
    pub startup_ms:u128,
    pub main_loop_work_ms:u128,
    pub max_frame_work_ms:u128,
    pub playable: bool,
    pub rendered_game_frame: bool,
    pub filesystem: Option<filesystem::FsReport>,
    pub headless_events: bool,
    pub headless_audio: bool,
    pub unavailable_services: Vec<String>,
    pub graphics: Option<serde_json::Value>,
    pub audio:Option<serde_json::Value>,
    pub main_loop_frames:u32,
    pub input:serde_json::Value,
    pub scripted_space_frame:Option<u32>,
    pub pointer:serde_json::Value,
    pub live:Option<serde_json::Value>,
    pub stack_global_fast_path:bool,
    pub performance:serde_json::Value,
    pub import_timings:serde_json::Value,
    // Keep device/surface/input summaries before the large diagnostics, so a
    // truncated clipboard paste still includes the result and timing fields.
    pub host_calls: BTreeMap<String, u64>,
    pub logs: Vec<String>,
}

fn host_error(message: impl Into<String>) -> wasmtime::Error {
    wasmtime::Error::msg(message.into())
}

fn value_i32(values: &[Val], index: usize) -> wasmtime::Result<i32> {
    values.get(index).and_then(Val::i32).ok_or_else(|| host_error("Expected i32 argument"))
}

fn read_u32(caller: &mut Caller<'_, Host>, offset: usize) -> wasmtime::Result<u32> {
    let memory = caller.data().memory.ok_or_else(|| host_error("Memory unavailable"))?;
    let mut bytes = [0; 4];
    memory.read(&*caller, offset, &mut bytes).map_err(|e| host_error(e.to_string()))?;
    Ok(u32::from_le_bytes(bytes))
}

fn string_at(caller: &mut Caller<'_, Host>, offset: usize) -> wasmtime::Result<String> {
    let memory = caller.data().memory.ok_or_else(|| host_error("Memory unavailable"))?;
    let data = memory.data(&*caller);
    let end = offset.checked_add(16 * 1024).unwrap_or(data.len()).min(data.len());
    let bytes = data.get(offset..end).ok_or_else(|| host_error("Invalid string address"))?;
    let length = bytes.iter().position(|&byte| byte == 0).unwrap_or(bytes.len());
    Ok(String::from_utf8_lossy(&bytes[..length]).into_owned())
}

fn write_i32(caller: &mut Caller<'_, Host>, offset: usize, value:i32) -> wasmtime::Result<()> {
    let memory=caller.data().memory.ok_or_else(||host_error("Memory unavailable"))?;
    memory.write(caller,offset,&value.to_le_bytes()).map_err(|error|host_error(error.to_string()))
}

fn set_errno(caller: &mut Caller<'_, Host>, value:i32)->wasmtime::Result<()> {
    if let Some(location)=caller.get_export("___errno_location").and_then(|value|value.into_func()) {
        let mut result=[Val::I32(0)];location.call(&mut *caller,&[],&mut result)?;
        write_i32(caller,result[0].i32().unwrap() as u32 as usize,value)?;
    }
    Ok(())
}

fn host_string(caller:&mut Caller<'_,Host>, args:&[Val], value:&str)->wasmtime::Result<i32>{
    let pointer=value_i32(args,0)? as u32 as usize;
    let capacity=value_i32(args,1)? as u32 as usize;
    if pointer!=0 && capacity!=0 {
        let mut count=value.len().min(capacity-1);
        while !value.is_char_boundary(count) {count-=1;}
        let memory=caller.data().memory.unwrap();
        memory.write(&mut *caller,pointer,&value.as_bytes()[..count]).map_err(|e|host_error(e.to_string()))?;
        memory.write(&mut *caller,pointer+count,&[0]).map_err(|e|host_error(e.to_string()))?;
    }
    Ok(value.len() as i32)
}

fn dispatch(mut caller: &mut Caller<'_, Host>, name: &str, trampoline:Option<(&str,usize)>, args: &[Val], results: &mut [Val]) -> wasmtime::Result<()> {
    if let Some((trampoline,slot))=trampoline {
        // This legacy build emulates C function pointer casts in dynCall_*.
        // Calling table[index] directly can select a different signature.
        // Only tagged C++ throws enter the implemented guest catch path.
        return exceptions::invoke(&mut caller,trampoline,slot,args,results);
    }
    if let Some(result)=io_bridge::dispatch(&mut caller,name,args)? {
        results[0]=Val::I32(result);return Ok(());
    }
    if let Some(result)=environment::dispatch(&mut caller,name,args)? {
        results[0]=Val::I32(result);return Ok(());
    }
    if graphics::dispatch(&mut caller,name,args,results)? {return Ok(());}
    if audio::dispatch(&mut caller,name,args,results)?{return Ok(());}
    if scheduler::dispatch(&mut caller,name,args,results)?{return Ok(());}
    if exceptions::dispatch(&mut caller,name,args,results)?{return Ok(());}
    if calendar::dispatch(&mut caller,name,args,results)?{return Ok(());}
    if input::dispatch(&mut caller,name,args,results)?{return Ok(());}
    if pointer::dispatch(&mut caller,name,args,results)?{return Ok(());}
    if name.strip_prefix("env.").is_some_and(|name|SINGLE_THREAD_NOOPS.contains(&name)) {
        results[0]=Val::I32(0);return Ok(());
    }
    if caller.data().headless_events && name.strip_prefix("env._emscripten_set_")
        .and_then(|name| name.strip_suffix("_callback_on_thread"))
        .is_some_and(|event| EVENT_CALLBACKS.contains(&event)) {
        // Emscripten's explicit failure code, not a successful registration.
        // No callback, device capability, or input delivery is fabricated.
        caller.data_mut().unavailable_services.insert(name.to_string());
        results[0] = Val::I32(-1); // EMSCRIPTEN_RESULT_NOT_SUPPORTED
        return Ok(());
    }
    if caller.data().headless_events && matches!(name,"env._emscripten_get_fullscreen_status"|"env._emscripten_get_gamepad_status"|"env._emscripten_get_num_gamepads"){
        caller.data_mut().unavailable_services.insert(name.into());
        results[0]=Val::I32(if name=="env._emscripten_get_num_gamepads"{0}else{-1});return Ok(());
    }
    match name {
        "env.___buildEnvironment"=>environment::build(&mut caller,value_i32(args,0)?)?,
        "env._JS_FileSystem_Sync"=>{
            caller.data_mut().filesystem.as_mut().ok_or_else(||host_error("Native filesystem is not mounted"))?
                .sync_saves().map_err(|error|host_error(format!("Native save sync failed: {error:#}")))?;
        }
        // The pinned guest memory is non-shared and has one runtime owner.
        // Legacy i64 returns split the high half through guest setTempRet0.
        "env.___atomic_fetch_add_8"=>{
            let pointer=value_i32(args,0)? as u32 as usize;
            let low=read_u32(&mut caller,pointer)?;
            let high=read_u32(&mut caller,pointer+4)?;
            let value=(value_i32(args,1)? as u32 as u64)|((value_i32(args,2)? as u32 as u64)<<32);
            let previous=(low as u64)|((high as u64)<<32);
            let set_high=if let Some(f)=caller.data().set_temp_ret.clone() { f } else {
                let f=caller.get_export("setTempRet0").and_then(|value|value.into_func())
                    .ok_or_else(||host_error("Guest setTempRet0 missing for legacy i64 return"))?.typed::<i32,()>(&*caller)?;
                let f=std::rc::Rc::new(f);caller.data_mut().set_temp_ret=Some(f.clone());f
            };
            let memory=caller.data().memory.unwrap();
            memory.write(&mut caller,pointer,&previous.wrapping_add(value).to_le_bytes()).map_err(|error|host_error(error.to_string()))?;
            set_high.call(&mut caller,high as i32)?;
            results[0]=Val::I32(low as i32);
        }
        "env.___atomic_compare_exchange_8"=>{
            let pointer=value_i32(args,0)? as u32 as usize;
            let expected=value_i32(args,1)? as u32 as usize;
            let low=read_u32(&mut caller,pointer)?;let high=read_u32(&mut caller,pointer+4)?;
            let expected_low=read_u32(&mut caller,expected)?;let expected_high=read_u32(&mut caller,expected+4)?;
            let matches=low==expected_low && high==expected_high;
            let (destination,new_low,new_high)=if matches {
                (pointer,value_i32(args,2)? as u32,value_i32(args,3)? as u32)
            }else{(expected,low,high)};
            let bytes=((new_low as u64)|((new_high as u64)<<32)).to_le_bytes();
            caller.data().memory.unwrap().write(&mut caller,destination,&bytes).map_err(|error|host_error(error.to_string()))?;
            results[0]=Val::I32(matches as i32);
        }
        // The pinned framework's clock is elapsed milliseconds * 1000,
        // starting at its first call and wrapping through JS's signed i32.
        "env._clock"=>{
            let ticks=if let Some(start)=caller.data().clock_started {
                (start.elapsed().as_millis().wrapping_mul(1000) as u32) as i32
            }else{caller.data_mut().clock_started=Some(Instant::now());0};
            results[0]=Val::I32(ticks);
        }
        "env._JS_SystemInfo_GetMemory"=>results[0]=Val::I32((caller.data().memory.unwrap().data_size(&caller)/(1024*1024)) as i32),
        "env._JS_SystemInfo_GetOS"=>results[0]=Val::I32(host_string(&mut caller,args,std::env::consts::OS)?),
        "env._JS_SystemInfo_GetBrowserName"=>results[0]=Val::I32(host_string(&mut caller,args,"Ravenfield Native Host")?),
        "env._JS_SystemInfo_GetBrowserVersionString"=>results[0]=Val::I32(host_string(&mut caller,args,"0.4 / Wasmtime 49")?),
        // Unity's native/offline host has no browser document URL.
        "env._JS_SystemInfo_GetDocumentURL"=>results[0]=Val::I32(host_string(&mut caller,args,"")?),
        // This host chooses a one-to-one logical/render pixel ratio.
        "env._JS_SystemInfo_GetPreferredDevicePixelRatio"=>results[0]=Val::F64(1.0f64.to_bits()),
        "env._getpagesize"=>results[0]=Val::I32(16384),
        "env.___syscall5"=>{
            let argv=value_i32(args,1)? as u32 as usize;
            let pointer=read_u32(&mut caller,argv)? as usize;
            let path=string_at(&mut caller,pointer)?;
            let flags=read_u32(&mut caller,argv+4)?;
            caller.data_mut().unsupported=Some(format!("filesystem open: {path}"));
            return Err(host_error(format!("Native filesystem is not mounted: open({path:?}, flags={flags})")));
        }
        "env.___syscall192"=>{
            let argv=value_i32(args,1)? as u32 as usize;
            let length=read_u32(&mut caller,argv+4)? as usize;
            let prot=read_u32(&mut caller,argv+8)?;
            let flags=read_u32(&mut caller,argv+12)?;
            let fd=read_u32(&mut caller,argv+16)? as i32;
            let offset=(read_u32(&mut caller,argv+20)? as u64)*4096;
            if length==0 {results[0]=Val::I32(-22);return Ok(());}
            if fd!=-1 {
                let Some(fs)=caller.data().filesystem.as_ref() else {
                    caller.data_mut().unsupported=Some("file-backed mmap".into());
                    return Err(host_error("File-backed mmap needs native filesystem integration"));
                };
                if let Err(errno)=fs.check_mapping(fd,offset,length,prot,flags) {results[0]=Val::I32(-errno);return Ok(());}
            }
            let allocate=caller.get_export("_memalign").and_then(|value|value.into_func()).ok_or_else(||host_error("Guest memalign export missing"))?;
            let mut pointer=[Val::I32(0)];
            allocate.call(&mut caller,&[Val::I32(16384),Val::I32(length as i32)],&mut pointer)?;
            let ptr=pointer[0].i32().unwrap() as u32 as usize;
            if ptr==0 {results[0]=Val::I32(-12);}
            else {
                let memory=caller.data().memory.unwrap();
                let end=ptr.checked_add(length).ok_or_else(||host_error("mmap length overflow"))?;
                let (data,host)=memory.data_and_store_mut(&mut caller);
                let bytes=data.get_mut(ptr..end).ok_or_else(||host_error("mmap outside guest heap"))?;
                bytes.fill(0);
                let copied=if fd!=-1 {
                    let fs=host.filesystem.as_mut().unwrap();let mut position=0;let mut error=None;
                    while position<length {match fs.pread(fd,offset+position as u64,&mut bytes[position..]){
                        Ok(0)=>break,Ok(count)=>position+=count,Err(errno)=>{error=Some(errno);break;}
                    }}
                    error
                }else{None};
                if let Some(errno)=copied {
                    let free=caller.get_export("_free").and_then(|value|value.into_func()).ok_or_else(||host_error("Guest free export missing"))?;
                    free.call(&mut caller,&[Val::I32(ptr as i32)],&mut [])?;results[0]=Val::I32(-errno);
                }else{caller.data_mut().mappings.insert(ptr,length);results[0]=Val::I32(ptr as i32);}
            }
        }
        "env.___syscall91"=>{
            let argv=value_i32(args,1)? as u32 as usize;
            let pointer=read_u32(&mut caller,argv)? as usize;
            let length=read_u32(&mut caller,argv+4)? as usize;
            if caller.data().mappings.get(&pointer)==Some(&length) {
                let free=caller.get_export("_free").and_then(|value|value.into_func()).ok_or_else(||host_error("Guest free export missing"))?;
                free.call(&mut caller,&[Val::I32(pointer as i32)],&mut [])?;
                caller.data_mut().mappings.remove(&pointer);
            }
            results[0]=Val::I32(0);
        }
        // These are explicitly no-ops in this non-shared-memory Emscripten build.
        "env.___lock"|"env.___unlock"=>{},
        "env.___setErrNo"=>set_errno(&mut caller,value_i32(args,0)?)?,
        "env._pthread_key_create"=>{
            if value_i32(args,0)?==0 {results[0]=Val::I32(22);return Ok(());}
            let key=caller.data().next_tls.checked_add(1).ok_or_else(||host_error("TLS key overflow"))?;
            write_i32(&mut caller,value_i32(args,0)? as u32 as usize,key as i32)?;
            caller.data_mut().next_tls=key;caller.data_mut().tls.insert(key,0);
            results[0]=Val::I32(0);
        }
        "env._pthread_key_delete"=>{
            let key=value_i32(args,0)? as u32;
            caller.data_mut().tls.remove(&key);results[0]=Val::I32(0);
        }
        "env._pthread_getspecific"=>results[0]=Val::I32(*caller.data().tls.get(&(value_i32(args,0)? as u32)).unwrap_or(&0) as i32),
        "env._pthread_setspecific"=>{
            let key=value_i32(args,0)? as u32;let value=value_i32(args,1)? as u32;
            results[0]=Val::I32(if let Some(slot)=caller.data_mut().tls.get_mut(&key){*slot=value;0}else{22});
        }
        "env._pthread_once"=>{
            let flag=value_i32(args,0)? as u32 as usize;
            if !caller.data().once.contains(&flag) {
                let function=caller.get_export("dynCall_v").and_then(|value|value.into_func()).ok_or_else(||host_error("Missing dynCall_v"))?;
                function.call(&mut caller,&[args[1]],&mut [])?;caller.data_mut().once.insert(flag);
            }
            results[0]=Val::I32(0);
        }
        "env._gettimeofday"=>{
            let now=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error|host_error(error.to_string()))?;
            let pointer=value_i32(args,0)? as u32 as usize;
            write_i32(&mut caller,pointer,now.as_secs() as i32)?;
            write_i32(&mut caller,pointer+4,now.subsec_micros() as i32)?;
            results[0]=Val::I32(0);
        }
        "env._clock_getres"|"env._clock_gettime"=>{
            let clock=value_i32(args,0)?;let pointer=value_i32(args,1)? as u32 as usize;
            if !matches!(clock,0|1){set_errno(&mut caller,22)?;results[0]=Val::I32(-1);return Ok(());}
            let (seconds,nanos)=if name=="env._clock_getres"{(0,if clock==0{1_000_000}else{1000})}else if clock==0{
                let millis=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error|host_error(error.to_string()))?.as_millis();
                ((millis/1000) as i32,((millis%1000)*1_000_000) as i32)
            }else{
                let micros=caller.data().started.unwrap().elapsed().as_micros();
                ((micros/1_000_000) as i32,((micros%1_000_000)*1000) as i32)
            };
            let bytes=[seconds.to_le_bytes(),nanos.to_le_bytes()].concat();
            caller.data().memory.unwrap().write(&mut caller,pointer,&bytes).map_err(|error|host_error(error.to_string()))?;results[0]=Val::I32(0);
        }
        "env._time"=>{
            let value=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error|host_error(error.to_string()))?.as_secs() as i32;
            let pointer=value_i32(args,0)? as u32 as usize;if pointer!=0 {write_i32(&mut caller,pointer,value)?;}
            results[0]=Val::I32(value);
        }
        "env._difftime"=>results[0]=Val::F64(((value_i32(args,0)? as f64)-(value_i32(args,1)? as f64)).to_bits()),
        "global.Math.pow" => {
            results[0] = Val::F64(args[0].f64().unwrap().powf(args[1].f64().unwrap()).to_bits());
        }
        "asm2wasm.f64-rem"=>results[0]=Val::F64((args[0].f64().unwrap()%args[1].f64().unwrap()).to_bits()),
        "env._llvm_log2_f32"|"env._llvm_log10_f32"|"env._llvm_log10_f64"|"env._llvm_exp2_f32"|
        "env._llvm_sin_f32"|"env._llvm_cos_f32"|"env._llvm_trunc_f32"=>{
            // The pinned JS ABI lowers these f32 intrinsics through f64 Math
            // functions. Rounding back to f32 belongs to the guest call site.
            let value=args[0].f64().ok_or_else(||host_error("Expected math f64 argument"))?;
            let value=match name{
                "env._llvm_log2_f32"=>value.ln()/std::f64::consts::LN_2,
                "env._llvm_log10_f32"|"env._llvm_log10_f64"=>value.ln()/std::f64::consts::LN_10,
                "env._llvm_exp2_f32"=>2f64.powf(value),"env._llvm_sin_f32"=>value.sin(),
                "env._llvm_cos_f32"=>value.cos(),_=>value.trunc(),
            };results[0]=Val::F64(value.to_bits());
        }
        "env._llvm_eh_typeid_for"=>results[0]=Val::I32(value_i32(args,0)?),
        "env._llvm_trap"=>return Err(host_error("Guest LLVM trap")),
        "env.getTotalMemory" => {
            results[0] = Val::I32(caller.data().memory.unwrap().data_size(&caller) as i32);
        }
        "env.enlargeMemory" => {
            let requested = read_u32(&mut caller, DYNAMIC_TOP_PTR)? as usize;
            let memory = caller.data().memory.unwrap();
            let current = memory.data_size(&caller);
            if requested > MAX_HEAP_BYTES { results[0] = Val::I32(0); }
            else {
                let target = requested.max(current.saturating_mul(2).min(MAX_HEAP_BYTES));
                let pages = target.div_ceil(65536);
                results[0] = Val::I32(if memory.grow(&mut caller, (pages-current/65536) as u64).is_ok() {1} else {0});
            }
        }
        "env._emscripten_memcpy_big" => {
            let dest=value_i32(args,0)? as u32 as usize;
            let source=value_i32(args,1)? as u32 as usize;
            let count=value_i32(args,2)? as u32 as usize;
            let memory=caller.data().memory.unwrap();
            let data=memory.data_mut(&mut caller);
            let end=source.checked_add(count).ok_or_else(||host_error("memcpy overflow"))?;
            if end>data.len() || dest.checked_add(count).is_none_or(|end|end>data.len()) {return Err(host_error("memcpy out of bounds"));}
            data.copy_within(source..end,dest);
            if !results.is_empty() {results[0]=Val::I32(dest as i32);}
        }
        "env._emscripten_get_now" => results[0]=Val::F64((caller.data().started.unwrap().elapsed().as_micros() as f64/1000.0).to_bits()),
        "env._JS_Log_Dump" => {
            let message=string_at(&mut caller,value_i32(args,0)? as u32 as usize)?;
            let logs=&mut caller.data_mut().logs;
            if logs.len()<100 {logs.push(message);}
        }
        "env.abort"|"env._abort"|"env.abortOnCannotGrowMemory" => return Err(host_error(format!("Unity aborted: {name} args={args:?}"))),
        _ => {
            caller.data_mut().unsupported=Some(name.to_string());
            return Err(host_error(format!("Unsupported Unity/Emscripten host service: {name} args={args:?}")));
        }
    }
    Ok(())
}

fn initialize(engine: &Engine, module: &Module) -> Result<(Store<Host>, Instance)> {
    initialize_guarded(engine,module,false)
}
fn initialize_guarded(engine:&Engine,module:&Module,epoch_guard:bool)->Result<(Store<Host>,Instance)>{
    let mut store=Store::new(engine,Host {started:Some(Instant::now()),epoch_guard,..Host::default()});
    if epoch_guard{store.set_epoch_deadline(6000);}else{store.set_fuel(2_000_000_000).map_err(anyhow::Error::from)?;}
    let mut linker=Linker::new(engine);
    for import in module.imports() {
        let (space,name)=(import.module(),import.name());
        match import.ty() {
            ExternType::Memory(kind) => {
                anyhow::ensure!(!kind.is_shared(),"Pinned native host supports only non-shared guest memory");
                let memory=Memory::new(&mut store,kind).map_err(anyhow::Error::from)?;
                memory.write(&mut store,DYNAMIC_TOP_PTR,&(STACK_MAX as u32).to_le_bytes()).map_err(anyhow::Error::from)?;
                store.data_mut().memory=Some(memory);
                linker.define(&mut store,space,name,memory).map_err(anyhow::Error::from)?;
            }
            ExternType::Table(kind) => {
                let table=Table::new(&mut store,kind,Ref::Func(None)).map_err(anyhow::Error::from)?;
                store.data_mut().table=Some(table);
                linker.define(&mut store,space,name,table).map_err(anyhow::Error::from)?;
            }
            ExternType::Global(kind) => {
                let value=match (space,name) {
                    ("env","tableBase")=>Val::I32(0),
                    ("env","DYNAMICTOP_PTR")=>Val::I32(DYNAMIC_TOP_PTR as i32),
                    ("env","STACKTOP")=>Val::I32(STACK_BASE),
                    ("env","STACK_MAX")=>Val::I32(STACK_MAX),
                    ("global","NaN")=>Val::F64(f64::NAN.to_bits()),
                    ("global","Infinity")=>Val::F64(f64::INFINITY.to_bits()),
                    _=>anyhow::bail!("Unsupported imported global {space}.{name}"),
                };
                let global=Global::new(&mut store,kind,value).map_err(anyhow::Error::from)?;
                linker.define(&mut store,space,name,global).map_err(anyhow::Error::from)?;
            }
            ExternType::Func(kind) => {
                let full=format!("{space}.{name}");
                let counter_slot=store.data().calls.len();
                store.data_mut().calls.push((full.clone(),0));
                // Names and ABI routing are immutable for this instance. Avoid
                // allocating a dynCall name on each of millions of invokes.
                let trampoline=full.strip_prefix("env.invoke_").map(|suffix|format!("dynCall_{suffix}"));
                let slot=store.data().exceptions.invokes.len();
                if trampoline.is_some(){store.data_mut().exceptions.invokes.push(None);}
                if let Some(trampoline)=&trampoline{
                    if exceptions::bind_typed_import(&mut linker,space,name,trampoline,slot,counter_slot).map_err(anyhow::Error::from)?{continue;}
                }
                if services::bind(&mut linker,space,name,counter_slot).map_err(anyhow::Error::from)?{continue;}
                linker.func_new(space,name,kind,move |mut caller,args,results|{
                    // The binding owns its counter slot for this Store. No
                    // String allocation or name lookup on the hot path.
                    caller.data_mut().calls[counter_slot].1+=1;
                    let clock=(trampoline.is_none()&&caller.data().profile_imports).then(Instant::now);
                    let result=dispatch(&mut caller,&full,trampoline.as_deref().map(|name|(name,slot)),args,results);
                    if let Some(clock)=clock{let elapsed=clock.elapsed().as_nanos();let timings=&mut caller.data_mut().import_timings;
                        if let Some((count,time))=timings.get_mut(&full){*count+=1;*time+=elapsed;}else{timings.insert(full.clone(),(1,elapsed));}}
                    result
                }).map_err(anyhow::Error::from)?;
            }
            _=>anyhow::bail!("Unsupported import kind: {space}.{name}"),
        }
    }
    let instance=linker.instantiate(&mut store,module).map_err(anyhow::Error::from)?;
    Ok((store,instance))
}

fn constructors(framework: &str) -> Result<Vec<String>> {
    let tail=framework.split("__ATINIT__.push(").nth(1).context("Constructor list absent")?;
    let body=tail.split(");").next().context("Constructor list unterminated")?;
    let mut names=Vec::new();
    let mut rest=body;
    while !rest.is_empty() {
        rest=rest.strip_prefix("{func:(function(){").context("Unexpected constructor callback syntax")?;
        let (name,tail)=rest.split_once("()})}").context("Invalid constructor callback")?;
        anyhow::ensure!(!name.is_empty()&&name.bytes().all(|byte|byte.is_ascii_alphanumeric()||byte==b'_'),"Invalid constructor export name");
        names.push(name.to_string());
        rest=if tail.is_empty(){tail}else{tail.strip_prefix(',').context("Missing constructor callback separator")?};
    }
    anyhow::ensure!(!names.is_empty(),"No constructors found");
    Ok(names)
}

pub fn probe(path: &str, framework: Option<&str>) -> Result<Report> {
    probe_impl(path,framework,false,&diagnostics::Progress::disabled(),None,ProbeOptions::default())
}

/// Compile locally authorized source WASM and mount its reviewed data for a desktop probe.
pub fn probe_with_data(path: &str, framework: Option<&str>, data: &str) -> Result<Report> {
    probe_impl(path,framework,false,&diagnostics::Progress::cli(),Some(data),ProbeOptions::default())
}

pub fn probe_source_with_options(path: &str, framework: Option<&str>, data: Option<&str>, options: ProbeOptions) -> Result<Report> {
    probe_impl(path,framework,false,&diagnostics::Progress::cli(),data,options)
}

/// # Safety
/// The artifact must be locally produced or verified from the signed Lab APK.
pub unsafe fn probe_trusted_with_options(path: &str, framework: Option<&str>, data: Option<&str>, options: ProbeOptions) -> Result<Report> {
    probe_impl(path,framework,true,&diagnostics::Progress::cli(),data,options)
}

/// Execute only an authentic, locally produced/signed-APK native artifact.
/// # Safety
/// Deserialized artifacts are native executable code; never accept untrusted input.
pub unsafe fn smoke_trusted_artifact(path: &str) -> Result<serde_json::Value> {
    Ok(serde_json::to_value(diagnostics::smoke(&diagnostics::Progress::cli(), Some(path))?)?)
}

fn engine_config() -> Config {
    engine_config_profile(false)
}
fn engine_config_profile(guest_eh:bool) -> Config {
    engine_config_codegen(guest_eh,false)
}
fn engine_config_codegen(guest_eh:bool,inline_eager:bool) -> Config {
    let mut config = Config::new();
    config.consume_fuel(true).max_wasm_stack(2 * 1024 * 1024);
    if inline_eager{config.compiler_inlining(wasmtime::Inlining::Yes).table_lazy_init(false);}
    // Android avoids costly libunwind registration for the large Unity module.
    // Windows requires native unwind info for its exception/trap machinery.
    // Wasmtime's own WASM backtraces remain available on both platforms.
    config.native_unwind_info(cfg!(windows));
    #[cfg(feature="guest-eh-prototype")]
    if guest_eh{
        config.wasm_gc(false).wasm_function_references(false).wasm_exceptions(true).gc_support(true);
    }else{
        // Wasmtime49 defaults the proposal bits on even without GC_TYPES.
        // Preserve those defaults for existing 0.17 AOT compatibility.
        config.wasm_exceptions(false).gc_support(false);
    }
    #[cfg(not(feature="guest-eh-prototype"))]let _=guest_eh;
    #[cfg(feature = "compiler")]
    config.parallel_compilation(false).cranelift_opt_level(wasmtime::OptLevel::None);
    config
}
fn engine_config_execution(guest_eh:bool,inline_eager:bool,epoch_guard:bool)->Config{
    let mut config=engine_config_codegen(guest_eh,inline_eager);
    if epoch_guard{config.consume_fuel(false).epoch_interruption(true);}
    config
}

#[cfg(feature = "aot-builder")]
pub fn precompile_for_target(source: &str, target: &str, output: &str) -> Result<()> {
    precompile_for_target_optimized(source,target,output,false)
}

#[cfg(feature = "aot-builder")]
pub fn precompile_for_target_optimized(source: &str, target: &str, output: &str, optimized:bool) -> Result<()> {
    precompile_for_target_profile(source,target,output,optimized,false,false)
}
#[cfg(feature = "aot-builder")]
pub fn precompile_for_target_profile(source:&str,target:&str,output:&str,optimized:bool,guest_eh:bool,parallel:bool)->Result<()>{
    precompile_for_target_codegen(source,target,output,optimized,guest_eh,parallel,false)
}
#[cfg(feature = "aot-builder")]
pub fn precompile_for_target_codegen(source:&str,target:&str,output:&str,optimized:bool,guest_eh:bool,parallel:bool,inline_eager:bool)->Result<()>{
    precompile_for_target_guarded(source,target,output,optimized,guest_eh,parallel,inline_eager,false)
}
#[cfg(feature = "aot-builder")]
pub fn precompile_for_target_guarded(source:&str,target:&str,output:&str,optimized:bool,guest_eh:bool,parallel:bool,inline_eager:bool,epoch_guard:bool)->Result<()>{
    anyhow::ensure!(!guest_eh||cfg!(feature="guest-eh-prototype"),"Build with guest-eh-prototype for this experiment");
    anyhow::ensure!(!inline_eager||!epoch_guard,"Do not combine the epoch trial with the failed inlining experiment");
    let mut config = engine_config_execution(guest_eh,inline_eager,epoch_guard);
    config.parallel_compilation(parallel);
    config.cranelift_opt_level(if optimized {wasmtime::OptLevel::Speed}else{wasmtime::OptLevel::None});
    config.target(target).map_err(anyhow::Error::from)?;
    // Cross-compilation must follow the artifact's OS, not the builder's OS.
    config.native_unwind_info(target.split('-').any(|part| part == "windows"));
    let engine = Engine::new(&config).map_err(anyhow::Error::from)?;
    let wasm = std::fs::read(source)?;
    anyhow::ensure!(wasm.starts_with(b"\0asm\x01\0\0\0"), "Input is not a WASM binary");
    let started = Instant::now();
    eprintln!("AOT begin target={target} opt_level={} source_bytes={} guest_eh={guest_eh} parallel={parallel} inline_eager={inline_eager} epoch_guard={epoch_guard}",if optimized {"speed"}else{"none"}, wasm.len());
    let native = engine.precompile_module(&wasm).map_err(anyhow::Error::from)?;
    std::fs::write(output, &native)?;
    eprintln!("AOT complete target={target} native_bytes={} elapsed_ms={}", native.len(), started.elapsed().as_millis());
    Ok(())
}

/// Only for artifacts produced locally by this build's Wasmtime compiler.
/// Never expose serialized native code loading through the Android file picker.
/// # Safety
/// The caller must establish that `path` is an authentic Wasmtime artifact.
pub unsafe fn probe_trusted_artifact(path: &str, framework: Option<&str>) -> Result<Report> {
    probe_impl(path,framework,true,&diagnostics::Progress::cli(),None,ProbeOptions::default())
}

/// # Safety
/// `path` must be an authentic local/signed-APK native artifact. `data` is a
/// locally authorized UnityWebData file, mounted only into a virtual read-only namespace.
pub unsafe fn probe_trusted_artifact_with_data(path:&str, framework:Option<&str>, data:&str)->Result<Report>{
    probe_impl(path,framework,true,&diagnostics::Progress::cli(),Some(data),ProbeOptions::default())
}

fn probe_impl(path: &str, framework: Option<&str>, compiled: bool, progress: &diagnostics::Progress, data:Option<&str>, options:ProbeOptions) -> Result<Report> {
    anyhow::ensure!(!options.guest_eh||cfg!(feature="guest-eh-prototype"),"Guest catch diagnostic requires guest-eh-prototype build");
    anyhow::ensure!(!options.android_gles||cfg!(target_os="android"),"Android GLES validation requires Android");
    let native_surface=(cfg!(windows)&&options.desktop_gles)||(cfg!(target_os="android")&&options.android_gles);
    let mut game=Vec::new();
    anyhow::ensure!(options.capture_frames.len()<=12,"At most 12 diagnostic captures");
    for (frame,_) in &options.capture_frames{anyhow::ensure!(native_surface&&*frame>0&&*frame<=options.frame_limit,"Invalid capture checkpoint");}
    for &(frame,kind,a,b) in &options.game_sequence{
        let event=session::GameEvent::from_wire(kind,a,b).context("Invalid scripted game event")?;
        anyhow::ensure!(native_surface&&frame>0&&frame<options.frame_limit,"Scripted game input requires a native surface and valid frame");game.push((frame,event));
    }
    let mut taps=options.tap_sequence.clone();if let Some(frame)=options.tap_at_frame{taps.push((frame,options.tap_x,options.tap_y));}taps.sort_by_key(|tap|tap.0);
    for &(frame,x,y) in &taps{anyhow::ensure!(native_surface&&frame>0&&frame.checked_add(1).is_some_and(|release|release<options.frame_limit)&&(0..1280).contains(&x)&&(0..720).contains(&y),"Invalid scripted pointer tap");}
    anyhow::ensure!(taps.windows(2).all(|pair|pair[1].0-pair[0].0>1),"Scripted pointer taps must leave a release frame");
    anyhow::ensure!(options.frame_limit<=7200,"Diagnostic frame limit cannot exceed 7200");
    anyhow::ensure!(!options.desktop_keyboard||options.desktop_gles,"Desktop keyboard requires its native GLES window");
    if let Some(frame)=options.space_at_frame{anyhow::ensure!(options.desktop_keyboard&&frame>0&&frame.checked_add(1).is_some_and(|release|release<options.frame_limit),"Scripted Space requires keyboard and two later loop iterations");}
    if let Some(frame)=options.tap_at_frame{anyhow::ensure!(native_surface&&frame>0&&frame.checked_add(1).is_some_and(|release|release<options.frame_limit)&&(0..1280).contains(&options.tap_x)&&(0..720).contains(&options.tap_y),"Scripted tap requires a native surface, valid coordinates and later frames");}
    #[cfg(not(windows))]anyhow::ensure!(!options.desktop_keyboard,"Desktop keyboard requires Windows");
    anyhow::ensure!(options.startup_fuel.unwrap_or(2_000_000_000)<=200_000_000_000,"Startup fuel exceeds diagnostic limit");
    anyhow::ensure!(options.frame_fuel.unwrap_or(2_000_000_000)<=200_000_000_000,"Frame fuel exceeds diagnostic limit");
    anyhow::ensure!(options.save_state.is_none() || data.is_some(),"Save storage requires a mounted UnityWebData container");
    #[cfg(not(windows))]
    anyhow::ensure!(!options.desktop_gles,"Desktop GLES validation requires Windows");
    progress.record("engine-create-enter");
    // This bounded ABI diagnostic is deliberately not a performance build.
    let config=engine_config_execution(options.guest_eh,options.inline_eager_codegen,options.epoch_guard);
    progress.record(&format!("inline-eager-codegen={}",options.inline_eager_codegen));
    let engine=Engine::new(&config).map_err(anyhow::Error::from)?;
    progress.record("engine-create-complete");
    let clock=Instant::now();
    progress.record(if compiled {"artifact-deserialize-enter"} else {"wasm-compile-enter"});
    let module=if compiled {
        // This path is used only by the explicit trusted-artifact CLI command.
        unsafe { Module::deserialize_file(&engine,path) }.map_err(anyhow::Error::from)?
    } else {
        #[cfg(feature = "compiler")]
        { Module::from_file(&engine,path).map_err(anyhow::Error::from)? }
        #[cfg(not(feature = "compiler"))]
        { anyhow::bail!("Source compilation is excluded from this AOT-only runtime") }
    };
    let load_ms=clock.elapsed().as_millis();
    progress.record(&format!("module-ready elapsed_ms={load_ms}"));
    let clock=Instant::now();
    progress.record("instantiate-enter");
    let _watchdog=options.epoch_guard.then(||execution_guard::Watchdog::start(&engine)).transpose()?;
    let (mut store,instance)=initialize_guarded(&engine,&module,options.epoch_guard)?;
    store.data_mut().headless_events=options.headless_events;
    store.data_mut().scripted_game=game;
    store.data_mut().capture_frames=options.capture_frames.iter().cloned().collect();
    store.data_mut().profile_imports=options.profile_imports;
    store.data_mut().trace_gl_calls=options.trace_gl_calls;
    if !options.epoch_guard{if let Some(fuel)=options.startup_fuel{store.set_fuel(fuel).map_err(anyhow::Error::from)?;}}
    store.data_mut().headless_audio=options.headless_audio;
    if options.native_audio||options.audio_wav.is_some(){store.data_mut().audio=Some(audio::State::new(options.audio_wav.as_deref())?);}
    #[cfg(windows)]
    if options.desktop_gles {store.data_mut().graphics=Some(graphics::Graphics::offscreen(1280,720)?);}
    store.data_mut().input.enabled=options.desktop_keyboard||options.control.is_some()||!options.game_sequence.is_empty();
    store.data_mut().control=options.control.clone().or_else(||(!taps.is_empty()||!options.game_sequence.is_empty()).then(session::Control::new));
    store.data_mut().pointer.enabled=store.data().control.is_some();
    if let Some(control)=store.data().control.clone(){if let Some(audio)=store.data_mut().audio.as_mut(){audio.muted=control.audio_muted.clone();}}
    #[cfg(target_os="android")]
    if options.android_gles {
        store.data_mut().graphics=Some(if let Some(window)=options.android_window.clone(){graphics::Graphics::window(window,1280,720)?}else{graphics::Graphics::offscreen(1280,720)?});
        store.data_mut().live=options.android_window.is_some();
    }
    if let Some(path)=data {
        progress.record("filesystem-mount-enter");
        let mut filesystem=filesystem::AssetFs::mount(path)?;
        if let Some(path)=&options.save_state {filesystem.mount_saves(path)?;}
        store.data_mut().filesystem=Some(filesystem);
        progress.record("filesystem-mounted");
    }
    progress.record("instantiate-complete");
    let mut report=Report {runtime:"Wasmtime 49 / native host",architecture:std::env::consts::ARCH, diagnostic_version:if options.epoch_guard{"0.19-guest-epoch"}else if options.guest_eh{"0.18-guest-eh"}else{"0.17-guest-intrinsics"}, compiler_included:cfg!(feature = "compiler"),guest_exception_boundaries:options.guest_eh,inline_eager_codegen:options.inline_eager_codegen,
        execution_guard:if options.epoch_guard{"epoch-10ms-startup60s-callback30s"}else{"fuel"},
        compile_ms:if compiled {0} else {load_ms},deserialize_ms:if compiled {load_ms} else {0},
        instantiate_ms:clock.elapsed().as_millis(),imports:module.imports().len(),
        memory_bytes:0,stage:"linked".into(),completed_constructors:0,first_blocker:None,
        allocator_roundtrip:false,startup_ms:0,main_loop_work_ms:0,max_frame_work_ms:0,stack_global_fast_path:false,performance:serde_json::Value::Null,import_timings:serde_json::Value::Null,host_calls:BTreeMap::new(),logs:Vec::new(),playable:false,rendered_game_frame:false,filesystem:None,
        headless_events:options.headless_events,headless_audio:options.headless_audio,unavailable_services:Vec::new(),graphics:None,audio:None,main_loop_frames:0,input:serde_json::Value::Null,scripted_space_frame:options.space_at_frame,pointer:serde_json::Value::Null,live:None};
    let startup_clock=Instant::now();let mut startup_done=false;
    let result:Result<()>= (|| {
        if store.data().control.as_ref().is_some_and(|control|control.stopped()){report.stage="live-session-stopped".into();return Ok(());}
        let constructor_names=if let Some(framework)=framework {constructors(framework)?}
            else {serde_json::from_str::<serde_json::Value>(include_str!("../build-profile.json"))?["constructors"]
                .as_array().context("Build profile constructors missing")?.iter()
                .map(|value|value.as_str().unwrap().to_string()).collect()};
        {
            progress.record("constructors-enter");
            for name in constructor_names {
                if store.data().control.as_ref().is_some_and(|control|control.stopped()){report.stage="live-session-stopped".into();return Ok(());}
                instance.get_typed_func::<(),()>(&mut store,&name).map_err(anyhow::Error::from)?
                    .call(&mut store,()).map_err(anyhow::Error::from)?;
                report.completed_constructors+=1;
                if report.completed_constructors % 25 == 0 {
                    progress.record(&format!("constructors-completed={}",report.completed_constructors));
                }
            }
            report.stage="static-constructors-complete".into();
            progress.record(&format!("constructors-complete count={}",report.completed_constructors));
        }
        let malloc=instance.get_typed_func::<i32,i32>(&mut store,"_malloc").map_err(anyhow::Error::from)?;
        let free=instance.get_typed_func::<i32,()>(&mut store,"_free").map_err(anyhow::Error::from)?;
        let pointer=malloc.call(&mut store,64).map_err(anyhow::Error::from)?;
        anyhow::ensure!(pointer>0,"Allocator returned null");
        let memory=store.data().memory.unwrap();
        let pattern=b"Ravenfield native WASM memory roundtrip";
        memory.write(&mut store,pointer as usize,pattern).map_err(anyhow::Error::from)?;
        let mut back=vec![0;pattern.len()];memory.read(&store,pointer as usize,&mut back).map_err(anyhow::Error::from)?;
        anyhow::ensure!(back==pattern,"Memory roundtrip mismatch");
        free.call(&mut store,pointer).map_err(anyhow::Error::from)?;
        report.allocator_roundtrip=true;report.stage="allocator-verified".into();
        progress.record("allocator-verified");
        // _main must stop at the first host service that lacks real semantics.
        // Never return zero for an absent renderer, filesystem, audio, or input.
        let main=instance.get_typed_func::<(i32,i32),i32>(&mut store,"_main").map_err(anyhow::Error::from)?;
        let program=b"./ravenfield-native\0";
        let name_pointer=malloc.call(&mut store,program.len() as i32).map_err(anyhow::Error::from)?;
        let argv_pointer=malloc.call(&mut store,8).map_err(anyhow::Error::from)?;
        anyhow::ensure!(name_pointer>0&&argv_pointer>0,"argv allocation failed");
        memory.write(&mut store,name_pointer as usize,program).map_err(anyhow::Error::from)?;
        memory.write(&mut store,argv_pointer as usize,&name_pointer.to_le_bytes()).map_err(anyhow::Error::from)?;
        memory.write(&mut store,argv_pointer as usize+4,&0_i32.to_le_bytes()).map_err(anyhow::Error::from)?;
        progress.record("main-enter");
        report.stage="main-entered".into();
        match main.call(&mut store,(1,argv_pointer)){
            Ok(_)=>report.stage="main-returned".into(),
            Err(error) if error.downcast_ref::<scheduler::MainLoopYield>().is_some() && scheduler::active(store.data())=>report.stage="main-loop-registered".into(),
            Err(error)=>return Err(error.into()),
        }
        report.startup_ms=startup_clock.elapsed().as_millis();startup_done=true;
        if (options.frame_limit>0||store.data().live) && scheduler::active(store.data()){
            let limit=if store.data().live{u32::MAX}else{options.frame_limit};
            progress.record("main-loop-enter");scheduler::run(&mut store,&instance,limit,options.frame_fuel.unwrap_or(2_000_000_000),progress,options.trace_guest_lists,options.space_at_frame,&taps)?;
            report.stage=if store.data().live{"live-session-stopped"}else{"main-loop-bounded-complete"}.into();
            if !store.data().live{if let Some(control)=store.data().control.as_ref(){control.stop();}}
        }
        Ok(())
    })();
    if let Err(error)=result {report.first_blocker=Some(format!("{error:#}"));}
    if !startup_done{report.startup_ms=startup_clock.elapsed().as_millis();}
    report.main_loop_work_ms=store.data().scheduler.work_us/1000;report.max_frame_work_ms=store.data().scheduler.max_work_us/1000;
    report.performance=store.data().scheduler.performance.report();
    let import_timings=store.data().import_timings.iter().map(|(name,(calls,ns))|(name,serde_json::json!({"calls":calls,"inclusive_ms":*ns as f64/1_000_000.0}))).collect::<BTreeMap<_,_>>();
    report.import_timings=serde_json::json!({"enabled":options.profile_imports,"measurement":"inclusive service callback wall time; excludes invoke trampolines, includes driver waits and any nested callbacks; instrumentation affects runtime; not GPU execution time; nested callback durations must not be added as exclusive CPU time", "services":import_timings});
    if store.data().live{
        if let Some(filesystem)=store.data_mut().filesystem.as_mut(){if filesystem.report().saves.is_some_and(|saves|saves.dirty){if let Err(error)=filesystem.sync_saves(){report.first_blocker.get_or_insert_with(||format!("Shutdown save sync failed: {error:#}"));}}}
    }
    if let Some(path)=&options.capture{
        let graphics=store.data_mut().graphics.as_mut().context("Capture requires a native graphics backend")?;
        match graphics.capture(path){Ok(rendered)=>report.rendered_game_frame=rendered,Err(error)=>store.data_mut().logs.push(format!("Surface capture failed: {error:#}"))}
    }
    report.memory_bytes=store.data().memory.unwrap().data_size(&store);
    report.stack_global_fast_path=store.data().exceptions.stack_global_active();
    report.host_calls=store.data().calls.iter().filter(|(_,count)|*count!=0).cloned().collect();report.logs=store.data().logs.clone();
    report.filesystem=store.data().filesystem.as_ref().map(|fs|fs.report());
    report.unavailable_services=store.data().unavailable_services.iter().cloned().collect();
    report.audio=store.data_mut().audio.as_mut().map(|audio|audio.report());
    report.graphics=store.data().graphics.as_ref().map(|graphics|graphics.report());
    report.main_loop_frames=store.data().scheduler.frames;
    report.input=input::report(store.data());
    report.pointer=pointer::report(store.data());report.live=store.data().control.as_ref().map(|control|control.status());
    if store.data().live&&store.data().control.as_ref().is_some_and(|control|control.presented.load(std::sync::atomic::Ordering::Relaxed)>0){report.rendered_game_frame=true;}
    progress.record(&format!("probe-returned stage={} blocker={}",report.stage,report.first_blocker.as_deref().unwrap_or("none")));
    progress.checkpoint(&report);
    progress.record("native-resources-release-enter");
    drop(_watchdog);
    drop(store);
    progress.record("native-resources-release-complete");
    Ok(report)
}

#[cfg(all(test, feature = "compiler"))]
mod tests {
    use super::*;
    #[test]
    fn import_timers_are_opt_in_and_preserve_callback_counts(){
        let(mut store,instance)=fixture(r#"(module (import "env" "_emscripten_get_now" (func $now (result f64))) (func (export "run") (result f64) call $now))"#);
        let run=instance.get_typed_func::<(),f64>(&mut store,"run").unwrap();
        assert!(run.call(&mut store,()).unwrap()>=0.0);assert!(store.data().import_timings.is_empty());
        store.data_mut().profile_imports=true;
        run.call(&mut store,()).unwrap();run.call(&mut store,()).unwrap();
        assert_eq!(store.data().import_timings["env._emscripten_get_now"].0,2);
        assert_eq!(store.data().calls.iter().find(|(name,_)|name=="env._emscripten_get_now").unwrap().1,3);
    }
    pub(super) fn fixture(source:&str)->(Store<Host>,Instance){
        let mut config=Config::new();config.consume_fuel(true);
        let engine=Engine::new(&config).unwrap();
        let wasm=wat::parse_str(source).unwrap();
        let module=Module::new(&engine,wasm).unwrap();
        initialize(&engine,&module).unwrap()
    }
    #[test]
    fn startup_callbacks_include_all_export_names_in_framework_order(){
        let source="__ATINIT__.push({func:(function(){__GLOBAL__sub_I_first()})},{func:(function(){___cxx_global_var_init_23()})},{func:(function(){___emscripten_environ_constructor()})},{func:(function(){__GLOBAL__sub_I_last()})});";
        assert_eq!(constructors(source).unwrap(),["__GLOBAL__sub_I_first","___cxx_global_var_init_23","___emscripten_environ_constructor","__GLOBAL__sub_I_last"]);
        assert!(constructors("__ATINIT__.push({func:(function(){unexpected(1)})});").is_err());
        let profile:serde_json::Value=serde_json::from_str(include_str!("../build-profile.json")).unwrap();
        let names=profile["constructors"].as_array().unwrap();
        assert_eq!(names.len(),233);assert_eq!(names[9],"___cxx_global_var_init_23");
        assert_eq!(names.iter().filter(|name|!name.as_str().unwrap().starts_with("__GLOBAL__sub_I_")).count(),16);
    }
    #[test]
    fn pinned_single_thread_compatibility_functions_preserve_guest_memory(){
        let inventory:serde_json::Value=serde_json::from_str(include_str!("../abi-inventory.json")).unwrap();
        for name in SINGLE_THREAD_NOOPS {
            let item=inventory["imports"].as_array().unwrap().iter().find(|item|item["name"]==*name).unwrap();
            let count=item["params"].as_array().unwrap().len();
            let source=format!("(module (import \"env\" \"memory\" (memory 512)) (import \"env\" \"{name}\" (func $call (param {}) (result i32))) (func (export \"test\") (result i32) {} call $call))",vec!["i32";count].join(" "),vec!["i32.const 128";count].join(" "));
            let (mut store,instance)=fixture(&source);let memory=store.data().memory.unwrap();memory.write(&mut store,128,&[0xA5;16]).unwrap();
            assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"test").unwrap().call(&mut store,()).unwrap(),0);
            assert_eq!(&memory.data(&store)[128..144],&[0xA5;16]);
        }
    }
    #[test]
    fn indirect_call_reaches_real_guest_function(){
        let (mut store,instance)=fixture(r#"(module
          (type $increment_type (func (param i32) (result i32)))
          (import "env" "memory" (memory 512))
          (import "env" "table" (table 1 funcref))
          (import "env" "invoke_ii" (func $invoke (param i32 i32) (result i32)))
          (func $increment (type $increment_type) local.get 0 i32.const 1 i32.add)
          (elem (i32.const 0) $increment)
          (func (export "dynCall_ii") (param i32 i32) (result i32) local.get 1 local.get 0 call_indirect (type $increment_type))
          (func (export "test") (result i32) i32.const 0 i32.const 41 call $invoke))"#);
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"test").unwrap().call(&mut store,()).unwrap(),42);
    }
    #[test]
    fn unsupported_graphics_is_a_trap_instead_of_a_fake_frame(){
        let (mut store,instance)=fixture(r#"(module
          (import "env" "memory" (memory 512))
          (import "env" "_glDrawArrays" (func $draw (param i32 i32 i32)))
          (func (export "test") i32.const 4 i32.const 0 i32.const 3 call $draw))"#);
        assert!(instance.get_typed_func::<(),()>(&mut store,"test").unwrap().call(&mut store,()).is_err());
        assert_eq!(store.data().unsupported.as_deref(),Some("env._glDrawArrays"));
        store.data_mut().headless_events=true;
        assert!(instance.get_typed_func::<(),()>(&mut store,"test").unwrap().call(&mut store,()).is_err());
    }
    #[test]
    fn headless_events_return_real_failure_codes_without_changing_default_policy(){
        let inventory:serde_json::Value=serde_json::from_str(include_str!("../abi-inventory.json")).unwrap();
        let mut imports=String::new();let mut exports=String::new();
        for (index,event) in EVENT_CALLBACKS.iter().enumerate() {
            let name=format!("_emscripten_set_{event}_callback_on_thread");
            let entry=inventory["imports"].as_array().unwrap().iter().find(|entry|entry["name"]==name).unwrap();
            let params=entry["params"].as_array().unwrap();
            assert!(params.iter().all(|param|param=="i32"));
            assert_eq!(entry["results"],serde_json::json!(["i32"]));
            imports+=&format!("(import \"env\" \"{name}\" (func $event{index} (param {}) (result i32)))",vec!["i32";params.len()].join(" "));
            exports+=&format!("(func (export \"test{index}\") (result i32) {} call $event{index})",vec!["i32.const 0";params.len()].join(" "));
        }
        let (mut store,instance)=fixture(&format!("(module (import \"env\" \"memory\" (memory 512)) {imports} {exports})"));
        let first=instance.get_typed_func::<(),i32>(&mut store,"test0").unwrap();
        assert!(first.call(&mut store,()).is_err());
        assert!(store.data().unavailable_services.is_empty());
        store.data_mut().headless_events=true;
        for index in 0..EVENT_CALLBACKS.len() {
            assert_eq!(instance.get_typed_func::<(),i32>(&mut store,&format!("test{index}")).unwrap().call(&mut store,()).unwrap(),-1);
        }
        assert_eq!(store.data().unavailable_services.len(),EVENT_CALLBACKS.len());
    }
    #[test]
    fn getenv_uses_a_sealed_empty_guest_environment_and_checks_memory(){
        let (mut store,instance)=fixture(r#"(module
          (import "env" "memory" (memory 512))
          (import "env" "_getenv" (func $getenv (param i32) (result i32)))
          (data (i32.const 32) "PATH\00")
          (func (export "test") (param i32) (result i32) local.get 0 call $getenv))"#);
        let getenv=instance.get_typed_func::<i32,i32>(&mut store,"test").unwrap();
        assert_eq!(getenv.call(&mut store,32).unwrap(),0);
        assert!(getenv.call(&mut store,-1).is_err());
    }
    #[test]
    fn guest_environment_updates_environ_and_preserves_values_after_errors(){
        let (mut store,instance)=fixture(r#"(module
          (import "env" "memory" (memory 512))
          (import "env" "_getenv" (func $get (param i32) (result i32)))
          (import "env" "_setenv" (func $set (param i32 i32 i32) (result i32)))
          (import "env" "_unsetenv" (func $unset (param i32) (result i32)))
          (data (i32.const 32) "TEST\00") (data (i32.const 64) "one\00")
          (data (i32.const 96) "two\00") (data (i32.const 128) "BAD=NAME\00")
          (func (export "__get_environ") (result i32) i32.const 16)
          (func (export "___errno_location") (result i32) i32.const 24)
          (func (export "_malloc") (param i32) (result i32) i32.const 1024)
          (func (export "get") (param i32) (result i32) local.get 0 call $get)
          (func (export "set") (param i32 i32 i32) (result i32) local.get 0 local.get 1 local.get 2 call $set)
          (func (export "unset") (param i32) (result i32) local.get 0 call $unset))"#);
        let memory=store.data().memory.unwrap();
        let get=instance.get_typed_func::<i32,i32>(&mut store,"get").unwrap();
        let set=instance.get_typed_func::<(i32,i32,i32),i32>(&mut store,"set").unwrap();
        let unset=instance.get_typed_func::<i32,i32>(&mut store,"unset").unwrap();
        assert_eq!(get.call(&mut store,32).unwrap(),0);
        assert_eq!(set.call(&mut store,(32,64,1)).unwrap(),0);
        let pointer=get.call(&mut store,32).unwrap() as usize;
        assert_eq!(&memory.data(&store)[pointer..pointer+4],b"one\0");
        assert_eq!(&memory.data(&store)[16..20],&2048u32.to_le_bytes());
        assert_eq!(&memory.data(&store)[2048..2056],&[0,4,0,0,0,0,0,0]);
        assert_eq!(set.call(&mut store,(32,96,0)).unwrap(),0);
        assert_eq!(&memory.data(&store)[pointer..pointer+4],b"one\0");
        assert_eq!(set.call(&mut store,(32,96,1)).unwrap(),0);
        assert_eq!(&memory.data(&store)[pointer..pointer+4],b"two\0");
        assert_eq!(set.call(&mut store,(128,64,1)).unwrap(),-1);
        assert_eq!(&memory.data(&store)[24..28],&22i32.to_le_bytes());
        let mut large=vec![b'x';1025];large.push(0);memory.write(&mut store,4096,&large).unwrap();
        assert_eq!(set.call(&mut store,(32,4096,1)).unwrap(),-1);
        assert_eq!(&memory.data(&store)[24..28],&12i32.to_le_bytes());
        assert_eq!(&memory.data(&store)[pointer..pointer+4],b"two\0");
        assert_eq!(unset.call(&mut store,32).unwrap(),0);assert_eq!(get.call(&mut store,32).unwrap(),0);
        assert_eq!(&memory.data(&store)[2048..2052],&[0;4]);
        assert_eq!(unset.call(&mut store,32).unwrap(),0);
    }
    #[test]
    fn legacy_clock_starts_on_first_call_and_uses_microsecond_ticks(){
        let (mut store,instance)=fixture(r#"(module
          (import "env" "memory" (memory 512))
          (import "env" "_clock" (func $clock (result i32)))
          (func (export "test") (result i32) call $clock))"#);
        let clock=instance.get_typed_func::<(),i32>(&mut store,"test").unwrap();
        assert_eq!(clock.call(&mut store,()).unwrap(),0);
        store.data_mut().clock_started=Some(Instant::now()-std::time::Duration::from_millis(12));
        let ticks=clock.call(&mut store,()).unwrap();
        assert!(ticks>=12_000);assert_eq!(ticks%1000,0);
    }
    #[test]
    fn single_thread_i64_atomics_preserve_carry_return_halves_and_compare_exchange(){
        let (mut store,instance)=fixture(r#"(module
          (import "env" "memory" (memory 512))
          (import "env" "___atomic_fetch_add_8" (func $add (param i32 i32 i32 i32) (result i32)))
          (import "env" "___atomic_compare_exchange_8" (func $cmp (param i32 i32 i32 i32 i32 i32) (result i32)))
          (global $high (mut i32) (i32.const 0))
          (func (export "setTempRet0") (param i32) local.get 0 global.set $high)
          (func (export "high") (result i32) global.get $high)
          (func (export "add") (param i32 i32 i32) (result i32) local.get 0 local.get 1 local.get 2 i32.const 5 call $add)
          (func (export "cmp") (param i32 i32 i32 i32) (result i32) local.get 0 local.get 1 local.get 2 local.get 3 i32.const 0 i32.const 5 call $cmp))"#);
        let memory=store.data().memory.unwrap();
        let read=|store:&Store<Host>,offset:usize|u64::from_le_bytes(memory.data(store)[offset..offset+8].try_into().unwrap());
        memory.write(&mut store,32,&0x1_FFFFFFFFu64.to_le_bytes()).unwrap();
        let add=instance.get_typed_func::<(i32,i32,i32),i32>(&mut store,"add").unwrap();
        assert_eq!(add.call(&mut store,(32,2,0)).unwrap(),-1);assert_eq!(read(&store,32),0x2_00000001);
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"high").unwrap().call(&mut store,()).unwrap(),1);
        let cmp=instance.get_typed_func::<(i32,i32,i32,i32),i32>(&mut store,"cmp").unwrap();
        assert_eq!(cmp.call(&mut store,(32,48,7,9)).unwrap(),0);assert_eq!(read(&store,48),0x2_00000001);
        assert_eq!(cmp.call(&mut store,(32,48,7,9)).unwrap(),1);assert_eq!(read(&store,32),0x9_00000007);
        memory.write(&mut store,32,&u64::MAX.to_le_bytes()).unwrap();
        assert_eq!(add.call(&mut store,(32,1,0)).unwrap(),-1);assert_eq!(read(&store,32),0);
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"high").unwrap().call(&mut store,()).unwrap(),-1);
        assert!(add.call(&mut store,(33_554_428,1,0)).is_err());
        assert!(cmp.call(&mut store,(32,33_554_428,1,0)).is_err());
        assert_eq!(read(&store,32),0);
    }
    #[test]
    fn imported_memcpy_copies_bytes_and_rejects_out_of_bounds(){
        let (mut store,instance)=fixture(r#"(module
          (import "env" "memory" (memory 512))
          (import "env" "_emscripten_memcpy_big" (func $copy (param i32 i32 i32) (result i32)))
          (data (i32.const 10) "abc")
          (func (export "copy") (param i32) (result i32) local.get 0 i32.const 10 i32.const 3 call $copy))"#);
        let copy=instance.get_typed_func::<i32,i32>(&mut store,"copy").unwrap();
        assert_eq!(copy.call(&mut store,20).unwrap(),20);
        assert_eq!(&store.data().memory.unwrap().data(&store)[20..23],b"abc");
        assert!(copy.call(&mut store,33_554_431).is_err());
    }
    #[test]
    fn heap_growth_preserves_data_and_obeys_two_gib_limit(){
        let (mut store,instance)=fixture(r#"(module
          (import "env" "memory" (memory 512))
          (import "env" "enlargeMemory" (func $grow (result i32)))
          (func (export "grow") (result i32) call $grow))"#);
        let memory=store.data().memory.unwrap();
        memory.write(&mut store,DYNAMIC_TOP_PTR,&33_554_440_u32.to_le_bytes()).unwrap();
        let grow=instance.get_typed_func::<(),i32>(&mut store,"grow").unwrap();
        assert_eq!(grow.call(&mut store,()).unwrap(),1);
        assert_eq!(memory.data_size(&store),67_108_864);
        memory.write(&mut store,DYNAMIC_TOP_PTR,&u32::MAX.to_le_bytes()).unwrap();
        assert_eq!(grow.call(&mut store,()).unwrap(),0);
        assert_eq!(memory.data_size(&store),67_108_864);
    }
}

#[cfg(all(test, feature = "aot-builder"))]
mod cross_compile_tests {
    #[test]
    fn arm64_android_aot_uses_the_target_unwind_profile() {
        let root = std::env::temp_dir().join(format!("raven-cross-aot-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("smoke.wasm");
        let output = root.join("smoke.cwasm");
        std::fs::write(&source, include_bytes!("../tests/smoke.wasm")).unwrap();
        super::precompile_for_target(source.to_str().unwrap(), "aarch64-linux-android", output.to_str().unwrap()).unwrap();
        let artifact = std::fs::read(&output).unwrap();
        assert_eq!(&artifact[..6], b"\x7fELF\x02\x01");
        assert_eq!(u16::from_le_bytes(artifact[18..20].try_into().unwrap()), 183);
        assert!(!artifact.windows(b".eh_frame\0".len()).any(|bytes| bytes == b".eh_frame\0"));
        std::fs::remove_file(source).unwrap();
        std::fs::remove_file(output).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}

#[cfg(all(test, feature = "compiler"))]
mod regression_tests {
    use super::*;
    use std::io::Write;
    fn fixture(source:&str)->(Store<Host>,Instance){
        let mut config=Config::new();config.consume_fuel(true);
        let engine=Engine::new(&config).unwrap();
        initialize(&engine,&Module::new(&engine,wat::parse_str(source).unwrap()).unwrap()).unwrap()
    }
    #[test]
    fn guest_write_and_sync_persist_real_bytes_and_require_a_save_mount(){
        let tag=format!("raven-save-abi-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos());
        let path=std::env::temp_dir().join(format!("{tag}.data"));let state=path.with_extension("saves");
        let mut bytes=b"UnityWebData1.0\0".to_vec();bytes.extend_from_slice(&20u32.to_le_bytes());std::fs::write(&path,bytes).unwrap();
        let (mut store,instance)=fixture(r#"(module
          (import "env" "memory" (memory 512))
          (import "env" "___syscall5" (func $open (param i32 i32) (result i32)))
          (import "env" "___syscall4" (func $write (param i32 i32) (result i32)))
          (import "env" "_JS_FileSystem_Sync" (func $sync))
          (func (export "open") (result i32) i32.const 5 i32.const 32 call $open)
          (func (export "write") (result i32) i32.const 4 i32.const 64 call $write)
          (func (export "sync") call $sync))"#);
        store.data_mut().filesystem=Some(filesystem::AssetFs::mount(path.to_str().unwrap()).unwrap());
        let sync=instance.get_typed_func::<(),()>(&mut store,"sync").unwrap();assert!(sync.call(&mut store,()).is_err());
        store.data_mut().filesystem.as_mut().unwrap().mount_saves(&state).unwrap();
        let memory=store.data().memory.unwrap();let words=|values:&[i32]|values.iter().flat_map(|n|n.to_le_bytes()).collect::<Vec<_>>();
        memory.write(&mut store,256,b"/idbfs/test\0").unwrap();memory.write(&mut store,32,&words(&[256,66])).unwrap();
        let fd=instance.get_typed_func::<(),i32>(&mut store,"open").unwrap().call(&mut store,()).unwrap();assert_eq!(fd,3);
        memory.write(&mut store,512,b"hello").unwrap();memory.write(&mut store,64,&words(&[fd,512,5])).unwrap();
        let write=instance.get_typed_func::<(),i32>(&mut store,"write").unwrap();assert_eq!(write.call(&mut store,()).unwrap(),5);
        memory.write(&mut store,64,&words(&[fd,33_554_431,5])).unwrap();assert_eq!(write.call(&mut store,()).unwrap(),-14);
        sync.call(&mut store,()).unwrap();let report=store.data().filesystem.as_ref().unwrap().report().saves.unwrap();
        assert_eq!(report.syncs,1);assert_eq!(report.written_bytes,5);assert!(!report.dirty);drop(store);
        let mut fs=filesystem::AssetFs::mount(path.to_str().unwrap()).unwrap();fs.mount_saves(&state).unwrap();let fd=fs.open("/idbfs/test",0).unwrap();
        let mut out=[0;5];assert_eq!(fs.read(fd,&mut out).unwrap(),5);assert_eq!(&out,b"hello");drop(fs);
        std::fs::remove_file(state).unwrap();std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn legacy_function_pointer_cast_uses_guest_trampoline(){
        let (mut store,instance)=fixture(r#"(module
          (import "env" "memory" (memory 512))
          (import "env" "table" (table 1 funcref))
          (import "env" "invoke_viii" (func $invoke (param i32 i32 i32 i32)))
          (func $two_args (param i32 i32) i32.const 100 local.get 0 local.get 1 i32.add i32.store)
          (elem (i32.const 0) $two_args)
          (func (export "dynCall_viii") (param i32 i32 i32 i32) local.get 1 local.get 2 call $two_args)
          (func (export "test") i32.const 0 i32.const 20 i32.const 22 i32.const 999 call $invoke))"#);
        instance.get_typed_func::<(),()>(&mut store,"test").unwrap().call(&mut store,()).unwrap();
        assert_eq!(&store.data().memory.unwrap().data(&store)[100..104],&42_i32.to_le_bytes());
    }
    #[test]
    fn anonymous_mapping_is_zeroed_and_unmapped_through_guest_allocator(){
        let (mut store,instance)=fixture(r#"(module
          (import "env" "memory" (memory 512))
          (import "env" "___syscall192" (func $map (param i32 i32) (result i32)))
          (import "env" "___syscall91" (func $unmap (param i32 i32) (result i32)))
          (func (export "_memalign") (param i32 i32) (result i32) i32.const 512)
          (func (export "_free") (param i32) i32.const 96 i32.const 1 i32.store)
          (func (export "map") (result i32) i32.const 192 i32.const 32 call $map)
          (func (export "unmap") (result i32) i32.const 91 i32.const 64 call $unmap))"#);
        let memory=store.data().memory.unwrap();
        let args=[0_i32,4096,3,34,-1,0].iter().flat_map(|value|value.to_le_bytes()).collect::<Vec<_>>();
        memory.write(&mut store,32,&args).unwrap();
        memory.write(&mut store,512,&[255;4096]).unwrap();
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"map").unwrap().call(&mut store,()).unwrap(),512);
        assert!(memory.data(&store)[512..4608].iter().all(|&byte|byte==0));
        memory.write(&mut store,64,&[512_i32,4096].iter().flat_map(|value|value.to_le_bytes()).collect::<Vec<_>>()).unwrap();
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"unmap").unwrap().call(&mut store,()).unwrap(),0);
        assert_eq!(&memory.data(&store)[96..100],&1_i32.to_le_bytes());
        assert!(store.data().mappings.is_empty());
    }
    #[test]
    fn single_thread_tls_stores_and_deletes_values(){
        let (mut store,instance)=fixture(r#"(module
          (import "env" "memory" (memory 512))
          (import "env" "_pthread_key_create" (func $create (param i32 i32) (result i32)))
          (import "env" "_pthread_setspecific" (func $set (param i32 i32) (result i32)))
          (import "env" "_pthread_getspecific" (func $get (param i32) (result i32)))
          (import "env" "_pthread_key_delete" (func $delete (param i32) (result i32)))
          (func (export "test") (result i32)
            i32.const 128 i32.const 0 call $create drop
            i32.const 128 i32.load i32.const 7 call $set drop
            i32.const 128 i32.load call $get)
          (func (export "clear") (result i32)
            i32.const 128 i32.load call $delete drop
            i32.const 128 i32.load call $get))"#);
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"test").unwrap().call(&mut store,()).unwrap(),7);
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"clear").unwrap().call(&mut store,()).unwrap(),0);
    }
    #[test]
    fn filesystem_syscall_abi_and_file_mapping_read_real_container_bytes(){
        let path=std::env::temp_dir().join(format!("raven-syscall-{}.data",std::process::id()));
        let mut file=std::fs::File::create(&path).unwrap();
        file.write_all(b"UnityWebData1.0\0").unwrap();
        for value in [43_u32,43,5,11]{file.write_all(&value.to_le_bytes()).unwrap();}
        file.write_all(b"boot.confighello").unwrap();drop(file);
        let (mut store,instance)=fixture(r#"(module
          (import "env" "memory" (memory 512))
          (import "env" "___syscall5" (func $open (param i32 i32) (result i32)))
          (import "env" "___syscall145" (func $readv (param i32 i32) (result i32)))
          (import "env" "___syscall197" (func $stat (param i32 i32) (result i32)))
          (import "env" "___syscall220" (func $dents (param i32 i32) (result i32)))
          (import "env" "___syscall192" (func $map (param i32 i32) (result i32)))
          (func (export "_memalign") (param i32 i32) (result i32) i32.const 16384)
          (func (export "_free") (param i32))
          (func (export "open") (result i32) i32.const 5 i32.const 32 call $open)
          (func (export "readv") (result i32) i32.const 145 i32.const 64 call $readv)
          (func (export "stat") (result i32) i32.const 197 i32.const 96 call $stat)
          (func (export "getdents") (result i32) i32.const 220 i32.const 160 call $dents)
          (func (export "map") (result i32) i32.const 192 i32.const 128 call $map))"#);
        store.data_mut().filesystem=Some(filesystem::AssetFs::mount(path.to_str().unwrap()).unwrap());
        let memory=store.data().memory.unwrap();
        let words=|values:&[i32]|values.iter().flat_map(|n|n.to_le_bytes()).collect::<Vec<_>>();
        memory.write(&mut store,256,b"boot.config\0").unwrap();
        memory.write(&mut store,32,&words(&[256,32768])).unwrap();
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"open").unwrap().call(&mut store,()).unwrap(),3);
        memory.write(&mut store,64,&words(&[3,192,2])).unwrap();
        memory.write(&mut store,192,&words(&[512,2,514,8])).unwrap();
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"readv").unwrap().call(&mut store,()).unwrap(),5);
        assert_eq!(&memory.data(&store)[512..517],b"hello");
        memory.write(&mut store,96,&words(&[3,768])).unwrap();
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"stat").unwrap().call(&mut store,()).unwrap(),0);
        assert_eq!(&memory.data(&store)[804..808],&5_i32.to_le_bytes());
        memory.write(&mut store,128,&words(&[0,4096,3,2,3,0])).unwrap();
        memory.write(&mut store,16384,&[255;4096]).unwrap();
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"map").unwrap().call(&mut store,()).unwrap(),16384);
        assert_eq!(&memory.data(&store)[16384..16389],b"hello");
        assert!(memory.data(&store)[16389..20480].iter().all(|&b|b==0));
        // Shared writes and mappings requiring beyond-EOF fault semantics are rejected.
        memory.write(&mut store,128,&words(&[0,4096,3,1,3,0])).unwrap();
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"map").unwrap().call(&mut store,()).unwrap(),-13);
        memory.write(&mut store,128,&words(&[0,8192,1,2,3,0])).unwrap();
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"map").unwrap().call(&mut store,()).unwrap(),-22);
        assert_eq!(store.data().filesystem.as_ref().unwrap().report().read_bytes,10);
        memory.write(&mut store,1024,b"/\0").unwrap();memory.write(&mut store,32,&words(&[1024,65536])).unwrap();
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"open").unwrap().call(&mut store,()).unwrap(),4);
        memory.write(&mut store,160,&words(&[4,-2,268])).unwrap();
        let dents=instance.get_typed_func::<(),i32>(&mut store,"getdents").unwrap();assert_eq!(dents.call(&mut store,()).unwrap(),-14);
        memory.write(&mut store,160,&words(&[4,2048,268])).unwrap();assert_eq!(dents.call(&mut store,()).unwrap(),268);
        assert_eq!(&memory.data(&store)[2059..2071],b"boot.config\0");assert_eq!(memory.data(&store)[2058],8);
        drop(store);std::fs::remove_file(path).unwrap();
    }
}
