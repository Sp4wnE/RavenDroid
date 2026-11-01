//! Opt-in epoch experiment: wall-clock interruption remains active without fuel.
use std::{sync::{Arc,atomic::{AtomicBool,Ordering}},thread::{self,JoinHandle},time::Duration};
pub(super) struct Watchdog {stop:Arc<AtomicBool>,thread:Option<JoinHandle<()>>}
impl Watchdog {
    pub fn start(engine:&wasmtime::Engine)->std::io::Result<Self>{
        let stop=Arc::new(AtomicBool::new(false));let signal=stop.clone();let engine=engine.clone();
        let thread=thread::Builder::new().name("RavenEpochGuard".into()).spawn(move||{
            while !signal.load(Ordering::Acquire){
                thread::park_timeout(Duration::from_millis(10));
                if !signal.load(Ordering::Acquire){engine.increment_epoch();}
            }
        })?;
        Ok(Self{stop,thread:Some(thread)})
    }
}
impl Drop for Watchdog {
    fn drop(&mut self){
        self.stop.store(true,Ordering::Release);
        if let Some(thread)=self.thread.take(){thread.thread().unpark();let _=thread.join();}
    }
}
#[cfg(all(test,feature="compiler"))]mod tests {
    use super::*;
    #[test]fn infinite_guest_is_interrupted_and_later_calls_can_reset_the_deadline(){
        let mut config=crate::engine_config_execution(false,false,true);
        config.cranelift_opt_level(wasmtime::OptLevel::Speed);
        let engine=wasmtime::Engine::new(&config).unwrap();
        let wasm=wat::parse_str("(module (func (export \"spin\") (loop br 0)) (func (export \"value\") (result i32) i32.const 42))").unwrap();
        let module=wasmtime::Module::new(&engine,wasm).unwrap();
        let mut store=wasmtime::Store::new(&engine,());store.set_epoch_deadline(3);
        let instance=wasmtime::Instance::new(&mut store,&module,&[]).unwrap();
        let _guard=Watchdog::start(&engine).unwrap();let start=std::time::Instant::now();
        let error=instance.get_typed_func::<(),()>(&mut store,"spin").unwrap().call(&mut store,()).unwrap_err();
        assert_eq!(error.downcast_ref::<wasmtime::Trap>(),Some(&wasmtime::Trap::Interrupt));
        assert!(start.elapsed()<Duration::from_secs(2));
        store.set_epoch_deadline(100);
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"value").unwrap().call(&mut store,()).unwrap(),42);
    }
}
