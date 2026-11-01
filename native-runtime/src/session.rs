//! Thread-safe control plane. Never contains a VM, GL context or native window.
use std::{collections::VecDeque,sync::{Arc,Mutex,atomic::{AtomicBool,AtomicU64,Ordering}},time::Instant};
#[derive(Clone,Copy,Debug,PartialEq)]pub(crate) struct Pointer{pub kind:i32,pub x:i32,pub y:i32}
struct Queue{events:VecDeque<Pointer>,overflow:bool}
#[derive(Clone,Copy,Debug,PartialEq)]pub(crate) enum GameEvent{Key{code:u32,down:bool},Button{button:u8,down:bool},Look{dx:i32,dy:i32},Reset}
impl GameEvent{pub(crate) fn from_wire(kind:i32,a:i32,b:i32)->Option<Self>{match kind{
    0 if super::game_input::supported(a as u32)&&matches!(b,0|1)=>Some(Self::Key{code:a as u32,down:b!=0}),
    1 if a.abs_diff(0)<=4096&&b.abs_diff(0)<=4096=>Some(Self::Look{dx:a,dy:b}),
    2 if matches!(a,0|2)&&matches!(b,0|1)=>Some(Self::Button{button:a as u8,down:b!=0}),
    3=>Some(Self::Reset),_=>None,
}}}
struct GameQueue{events:VecDeque<(GameEvent,Instant)>,overflow:bool}
pub(crate) struct Control{
    pub audio_muted:Arc<AtomicBool>,stopped:AtomicBool,queue:Mutex<Queue>,pub frames:AtomicU64,pub draws:AtomicU64,
    pub presented:AtomicU64,pub callbacks:AtomicU64,last_frame:AtomicU64,started:Instant,
    game:Mutex<GameQueue>,
}
impl Control{
    pub(crate) fn new()->Arc<Self>{Arc::new(Self{audio_muted:Arc::new(AtomicBool::new(false)),stopped:AtomicBool::new(false),queue:Mutex::new(Queue{events:VecDeque::new(),overflow:false}),game:Mutex::new(GameQueue{events:VecDeque::new(),overflow:false}),frames:AtomicU64::new(0),draws:AtomicU64::new(0),presented:AtomicU64::new(0),callbacks:AtomicU64::new(0),last_frame:AtomicU64::new(0),started:Instant::now()})}
    pub(crate) fn push_game(&self,event:GameEvent)->bool{
        if self.stopped(){return false;}
        if !match event{GameEvent::Key{code,..}=>super::game_input::supported(code),GameEvent::Button{button,..}=>matches!(button,0|2),GameEvent::Look{dx,dy}=>dx.abs_diff(0)<=4096&&dy.abs_diff(0)<=4096,GameEvent::Reset=>true}{return false;}
        let mut q=self.game.lock().unwrap_or_else(|e|e.into_inner());
        if event==GameEvent::Reset{
            q.events.clear();q.overflow=false;q.events.push_back((event,Instant::now()));
            let mut pointer=self.queue.lock().unwrap_or_else(|e|e.into_inner());pointer.events.clear();pointer.overflow=true;
            return true;
        }
        if q.overflow{return false;}
        if let GameEvent::Look{dx,dy}=event{if let Some((GameEvent::Look{dx:x,dy:y},time))=q.events.back_mut(){
            if time.elapsed().as_millis()>250{*x=0;*y=0;*time=Instant::now();}
            *x=(*x+dx).clamp(-4096,4096);*y=(*y+dy).clamp(-4096,4096);return true;
        }}
        if q.events.len()==256{q.events.clear();q.overflow=true;return false;}
        q.events.push_back((event,Instant::now()));true
    }
    pub(crate) fn drain_game(&self)->Vec<GameEvent>{
        let mut q=self.game.lock().unwrap_or_else(|e|e.into_inner());
        if q.overflow{q.overflow=false;return vec![GameEvent::Reset];}
        let mut output=Vec::new();let mut edges=std::collections::BTreeSet::new();
        while let Some(&(event,time))=q.events.front(){
            let identity=match event{GameEvent::Key{code,..}=>Some(code),GameEvent::Button{button,..}=>Some(1000+button as u32),_=>None};
            if identity.is_some_and(|id|!edges.insert(id)){break;}
            q.events.pop_front();
            if !matches!(event,GameEvent::Look{..})||time.elapsed().as_millis()<=250{output.push(event);}
            if event==GameEvent::Reset{break;}
        }output
    }
    pub(crate) fn stop(&self){self.stopped.store(true,Ordering::Release);}
    pub(crate) fn stopped(&self)->bool{self.stopped.load(Ordering::Acquire)}
    pub(crate) fn push(&self,event:Pointer)->bool{
        if self.stopped()||!matches!(event.kind,0..=3)||!(0..1280).contains(&event.x)||!(0..720).contains(&event.y){return false;}
        let mut queue=self.queue.lock().unwrap_or_else(|error|error.into_inner());
        if queue.overflow{return false;}
        if event.kind==1&&queue.events.back().is_some_and(|last|last.kind==1){*queue.events.back_mut().unwrap()=event;return true;}
        if queue.events.len()==256{queue.events.clear();queue.overflow=true;return false;}
        queue.events.push_back(event);true
    }
    pub(crate) fn drain_frame(&self)->Vec<Pointer>{
        let mut queue=self.queue.lock().unwrap_or_else(|error|error.into_inner());
        if queue.overflow{queue.overflow=false;return vec![Pointer{kind:3,x:0,y:0}];}
        let mut events=Vec::new();let mut transition=false;
        while let Some(next)=queue.events.front(){
            if next.kind!=1&&transition{break;}
            let event=queue.events.pop_front().unwrap();transition|=event.kind!=1;events.push(event);
        }events
    }
    pub(crate) fn frame(&self,frames:u32,draws:u64,presented:bool){
        self.frames.store(frames as u64,Ordering::Relaxed);self.draws.store(draws,Ordering::Relaxed);
        if presented{self.presented.fetch_add(1,Ordering::Relaxed);}
        self.last_frame.store(self.started.elapsed().as_millis() as u64,Ordering::Relaxed);
    }
    pub(crate) fn status(&self)->serde_json::Value{serde_json::json!({"stopped":self.stopped(),"frames":self.frames.load(Ordering::Relaxed),"draw_calls":self.draws.load(Ordering::Relaxed),"presented_frames":self.presented.load(Ordering::Relaxed),"input_callbacks":self.callbacks.load(Ordering::Relaxed),"last_frame_age_ms":(self.started.elapsed().as_millis() as u64).saturating_sub(self.last_frame.load(Ordering::Relaxed))})}
}

#[cfg(any(target_os="android",test))]
pub(crate) mod registry{
    use super::*;
    use std::{collections::BTreeMap,sync::OnceLock};
    static SESSIONS:OnceLock<Mutex<BTreeMap<u64,Arc<Control>>>>=OnceLock::new();
    static NEXT:AtomicU64=AtomicU64::new(1);static RUNNING:AtomicBool=AtomicBool::new(false);
    fn sessions()->&'static Mutex<BTreeMap<u64,Arc<Control>>>{SESSIONS.get_or_init(||Mutex::new(BTreeMap::new()))}
    pub(crate) fn create()->u64{
        let mut entries=sessions().lock().unwrap_or_else(|e|e.into_inner());
        if !entries.is_empty()||RUNNING.load(Ordering::Acquire){return 0;}
        let id=NEXT.fetch_add(1,Ordering::Relaxed);if id==0{return 0;}entries.insert(id,Control::new());id
    }
    pub(crate) fn get(id:u64)->Option<Arc<Control>>{sessions().lock().unwrap_or_else(|e|e.into_inner()).get(&id).cloned()}
    pub(crate) fn release(id:u64){if let Some(control)=sessions().lock().unwrap_or_else(|e|e.into_inner()).remove(&id){control.stop();}}
    pub(crate) fn busy()->bool{RUNNING.load(Ordering::Acquire)||!sessions().lock().unwrap_or_else(|e|e.into_inner()).is_empty()}
    pub(crate) struct Execution;
    impl Execution{pub(crate) fn enter(id:Option<u64>)->anyhow::Result<Self>{
        let entries=sessions().lock().unwrap_or_else(|e|e.into_inner());
        anyhow::ensure!(match id{Some(id)=>entries.contains_key(&id),None=>entries.is_empty()},"Another live session is still active or stopping");
        anyhow::ensure!(RUNNING.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).is_ok(),"Another native test is still active or stopping");Ok(Self)
    }}
    impl Drop for Execution{fn drop(&mut self){RUNNING.store(false,Ordering::Release);}}
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn gameplay_keeps_simultaneous_edges_but_separates_quick_releases(){
        let c=Control::new();for event in [GameEvent::Key{code:87,down:true},GameEvent::Button{button:0,down:true},GameEvent::Look{dx:3,dy:-2},GameEvent::Look{dx:5,dy:4},GameEvent::Key{code:87,down:false}]{assert!(c.push_game(event));}
        let events=c.drain_game();assert_eq!(events.len(),3);assert_eq!(events[2],GameEvent::Look{dx:8,dy:2});
        assert_eq!(c.drain_game(),vec![GameEvent::Key{code:87,down:false}]);
        c.push_game(GameEvent::Look{dx:10,dy:0});c.game.lock().unwrap().events.back_mut().unwrap().1=Instant::now()-std::time::Duration::from_secs(1);
        assert!(c.drain_game().is_empty());
        assert!(GameEvent::from_wire(0,87,2).is_none());assert!(GameEvent::from_wire(2,258,1).is_none());assert!(GameEvent::from_wire(1,i32::MIN,0).is_none());
    }
    #[test]fn gameplay_overflow_and_cancel_clear_stale_input_and_force_all_up(){
        let c=Control::new();for _ in 0..256{assert!(c.push_game(GameEvent::Key{code:87,down:true}));}
        assert!(!c.push_game(GameEvent::Look{dx:1,dy:0}));assert_eq!(c.drain_game(),vec![GameEvent::Reset]);
        c.push_game(GameEvent::Key{code:87,down:true});c.push_game(GameEvent::Reset);assert_eq!(c.drain_game(),vec![GameEvent::Reset]);
        assert!(c.drain_game().is_empty());
    }
    #[test]fn switching_control_mode_discards_pending_menu_touches(){
        let c=Control::new();c.push(Pointer{kind:0,x:100,y:200});c.push(Pointer{kind:2,x:100,y:200});
        c.push_game(GameEvent::Reset);assert_eq!(c.drain_frame(),vec![Pointer{kind:3,x:0,y:0}]);
        assert_eq!(c.drain_game(),vec![GameEvent::Reset]);assert!(c.drain_frame().is_empty());
    }
    #[test]fn taps_keep_press_and_release_in_separate_frames_and_moves_coalesce(){
        let session=Control::new();session.push(Pointer{kind:0,x:10,y:20});
        for x in 11..200{assert!(session.push(Pointer{kind:1,x,y:20}));}
        session.push(Pointer{kind:2,x:199,y:20});
        let first=session.drain_frame();assert_eq!(first.len(),2);assert_eq!(first[1].x,199);
        assert_eq!(session.drain_frame(),vec![Pointer{kind:2,x:199,y:20}]);
        assert!(!session.push(Pointer{kind:0,x:1280,y:0}));session.stop();assert!(!session.push(Pointer{kind:0,x:0,y:0}));
    }
    #[test]fn overflow_forces_a_release_instead_of_leaving_a_held_button(){
        let session=Control::new();for _ in 0..256{assert!(session.push(Pointer{kind:0,x:0,y:0}));}
        assert!(!session.push(Pointer{kind:2,x:0,y:0}));assert_eq!(session.drain_frame()[0].kind,3);assert!(session.drain_frame().is_empty());
    }
    #[test]fn only_one_vm_can_own_a_pending_or_stopping_session(){
        let id=registry::create();assert_ne!(id,0);assert_eq!(registry::create(),0);
        assert!(registry::Execution::enter(None).is_err());let owner=registry::Execution::enter(Some(id)).unwrap();
        assert!(registry::Execution::enter(Some(id)).is_err());registry::get(id).unwrap().stop();drop(owner);
        assert!(registry::busy());assert_eq!(registry::create(),0);registry::release(id);assert!(!registry::busy());
        let next=registry::create();assert!(next>id);assert!(registry::get(id).is_none());registry::release(next);
    }
}
