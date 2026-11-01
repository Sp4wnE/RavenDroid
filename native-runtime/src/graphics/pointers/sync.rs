//! Guest handles never contain truncated native GLsync pointers.
use super::*;
pub(super) fn dispatch(caller:&mut Caller<'_,Host>,name:&str,args:&[Val],results:&mut[Val])->wasmtime::Result<bool>{
    let i=|index|value_i32(args,index);
    match name{
        "env._glFenceSync"=>{
            let graphics=caller.data().graphics.as_ref().unwrap();
            if graphics.syncs.len()>=4096 || graphics.next_sync==u32::MAX{gl_error(caller,0x505);results[0]=Val::I32(0);return Ok(true);}
            let pointer=native!(caller,"glFenceSync",unsafe extern "system" fn(u32,u32)->*mut c_void;i(0)? as u32,i(1)? as u32);
            if pointer.is_null(){results[0]=Val::I32(0);}else{
                let graphics=caller.data_mut().graphics.as_mut().unwrap();let handle=graphics.next_sync;graphics.next_sync+=1;
                graphics.syncs.insert(handle,pointer as usize);results[0]=Val::I32(handle as i32);
            }
        }
        "env._glDeleteSync"=>{
            if i(0)?==0{return Ok(true);}
            if let Some(pointer)=caller.data_mut().graphics.as_mut().unwrap().syncs.remove(&(i(0)? as u32)){
                native!(caller,"glDeleteSync",unsafe extern "system" fn(*mut c_void);pointer as *mut c_void);
            }else{gl_error(caller,0x501);}
        }
        "env._glClientWaitSync"=>{
            let pointer=caller.data().graphics.as_ref().unwrap().syncs.get(&(i(0)? as u32)).copied();
            if let Some(pointer)=pointer{
                let timeout=(i(2)? as u32 as u64)|((i(3)? as u32 as u64)<<32);
                let status=native!(caller,"glClientWaitSync",unsafe extern "system" fn(*mut c_void,u32,u64)->u32;pointer as *mut c_void,i(1)? as u32,timeout);
                results[0]=Val::I32(status as i32);
            }else{gl_error(caller,0x501);results[0]=Val::I32(0x911D);}
        }
        _=>return Ok(false),
    }Ok(true)
}
