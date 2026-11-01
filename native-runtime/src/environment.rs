//! Isolated UTF-8 guest environment. No host process variables are imported.
use super::{Host,host_error,set_errno,string_at,value_i32};
use std::collections::BTreeMap;
use wasmtime::{Caller,Val};

const POOL:usize=1024;
const MAX_ENTRIES:usize=64;
const ARENA:usize=POOL+(MAX_ENTRIES+1)*4;

#[derive(Default)]
pub(super) struct GuestEnvironment {
    entries:BTreeMap<String,String>,
    values:BTreeMap<String,i32>,
    arena:Option<u32>,
}

fn failure(caller:&mut Caller<'_,Host>,errno:i32)->wasmtime::Result<i32>{
    set_errno(caller,errno)?;Ok(-1)
}

fn update(caller:&mut Caller<'_,Host>,entries:BTreeMap<String,String>,destination:Option<usize>)->wasmtime::Result<i32>{
    let size:usize=entries.iter().map(|(key,value)|key.len()+value.len()+2).sum();
    if entries.len()>MAX_ENTRIES || size>POOL {return failure(caller,12);}
    let mut result=[Val::I32(0)];
    let environ=if let Some(destination)=destination{destination}else{
    let get_environ=caller.get_export("__get_environ").and_then(|export|export.into_func())
        .ok_or_else(||host_error("Guest __get_environ missing"))?;
    get_environ.call(&mut *caller,&[],&mut result)?;
    result[0].i32().ok_or_else(||host_error("Invalid __get_environ result"))? as u32 as usize};
    let arena=if let Some(pointer)=caller.data().environment.arena {pointer}else{
        let malloc=caller.get_export("_malloc").and_then(|export|export.into_func())
            .ok_or_else(||host_error("Guest malloc missing for environment"))?;
        malloc.call(&mut *caller,&[Val::I32(ARENA as i32)],&mut result)?;
        let pointer=result[0].i32().ok_or_else(||host_error("Invalid malloc result"))? as u32;
        if pointer==0 {return failure(caller,12);}pointer
    };
    let memory=caller.data().memory.unwrap();
    if (arena as usize).checked_add(ARENA).is_none_or(|end|end>memory.data_size(&*caller))
        || environ.checked_add(4).is_none_or(|end|end>memory.data_size(&*caller)) {
        return Err(host_error("Environment allocation outside guest memory"));
    }
    let mut buffer=vec![0;ARENA];let mut position=0;let mut values=BTreeMap::new();
    for (index,(name,value)) in entries.iter().enumerate(){
        let line=format!("{name}={value}");
        buffer[position..position+line.len()].copy_from_slice(line.as_bytes());
        buffer[POOL+index*4..POOL+index*4+4].copy_from_slice(&(arena+position as u32).to_le_bytes());
        values.insert(name.clone(),(arena+position as u32+name.len() as u32+1) as i32);
        position+=line.len()+1;
    }
    memory.write(&mut *caller,arena as usize,&buffer).map_err(|error|host_error(error.to_string()))?;
    memory.write(&mut *caller,environ,&(arena+POOL as u32).to_le_bytes()).map_err(|error|host_error(error.to_string()))?;
    caller.data_mut().environment=GuestEnvironment{entries,values,arena:Some(arena)};
    Ok(0)
}

pub(super) fn build(caller:&mut Caller<'_,Host>,destination:i32)->wasmtime::Result<()>{
    let mut entries=caller.data().environment.entries.clone();
    if caller.data().environment.arena.is_none(){
        for (key,value) in [("USER","web_user"),("LOGNAME","web_user"),("PATH","/"),("PWD","/"),("HOME","/home/web_user"),("LANG","C.UTF-8"),("_","./ravenfield-native")]{entries.insert(key.into(),value.into());}
    }
    if update(caller,entries,Some(destination as u32 as usize))?!=0{return Err(host_error("Initialize guest environment failed"));}Ok(())
}

pub(super) fn dispatch(caller:&mut Caller<'_,Host>,name:&str,args:&[Val])->wasmtime::Result<Option<i32>>{
    if !matches!(name,"env._getenv"|"env._setenv"|"env._unsetenv"){return Ok(None);}
    let pointer=value_i32(args,0)? as u32 as usize;
    if pointer==0 {return Ok(Some(if name=="env._getenv" {0}else{failure(caller,22)?}));}
    let key=string_at(caller,pointer)?;
    if name=="env._getenv" {return Ok(Some(*caller.data().environment.values.get(&key).unwrap_or(&0)));}
    if key.is_empty() || key.contains('=') {return Ok(Some(failure(caller,22)?));}
    let mut entries=caller.data().environment.entries.clone();
    if name=="env._setenv" {
        if entries.contains_key(&key) && value_i32(args,2)?==0 {return Ok(Some(0));}
        let value_pointer=value_i32(args,1)? as u32 as usize;
        if value_pointer==0 {return Ok(Some(failure(caller,22)?));}
        let value=string_at(caller,value_pointer)?;entries.insert(key,value);
    }else if entries.remove(&key).is_none(){return Ok(Some(0));}
    Ok(Some(update(caller,entries,None)?))
}
