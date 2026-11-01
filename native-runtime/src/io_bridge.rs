//! Linux/Emscripten wasm32 syscall layouts from the pinned framework.
use super::{Host, host_error, read_u32, string_at, value_i32, filesystem};
use wasmtime::{Caller, Val};

fn scalar(value: Result<i32,i32>) -> i32 { value.unwrap_or_else(|errno|-errno) }

fn read(caller:&mut Caller<'_,Host>, fd:i32, pointer:usize, count:usize) -> i32 {
    if count > i32::MAX as usize {return -filesystem::EINVAL;}
    let Some(end)=pointer.checked_add(count) else {return -14;};
    let memory=caller.data().memory.unwrap();
    let (bytes,host)=memory.data_and_store_mut(&mut *caller);
    let Some(output)=bytes.get_mut(pointer..end) else {return -14;};
    host.filesystem.as_mut().unwrap().read(fd,output).map(|n|n as i32).unwrap_or_else(|e|-e)
}

fn write(caller:&mut Caller<'_,Host>, fd:i32, pointer:usize, count:usize) -> i32 {
    if count > i32::MAX as usize {return -filesystem::EINVAL;}
    let Some(end)=pointer.checked_add(count) else {return -14;};
    let memory=caller.data().memory.unwrap();
    let (bytes,host)=memory.data_and_store_mut(&mut *caller);
    let Some(input)=bytes.get(pointer..end) else {return -14;};
    if fd!=1 && fd!=2 {
        return host.filesystem.as_mut().unwrap().write(fd,input).map(|count|count as i32).unwrap_or_else(|errno|-errno);
    }
    // Bounded stdout/stderr diagnostic sink. All validated bytes are consumed.
    let line=String::from_utf8_lossy(&input[..input.len().min(16*1024)]).into_owned();
    if host.logs.len()==100 {host.logs.remove(0);}
    host.logs.push(line);
    count as i32
}

pub(super) fn dispatch(caller:&mut Caller<'_,Host>, name:&str, args:&[Val]) -> wasmtime::Result<Option<i32>> {
    if caller.data().filesystem.is_none() && name!="env.___syscall122" {return Ok(None);}
    if !matches!(name,"env.___syscall5"|"env.___syscall3"|"env.___syscall4"|"env.___syscall6"|
        "env.___syscall140"|"env.___syscall145"|"env.___syscall146"|"env.___syscall195"|
        "env.___syscall196"|"env.___syscall197"|"env.___syscall33"|"env.___syscall183"|
        "env.___syscall220"|"env.___syscall221"|"env.___syscall54"|"env.___syscall39"|"env.___syscall122") {return Ok(None);}
    let argv=value_i32(args,1)? as u32 as usize;
    let a=read_u32(caller,argv)?;
    let result=match name {
        "env.___syscall220"=>{
            let pointer=read_u32(caller,argv+4)? as usize;let count=read_u32(caller,argv+8)? as usize;
            if count>i32::MAX as usize{-filesystem::EINVAL}else{
                let memory=caller.data().memory.unwrap();let (bytes,host)=memory.data_and_store_mut(&mut *caller);
                match pointer.checked_add(count).and_then(|end|bytes.get_mut(pointer..end)){
                    Some(output)=>host.filesystem.as_mut().unwrap().getdents(a as i32,output).map(|count|count as i32).unwrap_or_else(|errno|-errno),None=>-14,
                }
            }
        }
        "env.___syscall122"=>{
            // Pinned virtual utsname layout and identity, not the host's
            // hostname, account or kernel details.
            let mut bytes=[0u8;390];
            for(offset,value)in [(0,"Emscripten"),(65,"emscripten"),(130,"1.0"),(195,"#1"),(260,"x86-JS")]{bytes[offset..offset+value.len()].copy_from_slice(value.as_bytes());}
            if a==0 || caller.data().memory.unwrap().write(&mut *caller,a as usize,&bytes).is_err(){-14}else{0}
        }
        "env.___syscall39"=>{
            let path=string_at(caller,a as usize)?;
            scalar(caller.data_mut().filesystem.as_mut().unwrap().mkdir(&path))
        }
        "env.___syscall5"=>{
            let path=string_at(caller,a as usize)?;let flags=read_u32(caller,argv+4)?;
            scalar(caller.data_mut().filesystem.as_mut().unwrap().open(&path,flags))
        }
        "env.___syscall6"=>scalar(caller.data_mut().filesystem.as_mut().unwrap().close(a as i32)),
        "env.___syscall3"|"env.___syscall4"=>{
            let ptr=read_u32(caller,argv+4)? as usize;let count=read_u32(caller,argv+8)? as usize;
            if name=="env.___syscall3" {read(caller,a as i32,ptr,count)} else {write(caller,a as i32,ptr,count)}
        }
        "env.___syscall140"=>{
            let high=read_u32(caller,argv+4)? as u64;let low=read_u32(caller,argv+8)? as u64;
            let output=read_u32(caller,argv+12)? as usize;let whence=read_u32(caller,argv+16)?;
            let memory=caller.data().memory.unwrap();
            if output.checked_add(8).is_none_or(|end|end>memory.data_size(&*caller)) {-14}
            else {match caller.data_mut().filesystem.as_mut().unwrap().seek(a as i32,((high<<32)|low) as i64,whence){
                Ok(position)=>{memory.write(&mut *caller,output,&position.to_le_bytes()).map_err(|e|host_error(e.to_string()))?;0},
                Err(errno)=>-errno,
            }}
        }
        "env.___syscall195"|"env.___syscall196"|"env.___syscall197"=>{
            let output=read_u32(caller,argv+4)? as usize;
            let stat=if name=="env.___syscall197" {caller.data().filesystem.as_ref().unwrap().fstat(a as i32)}
                else {let path=string_at(caller,a as usize)?;caller.data().filesystem.as_ref().unwrap().stat(&path)};
            match stat {
                Ok(bytes)=>{let memory=caller.data().memory.unwrap();if memory.write(&mut *caller,output,&bytes).is_ok(){0}else{-14}},
                Err(errno)=>-errno,
            }
        }
        "env.___syscall33"=>{
            let path=string_at(caller,a as usize)?;let mode=read_u32(caller,argv+4)?;
            scalar(caller.data().filesystem.as_ref().unwrap().access(&path,mode))
        }
        "env.___syscall183"=>{
            let size=read_u32(caller,argv+4)?;let memory=caller.data().memory.unwrap();
            if size==0 {-filesystem::EINVAL} else if size<2 {-34}
            else if memory.write(&mut *caller,a as usize,b"/\0").is_err(){-14}else{a as i32}
        }
        "env.___syscall221"=>{
            let cmd=read_u32(caller,argv+4)?;
            let argument=if matches!(cmd,2|4){read_u32(caller,argv+8)?}else{0};
            scalar(caller.data_mut().filesystem.as_mut().unwrap().fcntl(a as i32,cmd,argument))
        }
        "env.___syscall54"=>{
            match caller.data().filesystem.as_ref().unwrap().fstat(a as i32) {Ok(_)=>-25,Err(errno)=>-errno}
        }
        "env.___syscall145"|"env.___syscall146"=>{
            let vectors=read_u32(caller,argv+4)? as usize;let count=read_u32(caller,argv+8)? as usize;
            if count>1024 {-filesystem::EINVAL} else {
                let mut total:i32=0;
                for index in 0..count {
                    let ptr=read_u32(caller,vectors+index*8)? as usize;
                    let length=read_u32(caller,vectors+index*8+4)? as usize;
                    if length>(i32::MAX-total) as usize {if total==0 {total=-filesystem::EINVAL;}break;}
                    let result=if name=="env.___syscall145"{read(caller,a as i32,ptr,length)}else{write(caller,a as i32,ptr,length)};
                    if result<0 {if total==0 {total=result;}break;}
                    total+=result;if (result as usize)<length {break;}
                }
                total
            }
        }
        _=>unreachable!(),
    };
    Ok(Some(result))
}
