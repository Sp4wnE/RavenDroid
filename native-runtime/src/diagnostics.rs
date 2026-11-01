//! Durable phase markers survive native aborts. Do not attempt signal recovery.
use anyhow::{Context, Result};
use serde::Serialize;
use std::{fs::OpenOptions, io::Write, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
use std::time::Instant;
use wasmtime::{Engine, Instance, Module, Store};

pub(super) struct Progress {
    path: Option<PathBuf>,
    stderr: bool,
}

impl Progress {
    pub(super) fn disabled() -> Self { Self { path: None, stderr: false } }

    pub(super) fn cli() -> Self { Self { path: None, stderr: true } }

    #[cfg(target_os = "android")]
    pub(super) fn at(path: &Path) -> Self { Self { path: Some(path.to_path_buf()), stderr: false } }

    /// Frequent frame markers must not force a storage flush on the render thread.
    pub(super) fn frame_record(&self, message: &str) {
        if self.stderr { eprintln!("[RavenNative] {message}"); }
        if let Some(path)=&self.path { append(path,message,false); }
    }

    pub(super) fn record(&self, message: &str) {
        if self.stderr { eprintln!("[RavenNative] {message}"); }
        if let Some(path) = &self.path {
            append(path, message, true);
        }
    }
    /// Completed VM measurements remain reviewable if a driver stalls in teardown.
    pub(super) fn checkpoint<T:Serialize>(&self,value:&T){
        if let Some(path)=&self.path{
            let destination=path.with_file_name("native-session-report.json");
            let staging=destination.with_extension("json.partial");
            if let Ok(mut file)=OpenOptions::new().create(true).truncate(true).write(true).open(&staging){
                if serde_json::to_writer(&mut file,value).is_ok()&&file.sync_data().is_ok(){
                    let _=std::fs::rename(staging,destination);
                }
            }
        }
    }
}

fn append(path: &Path, message: &str, durable: bool) {
    // Each marker is bounded; a missing/unwritable diagnostic never aborts the host.
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let _ = writeln!(file, "{} {}", now.as_millis(), message.chars().take(8192).collect::<String>());
        if durable { let _ = file.sync_data(); }
    }
}

#[derive(Serialize)]
pub(super) struct SmokeReport {
    diagnostic_version: &'static str,
    architecture: &'static str,
    test: &'static str,
    compile_ms: u128,
    deserialize_ms: u128,
    compiler_included: bool,
    returned_value: i32,
    bounds_trap_caught: bool,
    playable: bool,
    rendered_game_frame: bool,
}

pub(super) fn smoke(progress: &Progress, artifact: Option<&str>) -> Result<SmokeReport> {
    progress.record("smoke-engine-create-enter");
    #[cfg(not(target_os="android"))]let config = super::engine_config();
    #[cfg(target_os="android")]let config=super::engine_config_execution(super::android_profile::GUEST_EH,super::android_profile::INLINE_EAGER,super::android_profile::EPOCH_GUARD);
    #[cfg(target_os="android")]let epoch_guard=super::android_profile::EPOCH_GUARD;
    #[cfg(not(target_os="android"))]let epoch_guard=false;
    let engine = Engine::new(&config).map_err(anyhow::Error::from)?;
    let clock = Instant::now();
    let module = if let Some(path) = artifact {
        progress.record("smoke-aot-deserialize-enter");
        // Only the app-private, hash-verified copy of its signed APK asset.
        unsafe { Module::deserialize_file(&engine, path) }.map_err(anyhow::Error::from)?
    } else {
        #[cfg(feature = "compiler")]
        { Module::new(&engine, include_bytes!("../tests/smoke.wasm")).map_err(anyhow::Error::from)? }
        #[cfg(not(feature = "compiler"))]
        { anyhow::bail!("This runtime accepts only its bundled trusted AOT module") }
    };
    let elapsed_ms = clock.elapsed().as_millis();
    progress.record("smoke-module-ready");
    let mut store = Store::new(&engine, ());
    let _watchdog=epoch_guard.then(||super::execution_guard::Watchdog::start(&engine)).transpose()?;
    if epoch_guard{store.set_epoch_deadline(100);}else{store.set_fuel(100_000).map_err(anyhow::Error::from)?;}
    progress.record("smoke-instantiate-enter");
    let instance = Instance::new(&mut store, &module, &[]).map_err(anyhow::Error::from)?;
    progress.record("smoke-call-enter");
    let returned_value = instance.get_typed_func::<(), i32>(&mut store, "run")
        .map_err(anyhow::Error::from)?.call(&mut store, ()).map_err(anyhow::Error::from)?;
    anyhow::ensure!(returned_value == 42, "Native smoke test returned the wrong value");
    progress.record("smoke-bounds-trap-enter");
    let trapped = instance.get_typed_func::<(), i32>(&mut store, "bounds_trap")
        .map_err(anyhow::Error::from)?.call(&mut store, ()).err().context("Expected a WASM bounds trap")?;
    let bounds_trap_caught = trapped.downcast_ref::<wasmtime::Trap>() == Some(&wasmtime::Trap::MemoryOutOfBounds);
    anyhow::ensure!(bounds_trap_caught, "Expected WASM bounds trap, received {trapped:#}");
    progress.record("smoke-complete value=42 bounds_trap_caught=true");
    Ok(SmokeReport { diagnostic_version: "0.4-disk-filesystem", architecture: std::env::consts::ARCH,
        test: "small-module-execution-and-bounds-trap", compile_ms: if artifact.is_none() { elapsed_ms } else { 0 },
        deserialize_ms: if artifact.is_some() { elapsed_ms } else { 0 }, compiler_included: cfg!(feature = "compiler"), returned_value, bounds_trap_caught,
        playable: false, rendered_game_frame: false })
}

#[cfg(target_os = "android")]
fn android_call(mut env: jni::JNIEnv, path: jni::objects::JString, log: jni::objects::JString, small: bool, data:Option<jni::objects::JString>,graphics:bool) -> jni::sys::jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<String> {
        let _execution=super::session::registry::Execution::enter(None)?;
        let log: String = env.get_string(&log)?.into();
        let progress = Progress::at(Path::new(&log));
        // This is a diagnostic hook, not a handler for SIGABRT/SIGSEGV. Android
        // retains its own crash reporting. Never try to resume an aborted VM.
        static PANIC_PATH: std::sync::OnceLock<std::sync::Mutex<PathBuf>> = std::sync::OnceLock::new();
        static INSTALL_HOOK: std::sync::Once = std::sync::Once::new();
        let path_slot = PANIC_PATH.get_or_init(|| std::sync::Mutex::new(PathBuf::new()));
        if let Ok(mut current) = path_slot.lock() { *current = PathBuf::from(&log); }
        INSTALL_HOOK.call_once(|| {
            let previous = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                if let Some(slot) = PANIC_PATH.get() {
                    if let Ok(path) = slot.lock() { append(&path, &format!("rust-panic {info}"), true); }
                }
                previous(info);
            }));
        });
        progress.record("jni-enter");
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        progress.record(&format!("host-page-size={page_size}"));
        let path: String = env.get_string(&path)?.into();
        let data=if let Some(value)=data {let path:String=env.get_string(&value)?.into();Some(path)}else{None};
        let result = if !small {
            let options=super::ProbeOptions{guest_eh:super::android_profile::GUEST_EH,inline_eager_codegen:super::android_profile::INLINE_EAGER,epoch_guard:super::android_profile::EPOCH_GUARD,save_state:data.as_ref().map(|path|Path::new(path).with_file_name("native-saves.bin")),
                android_gles:graphics,headless_events:graphics,headless_audio:graphics,
                frame_limit:if graphics{1800}else{0},startup_fuel:graphics.then_some(50_000_000_000),frame_fuel:graphics.then_some(50_000_000_000),
                capture:if graphics{data.as_ref().map(|path|Path::new(path).with_file_name("native-game-frame.ppm"))}else{None},..Default::default()};
            let probe = super::probe_impl(&path, None, true, &progress, data.as_deref(),options)?;
            serde_json::to_string_pretty(&probe)?
        } else {
            serde_json::to_string_pretty(&smoke(&progress, Some(&path))?)?
        };
        progress.record("jni-result-ready");
        Ok(result)
    }));
    let text = match result {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => serde_json::json!({"error":format!("{error:#}"),"playable":false}).to_string(),
        Err(_) => serde_json::json!({"error":"Rust panic; see durable native phase log","playable":false}).to_string(),
    };
    match env.new_string(text) { Ok(value) => value.into_raw(), Err(_) => std::ptr::null_mut() }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ravenfield_nativehost_MainActivity_runProbeNative(
    env: jni::JNIEnv, _class: jni::objects::JClass, path: jni::objects::JString, log: jni::objects::JString, data: jni::objects::JString,
) -> jni::sys::jstring { android_call(env, path, log, false,Some(data),false) }

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ravenfield_nativehost_MainActivity_runGraphicsNative(
    env: jni::JNIEnv, _class: jni::objects::JClass, path: jni::objects::JString, log: jni::objects::JString, data: jni::objects::JString,
) -> jni::sys::jstring { android_call(env,path,log,false,Some(data),true) }

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ravenfield_nativehost_MainActivity_runSmokeNative(
    env: jni::JNIEnv, _class: jni::objects::JClass, path: jni::objects::JString, log: jni::objects::JString,
) -> jni::sys::jstring { android_call(env, path, log, true,None,false) }

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "compiler")]
    #[test]
    fn small_native_test_executes_guest_and_catches_real_bounds_trap() {
        let report = smoke(&Progress::disabled(), None).unwrap();
        assert_eq!(report.returned_value, 42);
        assert!(report.bounds_trap_caught);
        assert!(!report.playable);
        assert_eq!(wat::parse_str(include_str!("../tests/smoke.wat")).unwrap(), include_bytes!("../tests/smoke.wasm"));
    }

    #[test]
    fn checkpoint_survives_reopening_the_file() {
        let path = std::env::temp_dir().join(format!("raven-checkpoint-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let progress = Progress { path: Some(path.clone()), stderr: false };
        progress.record("compile-enter");
        progress.record("compile-complete");
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 2);
        assert!(content.contains("compile-enter") && content.contains("compile-complete"));
        std::fs::remove_file(path).unwrap();
    }
}
