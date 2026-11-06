//! Pinned Unity WebAudio ABI, decoded clips and an owner-controlled native mixer.
use super::{Host,value_i32,host_error};
use wasmtime::{Caller,Val};
use std::{collections::{BTreeMap,BTreeSet,HashMap},sync::{Arc,Mutex,atomic::{AtomicBool,AtomicU64,AtomicI32,Ordering}},time::Instant,io::{Write,Seek,SeekFrom}};
mod mixer;mod decode;
#[cfg(target_os="android")]mod android;
use mixer::{Clip,Channel,Voice,Mixer};
#[derive(Default)]
struct ChannelUpdate {
    gain: Option<f32>,
    spatial: Option<bool>,
    position: Option<[f64; 3]>,
}

impl ChannelUpdate {
    fn apply(self, channel: &mut Channel) {
        if let Some(gain) = self.gain { channel.gain = gain; }
        if let Some(spatial) = self.spatial { channel.spatial = spatial; }
        if let Some(position) = self.position { channel.position = position; }
    }
}

pub(super) struct State{
 pending_channels: HashMap<i32, ChannelUpdate>,
 queued_channel_updates: u64,
 channel_update_batches: u64,
 clips:BTreeMap<i32,Arc<Clip>>,encoded:BTreeMap<i32,Arc<[u8]>>,last_used:BTreeMap<i32,u64>,use_clock:u64,cache_hits:u64,cache_misses:u64,decode_us:u128,metadata:BTreeMap<i32,(usize,u32)>,cache_evictions:u64,cache_limit:usize,failed:BTreeSet<i32>,next:i32,mixer:Arc<Mutex<Mixer>>,pub muted:Arc<AtomicBool>,pub underruns:Arc<AtomicU64>,
 device_error:Arc<AtomicI32>,device_ready:bool,
 errors:Vec<String>,loads:u64,plays:u64,decode_errors:u64,headers:Vec<String>,formats:BTreeMap<String,u64>,
 offline:Option<std::fs::File>,clock:Instant,written:u64,
 #[cfg(target_os="android")]stream:Option<android::Stream>,
}
impl State{
 pub fn new(path:Option<&std::path::Path>)->anyhow::Result<Self>{let mut offline=path.map(std::fs::File::create).transpose()?;if let Some(f)=offline.as_mut(){f.write_all(&[0;44])?;}
 Ok(Self{pending_channels:Default::default(),queued_channel_updates:0,channel_update_batches:0,clips:Default::default(),encoded:Default::default(),last_used:Default::default(),use_clock:0,cache_hits:0,cache_misses:0,decode_us:0,metadata:Default::default(),cache_evictions:0,cache_limit:256*1024*1024,failed:Default::default(),next:0,mixer:Arc::new(Mutex::new(Mixer::new(48000))),muted:Arc::new(AtomicBool::new(false)),underruns:Arc::new(AtomicU64::new(0)),device_error:Arc::new(AtomicI32::new(0)),device_ready:false,errors:Vec::new(),loads:0,plays:0,decode_errors:0,headers:Vec::new(),formats:Default::default(),offline,clock:Instant::now(),written:0,#[cfg(target_os="android")]stream:None})}
 fn id(&mut self)->i32{self.next=self.next.checked_add(1).unwrap_or(1);self.next}
 fn error(&mut self,e:impl std::fmt::Display){if self.errors.len()<16{self.errors.push(e.to_string());}}
 fn resident_bytes(&self)->usize{
  let mut unique=BTreeMap::new();for c in self.clips.values(){unique.insert(Arc::as_ptr(c) as usize,c.data.len()*4);}
  // Snapshot only fixed-size identities while holding the callback's mutex.
  // Tree insertion and byte accounting must not delay the audio thread.
  let mut active=Vec::with_capacity(256);
  {let m=self.mixer.lock().unwrap();for c in m.channels.values(){if let Some(v)=&c.voice{active.push((Arc::as_ptr(&v.clip) as usize,v.clip.data.len()*4));}}}
  for (identity,bytes) in active{unique.insert(identity,bytes);}unique.values().sum()
 }
 fn cache(&mut self,id:i32,c:Clip)->anyhow::Result<Arc<Clip>>{
  let needed=c.data.len()*4;
  let mut used=self.resident_bytes();
  if used+needed>self.cache_limit{
   let mut evict=self.clips.iter().filter(|(id,c)|self.encoded.contains_key(id)&&Arc::strong_count(c)==1).map(|(id,_)|*id).collect::<Vec<_>>();
   evict.sort_by_key(|id|self.last_used.get(id).copied().unwrap_or(0));
   for old in evict{if used+needed<=self.cache_limit{break;}used-=self.clips.remove(&old).unwrap().data.len()*4;self.last_used.remove(&old);self.cache_evictions+=1;}
  }
  anyhow::ensure!(used+needed<=self.cache_limit,"Active audio exceeds decoded cache budget");
  let c=Arc::new(c);self.clips.insert(id,c.clone());self.touch(id);Ok(c)
 }
 fn add(&mut self,id:i32,clip:anyhow::Result<Clip>,encoded:Option<Arc<[u8]>>){match clip{
  Ok(c)=>{if let Some(bytes)=encoded{
   if self.encoded.values().map(|b|b.len()).sum::<usize>()+bytes.len()>128*1024*1024{self.failed.insert(id);self.error("Encoded audio budget exceeded");return;}
   self.encoded.insert(id,bytes);
  }
  let meta=(c.frames(),c.rate);
  match self.cache(id,c){Ok(_)=>{self.loads+=1;self.metadata.insert(id,meta);},Err(e)=>{self.encoded.remove(&id);self.failed.insert(id);self.error(e);}}
  },Err(e)=>{self.failed.insert(id);self.decode_errors+=1;self.error(format!("Clip {id}: {e:#}"));}
 }}
 fn touch(&mut self,id:i32){self.use_clock=self.use_clock.saturating_add(1);self.last_used.insert(id,self.use_clock);}
 fn clip(&mut self,id:i32)->Option<Arc<Clip>>{
  if let Some(c)=self.clips.get(&id).cloned(){self.cache_hits+=1;self.touch(id);return Some(c);}
  let bytes=self.encoded.get(&id)?.clone();self.cache_misses+=1;let clock=Instant::now();let decoded=decode::decode(bytes);self.decode_us+=clock.elapsed().as_micros();match decoded.and_then(|c|self.cache(id,c)){Ok(c)=>Some(c),Err(e)=>{self.error(e);None}}
 }
 fn start(&mut self){self.clock=Instant::now();#[cfg(target_os="android")]if self.offline.is_none()&&self.stream.is_none(){match android::Stream::open(self.mixer.clone(),self.muted.clone(),self.underruns.clone(),self.device_error.clone()){Ok(s)=>{self.stream=Some(s);self.device_ready=true;},Err(e)=>self.error(e)}}}
 fn channel_update(&mut self, id: i32) -> &mut ChannelUpdate {
    if self.pending_channels.len() >= 256 && !self.pending_channels.contains_key(&id) {
        let mut mixer = self.mixer.lock().unwrap();
        Self::apply_updates(&mut self.pending_channels, &mut mixer);
        self.channel_update_batches += 1;
    }
    self.queued_channel_updates += 1;
    self.pending_channels.entry(id).or_default()
 }

 fn apply_updates(updates: &mut HashMap<i32, ChannelUpdate>, mixer: &mut Mixer) {
    for (id, update) in updates.drain() {
        if let Some(channel) = mixer.channels.get_mut(&id) {
            update.apply(channel);
        }
    }
 }

 pub fn pump(&mut self)->anyhow::Result<Vec<(i32,i32)>>{
  // Publish spatial/volume changes as a frame batch. Offline rendering must
  // see the same updates before producing samples.
  if self.offline.is_some() && !self.pending_channels.is_empty() {
    let mut mixer = self.mixer.lock().unwrap();
    Self::apply_updates(&mut self.pending_channels, &mut mixer);
    self.channel_update_batches += 1;
  }
  if self.offline.is_some(){let due=(self.clock.elapsed().as_secs_f64()*48000.) as u64;let target=due.min(48000*300);let mut scratch=[0.;2048];let mut pcm=[0u8;4096];while self.written/4<target{let frames=(target-self.written/4).min(1024) as usize;self.mixer.lock().unwrap().render(&mut scratch[..frames*2]);for(i,s)in scratch[..frames*2].iter().enumerate(){pcm[i*2..i*2+2].copy_from_slice(&((*s*32767.) as i16).to_le_bytes());}self.offline.as_mut().unwrap().write_all(&pcm[..frames*4])?;self.written+=(frames*4) as u64;}}
  let mut m=self.mixer.lock().unwrap();
  if !self.pending_channels.is_empty() {
    Self::apply_updates(&mut self.pending_channels, &mut m);
    self.channel_update_batches += 1;
  }
  for c in m.channels.values_mut(){if c.voice.as_ref().is_some_and(|v|v.ended){c.voice=None;}}Ok(m.ended.drain(..).collect())
 }
 pub fn report(&mut self)->serde_json::Value{
  if let Some(f)=self.offline.as_mut(){let n=self.written as u32;let mut h=Vec::new();h.extend(b"RIFF");h.extend((n+36).to_le_bytes());h.extend(b"WAVEfmt ");h.extend(16u32.to_le_bytes());h.extend(1u16.to_le_bytes());h.extend(2u16.to_le_bytes());h.extend(48000u32.to_le_bytes());h.extend(192000u32.to_le_bytes());h.extend(4u16.to_le_bytes());h.extend(16u16.to_le_bytes());h.extend(b"data");h.extend(n.to_le_bytes());if let Err(e)=f.seek(SeekFrom::Start(0)).and_then(|_|f.write_all(&h)).and_then(|_|f.flush()){self.error(e);}}
  let resident_bytes=self.resident_bytes();let m=self.mixer.lock().unwrap();serde_json::json!({"backend":if self.offline.is_some(){"offline-pcm-wav"}else{if self.device_ready{"AAudio-float-stereo"}else{"mixer-only"}},"queued_channel_updates":self.queued_channel_updates,"channel_update_batches":self.channel_update_batches,"loaded_clips":self.loads,"resident_clips":self.clips.len(),"available_clips":self.metadata.len(),"cache_evictions":self.cache_evictions,"cache_hits":self.cache_hits,"cache_misses":self.cache_misses,"on_demand_decode_ms":self.decode_us as f64/1000.,"decoded_resident_bytes":resident_bytes,"encoded_bytes":self.encoded.values().map(|b|b.len()).sum::<usize>(),"decode_failures":self.decode_errors,"sample_headers":self.headers,"plays":self.plays,"formats":self.formats,"rendered_frames":m.frames,"nonzero_samples":m.nonzero,"peak":m.peak,"output_rate":m.rate,"device_error":self.device_error.load(Ordering::Relaxed),"muted":self.muted.load(Ordering::Relaxed),"underruns":self.underruns.load(Ordering::Relaxed),"errors":self.errors})
 }
}
fn f(args:&[Val],n:usize)->wasmtime::Result<f64>{let x=args.get(n).and_then(Val::f64).ok_or_else(||host_error("Expected audio f64"))?;if !x.is_finite(){return Err(host_error("Nonfinite audio value"));}Ok(x)}
pub(super) fn dispatch(caller:&mut Caller<'_,Host>,name:&str,args:&[Val],results:&mut[Val])->wasmtime::Result<bool>{
 if !name.starts_with("env._JS_Sound_"){return Ok(false);}
 if caller.data().audio.is_none(){return Ok(disabled(caller,name,results));}
 let i=|n|value_i32(args,n);
 let mut value=None;
 match name{
  "env._JS_Sound_Init"=>caller.data_mut().audio.as_mut().unwrap().start(),
  "env._JS_Sound_ResumeIfNeeded"=>{},
  "env._JS_Sound_Load"|"env._JS_Sound_Load_PCM"=>{
   let audio=caller.data_mut().audio.as_mut().unwrap();let id=audio.id();value=Some(id);
   let memory=caller.data().memory.ok_or_else(||host_error("Audio memory unavailable"))?;
   let mut encoded=None;
   let clip=if name.ends_with("Load_PCM"){
    let channels=i(0)? as usize;let frames=i(1)? as usize;let rate=i(2)? as u32;
    if !(1..=2).contains(&channels)||frames>24*1024*1024/channels||!(8000..=192000).contains(&rate){return Err(host_error("Invalid PCM clip dimensions"));}
    let ptr=i(3)? as u32 as usize;let bytes=frames*channels*4;let data=memory.data(&*caller).get(ptr..ptr.checked_add(bytes).ok_or_else(||host_error("PCM overflow"))?).ok_or_else(||host_error("PCM outside guest memory"))?;
    let mut out=vec![0.;frames*channels];for ch in 0..channels{for frame in 0..frames{let at=(ch*frames+frame)*4;let sample=f32::from_le_bytes(data[at..at+4].try_into().unwrap());out[frame*channels+ch]=if sample.is_finite(){sample}else{0.};}}Ok(Clip{data:out,channels,rate})
   }else{
    let ptr=i(0)? as u32 as usize;let length=i(1)? as u32 as usize;if length>32*1024*1024{return Err(host_error("Encoded clip exceeds bound"));}
    let bytes=memory.data(&*caller).get(ptr..ptr.checked_add(length).ok_or_else(||host_error("Audio overflow"))?).ok_or_else(||host_error("Encoded clip outside guest memory"))?.to_vec();
    {let a=caller.data_mut().audio.as_mut().unwrap();if a.headers.len()<8{a.headers.push(format!("length={} header={:02x?}",length,&bytes[..bytes.len().min(24)]));}}
    let format=if bytes.get(4..8)==Some(b"ftyp"){"mp4"}else if bytes.starts_with(b"RIFF"){"wav"}else if bytes.starts_with(b"OggS"){"ogg"}else if bytes.starts_with(b"ID3")||bytes.first()==Some(&255){"mp3"}else{"unknown"};*caller.data_mut().audio.as_mut().unwrap().formats.entry(format.into()).or_default()+=1;let bytes:Arc<[u8]>=bytes.into();let decoded=decode::decode(bytes.clone());encoded=Some(bytes);decoded
   };caller.data_mut().audio.as_mut().unwrap().add(id,clip,encoded);
  }
  "env._JS_Sound_GetLoadState"=>{let a=caller.data().audio.as_ref().unwrap();value=Some(if a.metadata.contains_key(&i(0)?){0}else{2});}
  "env._JS_Sound_GetLength"=>{value=Some(caller.data().audio.as_ref().unwrap().metadata.get(&i(0)?).map(|(frames,rate)|(*frames as u64*44100/(*rate as u64)) as i32).unwrap_or(0));}
  "env._JS_Sound_Create_Channel"=>{let cb=i(0)?;let user=i(1)?;let a=caller.data_mut().audio.as_mut().unwrap();let id=a.id();let mut m=a.mixer.lock().unwrap();if m.channels.len()<256{m.channels.insert(id,Channel::new(cb,user));value=Some(id);}else{value=Some(0);}}
  "env._JS_Sound_ReleaseInstance"=>{let id=i(0)?;let a=caller.data_mut().audio.as_mut().unwrap();a.pending_channels.remove(&id);a.clips.remove(&id);a.last_used.remove(&id);a.encoded.remove(&id);a.metadata.remove(&id);a.failed.remove(&id);let mut m=a.mixer.lock().unwrap();m.channels.remove(&id);value=Some(0);}
  "env._JS_Sound_Play"=>{let clip=i(0)?;let id=i(1)?;let offset=f(args,2)?.max(0.);let delay=f(args,3)?.max(0.);let a=caller.data_mut().audio.as_mut().unwrap();let clip=a.clip(clip);let mut m=a.mixer.lock().unwrap();let start=m.frame+(delay*m.rate as f64) as u64;if let (Some(clip),Some(c))=(clip,m.channels.get_mut(&id)){if let Some(update)=a.pending_channels.remove(&id){update.apply(c);}c.voice=Some(Voice{clip:clip.clone(),position:offset*clip.rate as f64,start,stop:None,ended:false});c.paused=false;a.plays+=1;}}
  "env._JS_Sound_IsStopped"=>{let m=caller.data().audio.as_ref().unwrap().mixer.lock().unwrap();value=Some(m.channels.get(&i(0)?).is_none_or(|c|c.voice.as_ref().is_none_or(|v|v.ended)) as i32);}
  "env._JS_Sound_SetListenerPosition"=>{caller.data().audio.as_ref().unwrap().mixer.lock().unwrap().listener=[f(args,0)?,f(args,1)?,f(args,2)?];}
  "env._JS_Sound_SetListenerOrientation"=>{let forward=[f(args,0)?,f(args,1)?,f(args,2)?];let up=[f(args,3)?,f(args,4)?,f(args,5)?];let r=[forward[1]*up[2]-forward[2]*up[1],forward[2]*up[0]-forward[0]*up[2],forward[0]*up[1]-forward[1]*up[0]];let len=r.iter().map(|v|v*v).sum::<f64>().sqrt();if len>0.{caller.data().audio.as_ref().unwrap().mixer.lock().unwrap().right=r.map(|x|x/len);}}
  "env._JS_Sound_Set3D" | "env._JS_Sound_SetPosition" | "env._JS_Sound_SetVolume" => {
   let id = i(0)?;
   let audio = caller.data_mut().audio.as_mut().unwrap();
   match name {
    "env._JS_Sound_Set3D" => audio.channel_update(id).spatial = Some(i(1)? != 0),
    "env._JS_Sound_SetPosition" => {
        let position = [f(args,1)?, f(args,2)?, f(args,3)?];
        audio.channel_update(id).position = Some(position);
    }
    _ => audio.channel_update(id).gain = Some(f(args,1)?.clamp(0.,4.) as f32),
   }
  }
  "env._JS_Sound_SetLoop"|"env._JS_Sound_SetLoopPoints"|"env._JS_Sound_SetPaused"|"env._JS_Sound_SetPitch"|"env._JS_Sound_Stop"=>{
   let id=i(0)?;let mut m=caller.data().audio.as_ref().unwrap().mixer.lock().unwrap();let now=m.frame;let rate=m.rate;
   if let Some(c)=m.channels.get_mut(&id){match name{
    "env._JS_Sound_SetLoop"=>c.looping=i(1)?!=0,"env._JS_Sound_SetPaused"=>c.paused=i(1)?!=0,
    "env._JS_Sound_SetLoopPoints"=>{c.loop_start=f(args,1)?.max(0.);c.loop_end=f(args,2)?.max(0.);},
    "env._JS_Sound_SetPitch"=>c.pitch=f(args,1)?.clamp(0.01,16.),
    "env._JS_Sound_Stop"=>{let delay=f(args,1)?;if delay<=0.{c.voice=None;}else if let Some(v)=c.voice.as_mut(){v.stop=Some(now+(delay*rate as f64) as u64);}},_=>{},
   }}
  }
  _=>return Ok(false),
 }
 if let Some(v)=value{results[0]=Val::I32(v);}Ok(true)
}
fn disabled(caller:&mut Caller<'_,Host>,name:&str,results:&mut[Val])->bool{
 if !caller.data().headless_audio{return false;}
 let v=match name{"env._JS_Sound_GetLoadState"=>Some(2),"env._JS_Sound_IsStopped"=>Some(1),"env._JS_Sound_Create_Channel"|"env._JS_Sound_GetLength"|"env._JS_Sound_Load"|"env._JS_Sound_Load_PCM"|"env._JS_Sound_ReleaseInstance"=>Some(0),"env._JS_Sound_Init"|"env._JS_Sound_Play"|"env._JS_Sound_ResumeIfNeeded"|"env._JS_Sound_SetListenerOrientation"|"env._JS_Sound_SetListenerPosition"|"env._JS_Sound_SetLoop"|"env._JS_Sound_SetLoopPoints"|"env._JS_Sound_SetPaused"|"env._JS_Sound_SetPitch"|"env._JS_Sound_SetPosition"|"env._JS_Sound_SetVolume"|"env._JS_Sound_Stop"=>None,_=>return false};
 caller.data_mut().unavailable_services.insert(name.into());if let Some(v)=v{results[0]=Val::I32(v);}true
}

#[cfg(all(test,feature="compiler"))]mod tests{
 use super::*;
 #[test]
 fn channel_updates_publish_latest_values_in_one_batch() {
    let mut audio = State::new(None).unwrap();
    audio.mixer.lock().unwrap().channels.insert(1, Channel::new(0, 0));
    audio.channel_update(1).gain = Some(0.25);
    audio.channel_update(1).gain = Some(0.75);
    audio.channel_update(1).position = Some([1., 2., 3.]);
    audio.channel_update(1).spatial = Some(true);
    audio.channel_update(999).gain = Some(0.);
    audio.pump().unwrap();
    let mixer = audio.mixer.lock().unwrap();
    assert_eq!(mixer.channels.len(), 1);
    assert_eq!(mixer.channels[&1].gain, 0.75);
    assert_eq!(mixer.channels[&1].position, [1., 2., 3.]);
    assert!(mixer.channels[&1].spatial);
    assert_eq!(audio.queued_channel_updates, 5);
    assert_eq!(audio.channel_update_batches, 1);
 }
 #[test]fn cache_keeps_recently_played_clip_instead_of_lowest_id(){
  let mut a=State::new(None).unwrap();a.cache_limit=128;
  for id in 1..=2{a.add(id,Ok(Clip{data:vec![0.5;16],channels:1,rate:44100}),Some(vec![1].into()));}
  drop(a.clip(1).unwrap());
  a.add(3,Ok(Clip{data:vec![0.5;16],channels:1,rate:44100}),Some(vec![1].into()));
  assert!(a.clips.contains_key(&1));assert!(!a.clips.contains_key(&2));assert_eq!(a.cache_hits,1);
  assert_eq!(a.metadata.len(),3);assert_eq!(a.resident_bytes(),128);
 }
 #[test]fn cache_evicts_idle_clips_but_preserves_metadata_and_playing_voices(){
  let mut a=State::new(None).unwrap();a.cache_limit=128;
  for id in 1..=3{a.add(id,Ok(Clip{data:vec![0.5;16],channels:1,rate:44100}),Some(vec![1].into()));}
  assert_eq!(a.metadata.len(),3);assert!(!a.clips.contains_key(&1));assert!(a.clips.contains_key(&3));assert_eq!(a.cache_evictions,1);
  let active=a.clips[&2].clone();a.cache(4,Clip{data:vec![0.25;16],channels:1,rate:44100}).unwrap();assert!(a.clips.contains_key(&2));assert!(!a.clips.contains_key(&3));assert_eq!(active.data[0],0.5);
 }
 #[test]fn guest_pcm_is_planar_length_is_44100_based_and_callbacks_stay_queued(){
  let (mut store,instance)=crate::tests::fixture(r#"(module (import "env" "memory" (memory 512))
   (import "env" "_JS_Sound_Load_PCM" (func $load (param i32 i32 i32 i32) (result i32)))
   (import "env" "_JS_Sound_GetLength" (func $len (param i32) (result i32)))
   (import "env" "_JS_Sound_GetLoadState" (func $state (param i32) (result i32)))
   (import "env" "_JS_Sound_Create_Channel" (func $channel (param i32 i32) (result i32)))
   (import "env" "_JS_Sound_Play" (func $play (param i32 i32 f64 f64)))
   (func (export "load") (result i32) i32.const 2 i32.const 4 i32.const 22050 i32.const 1024 call $load)
   (func (export "length") (result i32) i32.const 1 call $len)
   (func (export "state") (result i32) i32.const 1 call $state)
   (func (export "play") i32.const 7 i32.const 99 call $channel drop i32.const 1 i32.const 2 f64.const 0 f64.const 0 call $play))"#);
  store.data_mut().audio=Some(State::new(None).unwrap());let memory=store.data().memory.unwrap();let planar=[0.25f32,0.5,0.75,1.,-0.25,-0.5,-0.75,-1.];memory.write(&mut store,1024,&planar.iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>()).unwrap();
  assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"load").unwrap().call(&mut store,()).unwrap(),1);assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"length").unwrap().call(&mut store,()).unwrap(),8);assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"state").unwrap().call(&mut store,()).unwrap(),0);
  instance.get_typed_func::<(),()>(&mut store,"play").unwrap().call(&mut store,()).unwrap();let a=store.data_mut().audio.as_mut().unwrap();assert_eq!(a.clips[&1].data[..4],[0.25,-0.25,0.5,-0.5]);let mut out=[0.;40];a.mixer.lock().unwrap().render(&mut out);assert_eq!(a.pump().unwrap(),vec![(7,99)]);
 }
}
