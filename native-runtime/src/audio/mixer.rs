//! Shared float stereo mixer. The audio callback never enters Wasmtime or decodes files.
use std::{collections::BTreeMap,sync::Arc};
#[derive(Debug)]pub(super) struct Clip{pub data:Vec<f32>,pub channels:usize,pub rate:u32}
impl Clip{pub fn frames(&self)->usize{self.data.len()/self.channels}}
pub(super) struct Voice{pub clip:Arc<Clip>,pub position:f64,pub start:u64,pub stop:Option<u64>,pub ended:bool}
pub(super) struct Channel{pub callback:i32,pub user:i32,pub voice:Option<Voice>,pub gain:f32,pub pitch:f64,pub paused:bool,pub looping:bool,pub loop_start:f64,pub loop_end:f64,pub spatial:bool,pub position:[f64;3]}
impl Channel{pub fn new(callback:i32,user:i32)->Self{Self{callback,user,voice:None,gain:1.,pitch:1.,paused:false,looping:false,loop_start:0.,loop_end:0.,spatial:false,position:[0.;3]}}}
pub(super) struct Mixer{pub channels:BTreeMap<i32,Channel>,pub ended:Vec<(i32,i32)>,pub rate:u32,pub frame:u64,pub listener:[f64;3],pub right:[f64;3],pub frames:u64,pub peak:f32,pub nonzero:u64}
impl Mixer{
 pub fn new(rate:u32)->Self{Self{channels:BTreeMap::new(),ended:Vec::with_capacity(256),rate,frame:0,listener:[0.;3],right:[1.,0.,0.],frames:0,peak:0.,nonzero:0}}
 pub fn render(&mut self,out:&mut[f32]){
  out.fill(0.);let count=out.len()/2;
  for c in self.channels.values_mut(){let Some(v)=c.voice.as_mut() else{continue};if v.ended||c.paused{continue;}
   let n=v.clip.frames();if n==0{v.ended=true;continue;}
   let pan=if c.spatial{let d=[c.position[0]-self.listener[0],c.position[1]-self.listener[1],c.position[2]-self.listener[2]];let len=(d.iter().map(|x|x*x).sum::<f64>()).sqrt();if len>1e-9{d.iter().zip(self.right).map(|(a,b)|a*b).sum::<f64>()/len}else{0.}}else{0.}.clamp(-1.,1.);
   let gains=if c.spatial{[((1.-pan)*0.5).sqrt() as f32,((1.+pan)*0.5).sqrt() as f32]}else{[1.,1.]};
   let end=if c.loop_end>c.loop_start{(c.loop_end*v.clip.rate as f64).min(n as f64)}else{n as f64};
   let begin=(c.loop_start*v.clip.rate as f64).clamp(0.,end);
   let step=c.pitch*v.clip.rate as f64/self.rate as f64;
   for f in 0..count{let now=self.frame+f as u64;if now<v.start{continue;}
    if c.looping && end>begin && v.position>=end{v.position=begin+(v.position-begin)%(end-begin);}
    if v.position>=n as f64||v.stop.is_some_and(|t|now>=t){v.ended=true;break;}
    let idx=v.position as usize;let frac=(v.position-idx as f64) as f32;let next=if c.looping && (idx+1) as f64>=end{begin as usize}else{(idx+1).min(n-1)};
    for side in 0..2{let ch=side.min(v.clip.channels-1);let a=v.clip.data[idx*v.clip.channels+ch];let b=v.clip.data[next*v.clip.channels+ch];out[f*2+side]+=(a+(b-a)*frac)*c.gain*gains[side];}
    v.position+=step;
   }
   if v.ended && c.callback!=0 && self.ended.len()<256{self.ended.push((c.callback,c.user));}
  }
  for x in out.iter_mut(){*x=if x.is_finite(){x.clamp(-1.,1.)}else{0.};self.peak=self.peak.max(x.abs());if *x!=0.{self.nonzero+=1;}}
  self.frame+=count as u64;self.frames+=count as u64;
 }
}
#[cfg(test)]mod tests{
 use super::*;
 fn fixture()->Mixer{let mut m=Mixer::new(4);let mut c=Channel::new(7,99);c.voice=Some(Voice{clip:Arc::new(Clip{data:vec![1.,0.5,-0.5,-1.],channels:1,rate:4}),position:0.,start:2,stop:None,ended:false});m.channels.insert(1,c);m}
 #[test]fn delay_end_and_callback_once(){let mut m=fixture();let mut out=[0.;16];m.render(&mut out);assert_eq!(&out[..6],&[0.,0.,0.,0.,1.,1.]);assert_eq!(m.ended,vec![(7,99)]);m.render(&mut out);assert_eq!(m.ended.len(),1);assert!(out.iter().all(|v|*v==0.));}
 #[test]fn pause_loop_pitch_and_stop(){let mut m=fixture();let c=m.channels.get_mut(&1).unwrap();c.voice.as_mut().unwrap().start=0;c.looping=true;c.pitch=2.;c.paused=true;let mut out=[0.;8];m.render(&mut out);assert_eq!(out,[0.;8]);m.channels.get_mut(&1).unwrap().paused=false;m.render(&mut out);assert_eq!(out,[1.,1.,-0.5,-0.5,1.,1.,-0.5,-0.5]);let now=m.frame;m.channels.get_mut(&1).unwrap().voice.as_mut().unwrap().stop=Some(now);m.render(&mut out);assert_eq!(m.ended.len(),1);}
 #[test]fn spatial_right_and_stereo_channels(){let mut m=fixture();let c=m.channels.get_mut(&1).unwrap();c.voice.as_mut().unwrap().start=0;c.spatial=true;c.position=[1.,0.,0.];let mut out=[0.;2];m.render(&mut out);assert_eq!(out,[0.,1.]);}
}
