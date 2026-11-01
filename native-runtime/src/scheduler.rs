//! Owner-thread scheduling for the pinned, non-shared runtime.
use super::{Host,host_error,value_i32,write_i32};
use std::time::{Duration,Instant};
use wasmtime::{Caller,Val,Store,Instance};
#[derive(Default)]
pub(super) struct Scheduler {
    function:Option<i32>,mode:i32,value:i32,
    pub(super) frames:u32,
    pub(super) work_us:u128,pub(super) max_work_us:u128,
    pub(super) performance:super::performance::Performance,
    save_timer:Option<Instant>,
}
#[derive(Debug)]pub(super) struct MainLoopYield;
impl std::fmt::Display for MainLoopYield{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.write_str("Expected Emscripten main-loop unwind")}}
impl std::error::Error for MainLoopYield{}

pub(super) fn dispatch(caller:&mut Caller<'_,Host>,name:&str,args:&[Val],results:&mut[Val])->wasmtime::Result<bool>{
    match name{
        "env._JS_FileSystem_Initialize"=>{
            if caller.data().filesystem.as_ref().and_then(|fs|fs.report().saves).is_none(){return Err(host_error("Periodic save sync requires a mounted save store"));}
            caller.data_mut().scheduler.save_timer=Some(Instant::now()+Duration::from_secs(1));
        }
        "env._emscripten_set_main_loop"=>{
            let function=value_i32(args,0)?;let fps=value_i32(args,1)?;
            if function==0 || !(0..=1000).contains(&fps){return Err(host_error("Invalid main-loop function or frame rate"));}
            let scheduler=&mut caller.data_mut().scheduler;
            if scheduler.function.is_some(){return Err(host_error("A guest main loop is already registered"));}
            scheduler.function=Some(function);scheduler.mode=if fps==0{1}else{0};scheduler.value=if fps==0{1}else{1000/fps};
            if value_i32(args,2)?!=0{return Err(MainLoopYield.into());}
        }
        "env._emscripten_cancel_main_loop"=>caller.data_mut().scheduler.function=None,
        "env._emscripten_set_main_loop_timing"=>{
            let mode=value_i32(args,0)?;let value=value_i32(args,1)?;
            if !matches!(mode,0..=2) || value<0 || (mode==1 && value==0){return Err(host_error("Invalid main-loop timing"));}
            let scheduler=&mut caller.data_mut().scheduler;scheduler.mode=mode;scheduler.value=value;
            results[0]=Val::I32(scheduler.function.is_none() as i32);
        }
        "env._emscripten_get_main_loop_timing"=>{
            let scheduler=&caller.data().scheduler;let (mode,value)=(scheduler.mode,scheduler.value);
            if value_i32(args,0)?!=0{write_i32(caller,value_i32(args,0)? as u32 as usize,mode)?;}
            if value_i32(args,1)?!=0{write_i32(caller,value_i32(args,1)? as u32 as usize,value)?;}
        }
        _=>return Ok(false),
    }Ok(true)
}
pub(super) fn run(store:&mut Store<Host>,instance:&Instance,limit:u32,fuel:u64,progress:&super::diagnostics::Progress,trace_lists:bool,space_at:Option<u32>,taps:&[(u32,i32,i32)])->anyhow::Result<()> {
    let callback=instance.get_typed_func::<i32,() >(&mut *store,"dynCall_v").map_err(anyhow::Error::from)?;
    let mut pacing=FramePacing::default();
    #[cfg(target_os="android")]let mut hint=store.data().live.then(super::android_performance::Hint::new).flatten();
    #[cfg(target_os="android")]{store.data_mut().scheduler.performance.android_hint=hint.as_ref().map(|h|h.status()).unwrap_or_else(||serde_json::json!({"session_created":false}));}
    for _ in 0..limit {
        let Some(function)=store.data().scheduler.function else{break;};let start=Instant::now();
        let cpu_start=super::performance::thread_cpu_us();
        let draws_before=store.data().graphics.as_ref().map(|g|g.draw_calls).unwrap_or(0);
        let before=trace_lists.then(||list_snapshot(store));
        let epoch_guard=store.data().epoch_guard;
        // One callback also performs synchronous texture/shader/level loading.
        // Keep it bounded without treating legitimate loading as a hung frame.
        if epoch_guard{store.set_epoch_deadline(3000);}else{store.set_fuel(fuel).map_err(anyhow::Error::from)?;}
        if let Some(control)=store.data().control.clone(){
            if control.stopped(){break;}
            let frame=store.data().scheduler.frames+1;
            for &(pressed,x,y) in taps{
                if frame==pressed{control.push(super::session::Pointer{kind:0,x,y});}
                if pressed.checked_add(1)==Some(frame){control.push(super::session::Pointer{kind:2,x,y});}
            }
            let events=store.data().scripted_game.iter().filter(|(at,_)|*at==frame).map(|(_,event)|*event).collect::<Vec<_>>();
            for event in events{control.push_game(event);}
            for event in control.drain_frame(){super::pointer::deliver(store,instance,event)?;}
            for event in control.drain_game(){super::game_input::deliver(store,instance,event)?;}
            control.callbacks.store(store.data().pointer.callbacks,std::sync::atomic::Ordering::Relaxed);
            if !active(store.data()){break;}
        }
        #[cfg(windows)]if store.data().input.enabled{
            let frame=store.data().scheduler.frames+1;
            let graphics=store.data_mut().graphics.as_mut().ok_or_else(||host_error("Keyboard source lost its native window"))?;
            if space_at==Some(frame){graphics.post_space(true)?;}
            if space_at.is_some_and(|pressed|pressed+1==frame){graphics.post_space(false)?;}
            for _ in 0..256{
                let Some((key,message))=store.data_mut().graphics.as_mut().unwrap().next_key()? else{break;};
                let prevented=super::input::deliver(store,instance,key)?;
                store.data().graphics.as_ref().unwrap().finish_key(&message,prevented);
            }
            if store.data().scheduler.function.is_none(){break;}
            if store.data().scheduler.function!=Some(function){continue;}
        }
        #[cfg(not(windows))]let _=space_at;
        match callback.call(&mut *store,function){
            Ok(())=>{},Err(error) if error.downcast_ref::<MainLoopYield>().is_some()=>{},Err(error)=>{
                if let Some(before)=before{
                    let after=list_snapshot(store);
                    let diagnostic=format!("guest-list-trap frame={} before={before} after={after}",store.data().scheduler.frames+1);
                    progress.record(&diagnostic);store.data_mut().logs.push(diagnostic);
                }
                return Err(error.into());
            },
        }
        if let Some(audio)=store.data_mut().audio.as_mut(){
            let ended=audio.pump()?;
            for (function,user) in ended{instance.get_typed_func::<(i32,i32),()>(&mut *store,"dynCall_vi")?.call(&mut *store,(function,user))?;}
        }
        store.data_mut().scheduler.frames+=1;
        if let Some(path)=store.data().capture_frames.get(&store.data().scheduler.frames).cloned(){
            store.data_mut().graphics.as_mut().ok_or_else(||host_error("Checkpoint capture needs native graphics"))?.capture(&path)?;
            progress.record(&format!("capture-checkpoint frame={} path={}",store.data().scheduler.frames,path.display()));
        }
        let work=start.elapsed().as_micros();let scheduler=&mut store.data_mut().scheduler;
        scheduler.work_us+=work;scheduler.max_work_us=scheduler.max_work_us.max(work);
        if work>=250_000{progress.frame_record(&format!("main-loop-slow frame={} work_ms={}",scheduler.frames,work/1000));}
        let present_start=Instant::now();
        #[cfg(not(target_os="android"))]let presented=false;
        #[cfg(target_os="android")]let mut presented=false;
        #[cfg(target_os="android")]if store.data().live{
            let stopped=store.data().control.as_ref().is_some_and(|control|control.stopped());
            if stopped{break;}
            match store.data_mut().graphics.as_mut().unwrap().present(){Ok(value)=>presented=value,Err(error)=>{if store.data().control.as_ref().is_some_and(|control|control.stopped()){break;}return Err(error);}}
        }
        let present_us=present_start.elapsed().as_micros();let maintenance_start=Instant::now();
        #[cfg(target_os="android")]if let Some(hint)=hint.as_mut(){
            let s=&store.data().scheduler;
            let target=match s.mode{0 if s.value>0=>s.value as u128*1_000_000,1 if s.value>0=>s.value as u128*1_000_000_000/60,_=>16_666_667};
            hint.report(work*1000,target);
        }
        if let Some(control)=store.data().control.as_ref(){control.frame(store.data().scheduler.frames,store.data().graphics.as_ref().map(|graphics|graphics.draw_calls).unwrap_or(0),presented);}
        if store.data().scheduler.frames%30==0{progress.frame_record(&format!("main-loop-completed={}",store.data().scheduler.frames));}
        if store.data().scheduler.save_timer.is_some_and(|due|due<=Instant::now()) {
            if let Some(filesystem)=store.data_mut().filesystem.as_mut(){if filesystem.report().saves.is_some_and(|saves|saves.dirty){filesystem.sync_saves()?;}}
            store.data_mut().scheduler.save_timer=Some(Instant::now()+Duration::from_secs(1));
        }
        let maintenance_us=maintenance_start.elapsed().as_micros();let scheduler=&store.data().scheduler;
        let duration=match scheduler.mode{0=>Duration::from_millis(scheduler.value as u64),1=>Duration::from_secs_f64(scheduler.value as f64/60.0),_=>Duration::ZERO};
        let sleep_start=Instant::now();
        if let Some(remaining)=pacing.remaining(start,Instant::now(),duration){std::thread::sleep(remaining);}
        let consumed=if epoch_guard{0}else{fuel.saturating_sub(store.get_fuel().map_err(anyhow::Error::from)?)};
        let sample=super::performance::Sample{frame:store.data().scheduler.frames,work_us:work,present_us,maintenance_us,sleep_us:sleep_start.elapsed().as_micros(),total_us:start.elapsed().as_micros(),draws:store.data().graphics.as_ref().map(|g|g.draw_calls).unwrap_or(0).saturating_sub(draws_before),fuel:consumed,cpu_us:cpu_start.zip(super::performance::thread_cpu_us()).map(|(before,after)|after.saturating_sub(before))};
        store.data_mut().scheduler.performance.record(sample);
    }
    #[cfg(target_os="android")]if let Some(hint)=hint{store.data_mut().scheduler.performance.android_hint=hint.status();}
    Ok(())
}
pub(super) fn active(host:&Host)->bool{host.scheduler.function.is_some()}

// Carry ordinary sleep/vsync overshoot into the next deadline instead of adding
// it to every frame. A full missed period rebases; loading never causes a burst
// of catch-up ticks. Guest clocks and the requested frame rate are unchanged.
#[derive(Default)]
struct FramePacing { deadline:Option<Instant>,period:Duration }
impl FramePacing {
    fn remaining(&mut self,start:Instant,now:Instant,period:Duration)->Option<Duration>{
        if period.is_zero(){self.deadline=None;self.period=period;return None;}
        let due=if self.period==period{self.deadline.map(|d|d+period).unwrap_or(start+period)}else{start+period};
        self.period=period;
        self.deadline=Some(if now.saturating_duration_since(due)>=period{now}else{due});
        due.checked_duration_since(now).filter(|d|!d.is_zero())
    }
}
#[cfg(test)]mod pacing_tests {
    use super::*;
    #[test]fn oversleep_does_not_accumulate_and_loading_does_not_catch_up(){
        let start=Instant::now();let p=Duration::from_micros(16_667);let mut pacing=FramePacing::default();
        assert_eq!(pacing.remaining(start,start+Duration::from_millis(5),p),Some(p-Duration::from_millis(5)));
        let second=start+p+Duration::from_millis(1);
        assert_eq!(pacing.remaining(second,second+Duration::from_millis(5),p),Some(p-Duration::from_millis(6)));
        let loaded=start+Duration::from_secs(1);assert_eq!(pacing.remaining(second,loaded,p),None);
        assert_eq!(pacing.remaining(loaded,loaded+Duration::from_millis(5),p),Some(p-Duration::from_millis(5)));
    }
    #[test]fn timing_changes_reset_phase_and_unpaced_mode_has_no_sleep(){
        let start=Instant::now();let mut pacing=FramePacing::default();
        pacing.remaining(start,start,Duration::from_millis(16));
        let next=start+Duration::from_millis(17);
        assert_eq!(pacing.remaining(next,next+Duration::from_millis(2),Duration::from_millis(33)),Some(Duration::from_millis(31)));
        assert_eq!(pacing.remaining(next,next,Duration::ZERO),None);
        assert_eq!(pacing.remaining(next,next,Duration::from_millis(16)),Some(Duration::from_millis(16)));
    }
}

// Addresses belong only to the reviewed game.wasm (SHA256 03c6c951...).
// Walk at most 64 nodes, retain no raw heap dump, and never repair guest memory.
fn list_snapshot(store:&Store<Host>)->serde_json::Value{
    let Some(memory)=store.data().memory else{return serde_json::Value::Null;};
    let bytes=memory.data(store);
    let word=|address:u32|address.checked_add(4).and_then(|end|bytes.get(address as usize..end as usize))
        .map(|value|u32::from_le_bytes(value.try_into().unwrap()));
    let queue=word(7_309_176).unwrap_or(0);
    let pool=word(7_308_632).unwrap_or(0);
    let fields=|address:u32|(0..8).map(|index|address.checked_add(index*4).and_then(word)).collect::<Vec<_>>();
    let sentinel=queue.checked_add(20).unwrap_or(0);
    let mut current=queue.checked_add(24).and_then(word).unwrap_or(0);
    let mut seen=std::collections::BTreeSet::new();let mut nodes=Vec::new();
    let end=loop{
        if current==sentinel{break "sentinel";}
        if current==0{break "null";}
        if !seen.insert(current){break "cycle";}
        if nodes.len()==64{break "limit";}
        let Some(next)=current.checked_add(4).and_then(word) else{break "outside-memory";};
        nodes.push(serde_json::json!({"address":current,"previous":word(current),"next":next}));current=next;
    };
    let mut recycled=Vec::new();let mut recycle_seen=std::collections::BTreeSet::new();
    let mut recycled_pointer=pool.checked_add(28).and_then(word).unwrap_or(0);
    while recycled_pointer!=0 && recycled.len()<64 && recycle_seen.insert(recycled_pointer){
        recycled.push(serde_json::json!({"address":recycled_pointer,"previous":word(recycled_pointer),"next":recycled_pointer.checked_add(4).and_then(word)}));
        recycled_pointer=word(recycled_pointer).unwrap_or(0);
    }
    serde_json::json!({"queue":queue,"queue_words":fields(queue),"pool":pool,"pool_words":fields(pool),"nodes":nodes,"end":end,"end_address":current,"recycled":recycled,"recycled_end":recycled_pointer})
}

#[cfg(all(test,feature="compiler"))]
mod tests{
    use super::*;
    #[test]
    fn list_diagnostic_detects_cycles_without_modifying_guest_memory(){
        let (mut store,_)=crate::tests::fixture(r#"(module (import "env" "memory" (memory 512)))"#);
        let memory=store.data().memory.unwrap();
        for (address,value) in [(7_309_176,1024u32),(7_308_632,2048),(1044,4096),(1048,4096),(4096,1044),(4100,4096)]{
            memory.write(&mut store,address,&value.to_le_bytes()).unwrap();
        }
        let before=memory.data(&store).to_vec();
        let snapshot=list_snapshot(&store);
        assert_eq!(snapshot["end"],"cycle");assert_eq!(snapshot["nodes"].as_array().unwrap().len(),1);
        assert_eq!(memory.data(&store),before);
        memory.write(&mut store,1048,&u32::MAX.to_le_bytes()).unwrap();
        assert_eq!(list_snapshot(&store)["end"],"outside-memory");
        memory.write(&mut store,1048,&1044u32.to_le_bytes()).unwrap();
        assert_eq!(list_snapshot(&store)["end"],"sentinel");
    }
}
