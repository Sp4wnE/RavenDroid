//! Bounded owner-thread wall-time measurements. These are not GPU timestamps
//! or automatic gameplay/FPS detection; frame ranges identify loading windows.
use std::collections::VecDeque;
use serde::Serialize;

#[derive(Clone, Default)]
pub(super) struct Sample {
    pub frame:u32, pub work_us:u128, pub present_us:u128,
    pub maintenance_us:u128, pub sleep_us:u128, pub total_us:u128,
    pub draws:u64, pub fuel:u64,
    pub cpu_us:Option<u128>,
}
#[derive(Clone, Serialize)]
pub(super) struct Window {
    first_frame:u32,last_frame:u32,frames:usize,
    work_ms:f64,work_p50_ms:f64,work_p95_ms:f64,work_p99_ms:f64,work_max_ms:f64,
    presentation_ms:f64,maintenance_ms:f64,pacing_sleep_ms:f64,elapsed_ms:f64,
    elapsed_p95_ms:f64,elapsed_p99_ms:f64,elapsed_max_ms:f64,
    over_16_667_ms:usize,over_33_333_ms:usize,over_100_ms:usize,
    draw_calls:u64,fuel_consumed:u128,
    owner_thread_cpu_ms:Option<f64>,
}
fn summarize(samples:&[Sample])->Window {
    let mut times=samples.iter().map(|sample|sample.work_us).collect::<Vec<_>>();times.sort_unstable();
    let mut elapsed=samples.iter().map(|sample|sample.total_us).collect::<Vec<_>>();elapsed.sort_unstable();
    let elapsed_percentile=|p:usize|elapsed[(elapsed.len()*p).div_ceil(100).saturating_sub(1)] as f64/1000.0;
    let percentile=|p:usize|times[(times.len()*p).div_ceil(100).saturating_sub(1)] as f64/1000.0;
    Window{first_frame:samples[0].frame,last_frame:samples.last().unwrap().frame,frames:samples.len(),
        work_ms:samples.iter().map(|s|s.work_us).sum::<u128>() as f64/1000.0,
        work_p50_ms:percentile(50),work_p95_ms:percentile(95),work_p99_ms:percentile(99),work_max_ms:*times.last().unwrap() as f64/1000.0,
        presentation_ms:samples.iter().map(|s|s.present_us).sum::<u128>() as f64/1000.0,
        maintenance_ms:samples.iter().map(|s|s.maintenance_us).sum::<u128>() as f64/1000.0,
        pacing_sleep_ms:samples.iter().map(|s|s.sleep_us).sum::<u128>() as f64/1000.0,
        elapsed_ms:samples.iter().map(|s|s.total_us).sum::<u128>() as f64/1000.0,
        elapsed_p95_ms:elapsed_percentile(95),elapsed_p99_ms:elapsed_percentile(99),elapsed_max_ms:*elapsed.last().unwrap() as f64/1000.0,
        over_16_667_ms:times.iter().filter(|&&v|v>16_667).count(),over_33_333_ms:times.iter().filter(|&&v|v>33_333).count(),over_100_ms:times.iter().filter(|&&v|v>100_000).count(),
        draw_calls:samples.iter().map(|s|s.draws).sum(),fuel_consumed:samples.iter().map(|s|s.fuel as u128).sum(),
        owner_thread_cpu_ms:samples.iter().map(|s|s.cpu_us).collect::<Option<Vec<_>>>().map(|values|values.iter().sum::<u128>() as f64/1000.0)}
}
/// Current owner thread only. Worker/driver threads and GPU are not included.
pub(super) fn thread_cpu_us()->Option<u128>{
    #[cfg(windows)]{
        use windows_sys::Win32::{Foundation::FILETIME,System::Threading::{GetCurrentThread,GetThreadTimes}};
        let(mut created,mut exited,mut kernel,mut user):(FILETIME,FILETIME,FILETIME,FILETIME)=unsafe{std::mem::zeroed()};
        if unsafe{GetThreadTimes(GetCurrentThread(),&mut created,&mut exited,&mut kernel,&mut user)}==0{return None;}
        let ticks=|time:FILETIME|((time.dwHighDateTime as u128)<<32)|time.dwLowDateTime as u128;
        Some((ticks(kernel)+ticks(user))/10)
    }
    #[cfg(target_os="android")]{
        let mut time:libc::timespec=unsafe{std::mem::zeroed()};
        if unsafe{libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID,&mut time)}!=0{return None;}
        Some(time.tv_sec as u128*1_000_000+time.tv_nsec as u128/1000)
    }
    #[cfg(not(any(windows,target_os="android")))]{None}
}
#[derive(Default)]pub(super) struct Performance {
    tail:VecDeque<Sample>,pending:Vec<Sample>,windows:VecDeque<Window>,completed:u64,
    pub(super) android_hint:serde_json::Value,
}
impl Performance {
    pub fn record(&mut self,sample:Sample){
        self.completed+=1;
        if self.tail.len()==300{self.tail.pop_front();}self.tail.push_back(sample.clone());
        self.pending.push(sample);
        if self.pending.len()==120{
            if self.windows.len()==64{self.windows.pop_front();}
            self.windows.push_back(summarize(&self.pending));self.pending.clear();
        }
    }
    pub fn report(&self)->serde_json::Value{
        let tail=self.tail.iter().cloned().collect::<Vec<_>>();
        serde_json::json!({"measurement":"owner-thread wall time; work includes input, guest, host services and driver waits; presentation includes swap/vsync; GPU execution is not isolated; frame ranges are not scene labels",
            "completed_samples":self.completed,"retained_windows":self.windows,"android_performance_hint":self.android_hint,
            "partial_window":(!self.pending.is_empty()).then(||summarize(&self.pending)),
            "last_300_frames":(!tail.is_empty()).then(||summarize(&tail))})
    }
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn windows_keep_loading_separate_from_later_samples_and_stay_bounded(){
        let mut p=Performance::default();
        for frame in 1..=8000 {p.record(Sample{frame,work_us:if frame==1{20_000_000}else{10_000},present_us:1000,total_us:16_667,draws:2,fuel:3,..Default::default()});}
        let r=p.report();assert_eq!(r["completed_samples"],8000);assert_eq!(r["retained_windows"].as_array().unwrap().len(),64);
        assert_eq!(r["last_300_frames"]["first_frame"],7701);assert_eq!(r["last_300_frames"]["work_p95_ms"],10.0);
        assert_eq!(r["last_300_frames"]["draw_calls"],600);assert_eq!(r["last_300_frames"]["fuel_consumed"],900);
        assert_eq!(r["partial_window"]["frames"],80);
    }
    #[test]fn exact_percentiles_empty_and_short_sessions(){
        let mut p=Performance::default();assert!(p.report()["last_300_frames"].is_null());
        for frame in 1..=100{p.record(Sample{frame,work_us:frame as u128*1000,total_us:(frame as u128+10)*1000,..Default::default()});}
        let r=p.report();let w=&r["last_300_frames"];
        assert_eq!(w["work_p50_ms"],50.0);assert_eq!(w["work_p95_ms"],95.0);assert_eq!(w["work_p99_ms"],99.0);
        assert_eq!(w["elapsed_p95_ms"],105.0);assert_eq!(w["elapsed_p99_ms"],109.0);assert_eq!(w["elapsed_max_ms"],110.0);
        assert_eq!(w["over_33_333_ms"],67);assert_eq!(w["over_100_ms"],0);
    }
}
