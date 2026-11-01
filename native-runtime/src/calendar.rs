//! Isolated guest calendar uses UTC, with the pinned 32-bit JS time ABI.
use super::{Host,host_error,value_i32,read_u32,write_i32,string_at};
use wasmtime::{Caller,Val};
#[derive(Default)]pub(super) struct Calendar{zone:i32,current:i32}
fn days(year:i64,month:i64,day:i64)->i64{
    let year=year-i64::from(month<=2);let era=year.div_euclid(400);let y=year-era*400;
    let doy=(153*(month+if month>2{-3}else{9})+2)/5+day-1;
    era*146097+y*365+y/4-y/100+doy-719468
}
fn civil(day:i64)->(i64,i64,i64){
    let z=day+719468;let era=z.div_euclid(146097);let doe=z-era*146097;
    let y=(doe-doe/1460+doe/36524-doe/146096)/365;
    let year=y+era*400;let doy=doe-(365*y+y/4-y/100);let mp=(5*doy+2)/153;
    let day=doy-(153*mp+2)/5+1;let month=mp+if mp<10{3}else{-9};(year+i64::from(month<=2),month,day)
}
fn guest_call(caller:&mut Caller<'_,Host>,name:&str,args:&[i32])->wasmtime::Result<i32>{
    let function=caller.get_export(name).and_then(|value|value.into_func()).ok_or_else(||host_error(format!("Guest calendar helper missing: {name}")))?;
    let mut result=[Val::I32(0)];function.call(&mut *caller,&args.iter().copied().map(Val::I32).collect::<Vec<_>>(),&mut result)?;Ok(result[0].i32().unwrap())
}
fn initialize(caller:&mut Caller<'_,Host>)->wasmtime::Result<i32>{
    if caller.data().calendar.zone!=0{return Ok(caller.data().calendar.zone);}
    let zone=guest_call(caller,"_malloc",&[4])?;if zone==0{return Err(host_error("UTC guest string allocation failed"));}
    caller.data().memory.unwrap().write(&mut *caller,zone as u32 as usize,b"UTC\0").map_err(|error|host_error(error.to_string()))?;
    for helper in ["__get_timezone","__get_daylight"]{if caller.get_export(helper).is_some(){let pointer=guest_call(caller,helper,&[])?;write_i32(caller,pointer as u32 as usize,0)?;}}
    if caller.get_export("__get_tzname").is_some(){let pointer=guest_call(caller,"__get_tzname",&[])?;write_i32(caller,pointer as u32 as usize,zone)?;write_i32(caller,pointer as u32 as usize+4,zone)?;}
    caller.data_mut().calendar.zone=zone;Ok(zone)
}
fn epoch(tm:&[i32;11])->i64{
    let mut year=tm[5] as i64+1900;if(0..=99).contains(&year){year+=1900;}// JS Date constructor's historical year rule.
    year+=(tm[4] as i64).div_euclid(12);let month=(tm[4] as i64).rem_euclid(12)+1;
    (days(year,month,1)+tm[3] as i64-1)*86400+tm[2] as i64*3600+tm[1] as i64*60+tm[0] as i64
}
fn broken_down(seconds:i64,zone:i32)->[i32;11]{
    let day=seconds.div_euclid(86400);let clock=seconds.rem_euclid(86400);let(year,month,date)=civil(day);
    [(clock%60) as i32,(clock/60%60) as i32,(clock/3600) as i32,date as i32,month as i32-1,(year-1900) as i32,(day+4).rem_euclid(7) as i32,(day-days(year,1,1)) as i32,0,0,zone]
}
fn format_time(pattern:&str,tm:&[i32;11],zone:&str,depth:u8)->wasmtime::Result<String>{
    if depth>3{return Err(host_error("Calendar format expansion exceeds bound"));}
    if !(0..=60).contains(&tm[0]) || !(0..=59).contains(&tm[1]) || !(0..=23).contains(&tm[2]) || !(1..=31).contains(&tm[3]) || !(0..=11).contains(&tm[4]) || !(0..=6).contains(&tm[6]) || !(0..=365).contains(&tm[7]){return Err(host_error("Invalid broken-down calendar fields"));}
    let weekdays=["Sunday","Monday","Tuesday","Wednesday","Thursday","Friday","Saturday"];
    let months=["January","February","March","April","May","June","July","August","September","October","November","December"];
    let year=tm[5] as i64+1900;let day=days(year,1,1)+tm[7] as i64;
    let iso_day=(day+3).rem_euclid(7)+1;let iso_year=civil(day+4-iso_day).0;
    let jan4=days(iso_year,1,4);let iso_week=(day-(jan4-(jan4+3).rem_euclid(7))).div_euclid(7)+1;
    let mut output=String::new();let mut chars=pattern.chars();
    while let Some(character)=chars.next(){
        if character!='%'{output.push(character);continue;}
        let Some(mut token)=chars.next()else{output.push('%');break;};
        if matches!(token,'E'|'O'){token=chars.next().unwrap_or(token);}
        let expansion=match token{'c'=>Some("%a %b %d %H:%M:%S %Y"),'D'|'x'=>Some("%m/%d/%y"),'F'=>Some("%Y-%m-%d"),'h'=>Some("%b"),'r'=>Some("%I:%M:%S %p"),'R'=>Some("%H:%M"),'T'|'X'=>Some("%H:%M:%S"),_=>None};
        let value=if let Some(pattern)=expansion{format_time(pattern,tm,zone,depth+1)?}else{match token{
            '%'=>"%".into(),'n'=>"\n".into(),'t'=>"\t".into(),
            'a'|'A'=>{let value=weekdays.get(tm[6] as usize).ok_or_else(||host_error("Invalid tm weekday"))?;if token=='a'{value[..3].into()}else{value.to_string()}},
            'b'|'B'=>{let value=months.get(tm[4] as usize).ok_or_else(||host_error("Invalid tm month"))?;if token=='b'{value[..3].into()}else{value.to_string()}},
            'C'=>format!("{:02}",year.div_euclid(100)),'d'=>format!("{:02}",tm[3]),'e'=>format!("{:2}",tm[3]),
            'H'=>format!("{:02}",tm[2]),'I'=>format!("{:02}",if tm[2].rem_euclid(12)==0{12}else{tm[2].rem_euclid(12)}),
            'j'=>format!("{:03}",tm[7]+1),'m'=>format!("{:02}",tm[4]+1),'M'=>format!("{:02}",tm[1]),'S'=>format!("{:02}",tm[0]),
            'p'=>if tm[2]<12{"AM".into()}else{"PM".into()},'w'=>tm[6].to_string(),'u'=>if tm[6]==0{"7".into()}else{tm[6].to_string()},
            'U'=>format!("{:02}",(tm[7]+7-tm[6])/7),'W'=>format!("{:02}",(tm[7]+7-(tm[6]+6).rem_euclid(7))/7),
            'V'=>format!("{iso_week:02}"),'g'=>format!("{:02}",iso_year.rem_euclid(100)),'G'=>iso_year.to_string(),
            'y'=>format!("{:02}",year.rem_euclid(100)),'Y'=>year.to_string(),'Z'=>zone.into(),
            'z'=>{let offset=tm[9] as i64;format!("{}{:02}{:02}",if offset<0{'-'}else{'+'},offset.abs()/3600,offset.abs()%3600/60)},
            _=>format!("%{token}"),
        }};output.push_str(&value);
        if output.len()>1024*1024{return Err(host_error("Calendar formatted text exceeds bound"));}
    }Ok(output)
}
pub(super) fn dispatch(caller:&mut Caller<'_,Host>,name:&str,args:&[Val],results:&mut[Val])->wasmtime::Result<bool>{
    if name=="env._strftime"{
        let target=value_i32(args,0)? as u32 as usize;let capacity=value_i32(args,1)? as u32 as usize;
        let pattern=string_at(caller,value_i32(args,2)? as u32 as usize)?;let pointer=value_i32(args,3)? as u32 as usize;
        let mut tm=[0i32;11];for(index,value)in tm.iter_mut().enumerate(){*value=read_u32(caller,pointer+index*4)? as i32;}
        let zone=if tm[10]!=0{string_at(caller,tm[10] as u32 as usize)?}else{String::new()};let text=format_time(&pattern,&tm,&zone,0)?;
        if text.len()+1>capacity{results[0]=Val::I32(0);}else{
            let bytes=text.bytes().chain([0]).collect::<Vec<_>>();caller.data().memory.unwrap().write(&mut *caller,target,&bytes).map_err(|error|host_error(error.to_string()))?;results[0]=Val::I32(text.len() as i32);
        }return Ok(true);
    }
    if !matches!(name,"env._mktime"|"env._gmtime"|"env._localtime"){return Ok(false);}
    let zone=initialize(caller)?;let pointer=value_i32(args,0)? as u32 as usize;
    if name=="env._mktime"{
        let mut tm=[0i32;11];for(index,value)in tm.iter_mut().enumerate(){*value=read_u32(caller,pointer+index*4)? as i32;}
        let seconds=epoch(&tm);let normalized=broken_down(seconds,zone);
        if tm[8]<0{write_i32(caller,pointer+32,0)?;}
        for(index,value)in [(24,normalized[6]),(28,normalized[7])]{write_i32(caller,pointer+index,value)?;}
        results[0]=Val::I32(if seconds.abs()>8_640_000_000_000{0}else{seconds as i32});
    }else{
        let seconds=read_u32(caller,pointer)? as i32 as i64;
        if caller.data().calendar.current==0{let pointer=guest_call(caller,"_malloc",&[44])?;if pointer==0{return Err(host_error("Guest tm allocation failed"));}caller.data_mut().calendar.current=pointer;}
        let target=caller.data().calendar.current;
        let bytes=broken_down(seconds,zone).iter().flat_map(|value|value.to_le_bytes()).collect::<Vec<_>>();
        caller.data().memory.unwrap().write(&mut *caller,target as u32 as usize,&bytes).map_err(|error|host_error(error.to_string()))?;results[0]=Val::I32(target);
    }Ok(true)
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn calendar_leap_days_negative_epoch_and_month_overflow(){
        assert_eq!(days(1970,1,1),0);assert_eq!(civil(-1),(1969,12,31));
        for year in [1,1900,1969,1970,2000,2024,2038]{for month in 1..=12{assert_eq!(civil(days(year,month,1)),(year,month,1));}}
        assert_eq!(days(2000,3,1)-days(2000,2,1),29);assert_eq!(days(1900,3,1)-days(1900,2,1),28);
        let mut tm=[0;11];tm[5]=70;tm[3]=1;assert_eq!(epoch(&tm),0);tm[4]=-1;assert_eq!(epoch(&tm),-31*86400);
        assert_eq!(&broken_down(-1,0)[..9],&[59,59,23,31,11,69,3,364,0]);
    }
    #[test]fn calendar_formats_english_dates_and_iso_week_year_boundaries(){
        let tm=broken_down(days(2021,1,1)*86400,0);
        assert_eq!(format_time("%F %A %G-W%V-%u %Z %z",&tm,"UTC",0).unwrap(),"2021-01-01 Friday 2020-W53-5 UTC +0000");
        assert_eq!(format_time("%c %%",&broken_down(0,0),"UTC",0).unwrap(),"Thu Jan 01 00:00:00 1970 %");
    }
}
