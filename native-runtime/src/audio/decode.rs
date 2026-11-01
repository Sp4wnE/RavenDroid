use super::mixer::Clip;
use anyhow::{Result,Context,bail,ensure};
use symphonia::core::{audio::SampleBuffer,codecs::DecoderOptions,formats::FormatOptions,io::MediaSourceStream,meta::MetadataOptions,probe::Hint};
pub(super) fn decode(bytes:std::sync::Arc<[u8]>)->Result<Clip>{
 let source=MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes)),Default::default());
 let mut format=symphonia::default::get_probe().format(&Hint::new(),source,&FormatOptions::default(),&MetadataOptions::default())?.format;
 let track=format.default_track().context("No audio track")?;let id=track.id;let mut decoder=symphonia::default::get_codecs().make(&track.codec_params,&DecoderOptions::default())?;
 let mut data=Vec::new();let mut rate=0;let mut channels=0;
 let mut scratch:Option<SampleBuffer<f32>>=None;let mut capacity=0;
 loop{let packet=match format.next_packet(){Ok(p)=>p,Err(symphonia::core::errors::Error::IoError(e)) if e.kind()==std::io::ErrorKind::UnexpectedEof=>break,Err(e)=>return Err(e.into())};if packet.track_id()!=id{continue;}
  let audio=decoder.decode(&packet)?;let spec=*audio.spec();let n=spec.channels.count();ensure!(n==1||n==2,"Only mono/stereo clips are supported");if rate!=0{ensure!(rate==spec.rate && channels==n,"Audio format changed mid-clip");}rate=spec.rate;channels=n;
  if audio.capacity()>capacity || scratch.is_none(){capacity=audio.capacity();scratch=Some(SampleBuffer::<f32>::new(capacity as u64,spec));}
  let samples=scratch.as_mut().unwrap();samples.copy_interleaved_ref(audio);
  ensure!(data.len()+samples.len()<=24*1024*1024,"Decoded clip exceeds 96 MiB bound");data.extend(samples.samples().iter().map(|s|if s.is_finite(){*s}else{0.}));
 }
 if rate==0||channels==0{bail!("Empty audio stream");}Ok(Clip{data,channels,rate})
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn real_wav_pcm_decode_and_bad_data(){
  let samples=[0i16,16384,-16384,32767];let n=(samples.len()*2) as u32;let mut wav=Vec::new();wav.extend(b"RIFF");wav.extend((n+36).to_le_bytes());wav.extend(b"WAVEfmt ");wav.extend(16u32.to_le_bytes());wav.extend(1u16.to_le_bytes());wav.extend(1u16.to_le_bytes());wav.extend(44100u32.to_le_bytes());wav.extend(88200u32.to_le_bytes());wav.extend(2u16.to_le_bytes());wav.extend(16u16.to_le_bytes());wav.extend(b"data");wav.extend(n.to_le_bytes());for s in samples{wav.extend(s.to_le_bytes());}let c=decode(wav.into()).unwrap();assert_eq!(c.rate,44100);assert_eq!(c.channels,1);assert_eq!(c.frames(),4);assert_eq!(c.data[1],0.5);assert_eq!(c.data[2],-0.5);assert!(decode(vec![0;64].into()).is_err());
 }
}
