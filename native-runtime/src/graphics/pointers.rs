//! Pointer-bearing GL calls stage outputs or validate a bounded guest range.
//! Native GL never receives an unconverted wasm32 CPU address.
use super::{Host,host_error,value_i32,procedure};
use wasmtime::{Caller,Val};
use std::{ffi::{CStr,c_void},ptr};
mod query;

macro_rules! native {
    ($caller:expr,$name:expr,$signature:ty; $($argument:expr),* $(,)?)=>{{
        let function:$signature=unsafe{std::mem::transmute(procedure($caller,$name)?)};
        // No GL callback is installed that can reenter/grow guest memory.
        unsafe{function($($argument),*)}
    }};
}
mod transfers;
pub(super) use transfers::{vertex_attrib_pointer,draw_elements};
mod reflection;
mod sync;
fn gl_error(caller:&mut Caller<'_,Host>,error:u32){
    let state=caller.data_mut().graphics.as_mut().unwrap();if state.error==0{state.error=error;}
}
fn count(value:i32,multiplier:usize)->wasmtime::Result<usize>{
    if value<0{return Err(host_error("Negative native GL buffer count"));}
    (value as usize).checked_mul(multiplier).filter(|length|*length<=512*1024*1024)
        .ok_or_else(||host_error("Native GL transfer exceeds bounds"))
}
fn range(caller:&Caller<'_,Host>,pointer:i32,length:usize)->wasmtime::Result<*const c_void>{
    let memory=caller.data().memory.unwrap();let start=pointer as u32 as usize;
    let end=start.checked_add(length).ok_or_else(||host_error("GL guest range overflow"))?;
    let data=memory.data(caller);let bytes=data.get(start..end).ok_or_else(||host_error("GL transfer outside guest memory"))?;
    Ok(bytes.as_ptr().cast())
}
fn write(caller:&mut Caller<'_,Host>,pointer:i32,bytes:&[u8])->wasmtime::Result<()>{
    caller.data().memory.unwrap().write(caller,pointer as u32 as usize,bytes).map_err(|error|host_error(error.to_string()))
}
fn words(caller:&Caller<'_,Host>,pointer:i32,length:usize)->wasmtime::Result<Vec<u32>>{
    let bytes=count(length as i32,4)?;let memory=caller.data().memory.unwrap();let start=pointer as u32 as usize;
    let data=memory.data(caller).get(start..start.checked_add(bytes).ok_or_else(||host_error("GL word range overflow"))?).ok_or_else(||host_error("GL word range outside guest memory"))?;
    Ok(data.chunks_exact(4).map(|bytes|u32::from_le_bytes(bytes.try_into().unwrap())).collect())
}
fn c_string(caller:&Caller<'_,Host>,pointer:i32)->wasmtime::Result<Vec<u8>>{
    let memory=caller.data().memory.unwrap();let start=pointer as u32 as usize;let data=memory.data(caller);
    let bytes=data.get(start..).ok_or_else(||host_error("Invalid GL string address"))?;
    let end=bytes.iter().take(1024*1024).position(|byte|*byte==0).ok_or_else(||host_error("Unterminated GL string"))?;
    Ok(bytes[..=end].to_vec())
}
fn output_i32(caller:&mut Caller<'_,Host>,pointer:i32,values:&[i32])->wasmtime::Result<()>{
    let start=pointer as u32 as usize;
    let end=values.len().checked_mul(4).and_then(|n|start.checked_add(n)).ok_or_else(||host_error("GL output range overflow"))?;
    let memory=caller.data().memory.unwrap();
    let output=memory.data_mut(caller).get_mut(start..end).ok_or_else(||host_error("GL output outside guest memory"))?;
    for (bytes,value) in output.chunks_exact_mut(4).zip(values){bytes.copy_from_slice(&value.to_le_bytes());}Ok(())
}
// The measured hot uniform upload uses the same bounds and alignment policy as
// the generic transfer path, without decoding the uniform name each time.
pub(super) fn uniform4fv(caller:&mut Caller<'_,Host>,location:i32,rows:i32,address:i32)->wasmtime::Result<()>{
    uniform_fv(caller,"glUniform4fv",4,location,rows,address)
}
pub(super) fn uniform_fv(caller:&mut Caller<'_,Host>,name:&str,components:usize,location:i32,rows:i32,address:i32)->wasmtime::Result<()>{
    let bytes=count(rows,components*4)?;let staged;
    let pointer=if address as u32%4==0{range(caller,address,bytes)?.cast::<u32>()}
        else{staged=words(caller,address,bytes/4)?;staged.as_ptr()};
    native!(caller,name,unsafe extern "system" fn(i32,i32,*const u32);location,rows,pointer);Ok(())
}
pub(super) fn buffer_sub_data(caller:&mut Caller<'_,Host>,target:i32,offset:i32,size:i32,address:i32)->wasmtime::Result<()>{
    let length=count(size,1)?;let pointer=range(caller,address,length)?;
    native!(caller,"glBufferSubData",unsafe extern "system" fn(u32,isize,isize,*const c_void);target as u32,offset as isize,size as isize,pointer);Ok(())
}
fn scalar(caller:&mut Caller<'_,Host>,name:u32)->wasmtime::Result<i32>{
    let slot=match name{0x8894=>Some(0),0x8895=>Some(1),0x88ED=>Some(2),0x88EF=>Some(3),
        0xD05=>Some(4),0xD02=>Some(5),0xD03=>Some(6),0xD04=>Some(7),
        0xCF5=>Some(8),0xCF2=>Some(9),0xCF3=>Some(10),0xCF4=>Some(11),0x806E=>Some(12),0x806D=>Some(13),_=>None};
    if let Some(slot)=slot{if let Some(value)=caller.data().graphics.as_ref().unwrap().state_cache[slot]{caller.data_mut().graphics.as_mut().unwrap().state_hits+=1;return Ok(value);}}
    let mut value=0i32;
    native!(caller,"glGetIntegerv",unsafe extern "system" fn(u32,*mut i32);name,&mut value);
    if value != 0 && matches!(name, 0x8894 | 0x8895) {
        let object = value as u32;
        let known = &caller.data().graphics.as_ref().unwrap().verified_buffers;
        if known.len() < 32768 && !known.contains(&object) {
            // A non-current VAO can retain a deleted object. glIsBuffer
            // excludes names no longer usable for a fresh explicit bind.
            caller.data_mut().graphics.as_mut().unwrap().state_queries += 1;
            if native!(caller, "glIsBuffer", unsafe extern "system" fn(u32) -> u8; object) != 0 {
                caller.data_mut().graphics.as_mut().unwrap().verified_buffers.insert(object);
            }
        }
    }
    let graphics=caller.data_mut().graphics.as_mut().unwrap();graphics.state_queries+=1;if let Some(slot)=slot{graphics.state_cache[slot]=Some(value);}Ok(value)
}

pub(super) fn dispatch(caller:&mut Caller<'_,Host>,name:&str,args:&[Val],results:&mut[Val])->wasmtime::Result<bool>{
    let i=|index|value_i32(args,index);
    match name {
        "env._glBindFramebuffer"=>{
            let handle=if i(1)?==0{caller.data().graphics.as_ref().unwrap().framebuffer()}else{i(1)? as u32};
            native!(caller,"glBindFramebuffer",unsafe extern "system" fn(u32,u32);i(0)? as u32,handle);
        }
        "env._glReadBuffer"=>{
            let buffer=if i(0)?==0x405 && scalar(caller,0x8CAA)? as u32==caller.data().graphics.as_ref().unwrap().framebuffer(){0x8CE0}else{i(0)? as u32};
            native!(caller,"glReadBuffer",unsafe extern "system" fn(u32);buffer);
        }
        "env._glGetString"|"env._glGetStringi"=>{
            let parameter=match i(0)? as u32{0x9245=>0x1F00,0x9246=>0x1F01,value=>value};
            let index=if name.ends_with('i'){i(1)? as u32}else{u32::MAX};
            if let Some(pointer)=caller.data().graphics.as_ref().unwrap().strings.get(&(parameter,index)).copied(){results[0]=Val::I32(pointer);return Ok(true);}
            let interface=caller.data().graphics.as_ref().unwrap();
            let exposed=interface.interface_extensions.iter().flat_map(|name|[name.clone(),format!("GL_{name}")]).collect::<Vec<_>>();
            let text=if index!=u32::MAX {
                if parameter!=0x1F03{gl_error(caller,0x500);results[0]=Val::I32(0);return Ok(true);}
                let Some(value)=exposed.get(index as usize)else{gl_error(caller,0x501);results[0]=Val::I32(0);return Ok(true);};Some(value.clone())
            }else{match parameter{
                0x1F03=>Some(exposed.join(" ")),
                0x1F02=>Some("OpenGL ES 3.0 (Ravenfield native GLES bridge)".into()),
                0x8B8C=>Some("OpenGL ES GLSL ES 3.00 (Ravenfield native GLES bridge)".into()),
                _=>None,
            }};
            let bytes=if let Some(text)=text{text.bytes().chain([0]).collect::<Vec<_>>()}else{
                let value=native!(caller,"glGetString",unsafe extern "system" fn(u32)->*const u8;parameter);
                if value.is_null(){results[0]=Val::I32(0);return Ok(true);}
                unsafe{CStr::from_ptr(value.cast())}.to_bytes_with_nul().to_vec()
            };
            if bytes.len()>1024*1024{return Err(host_error("Driver GL string exceeds bound"));}
            let malloc=caller.get_export("_malloc").and_then(|value|value.into_func()).ok_or_else(||host_error("Guest malloc missing for GL string"))?;
            let mut pointer=[Val::I32(0)];malloc.call(&mut *caller,&[Val::I32(bytes.len() as i32)],&mut pointer)?;
            let pointer=pointer[0].i32().unwrap();if pointer==0{results[0]=Val::I32(0);return Ok(true);}
            write(caller,pointer,&bytes)?;caller.data_mut().graphics.as_mut().unwrap().strings.insert((parameter,index),pointer);results[0]=Val::I32(pointer);
        }
        "env._glGetIntegerv"=>{
            let parameter=i(0)? as u32;
            let interface_value=match parameter{
                0x821D=>Some(caller.data().graphics.as_ref().unwrap().interface_extensions.len() as i32*2),
                0x821B=>Some(3),0x821C|0x87FE|0x8DF9=>Some(0),_=>None,
            };
            if let Some(value)=interface_value{output_i32(caller,i(1)?,&[value])?;return Ok(true);}
            if matches!(parameter, 0x0C02 | 0x8825) {
                let binding = if parameter == 0x0C02 { 0x8CAA } else { 0x8CA6 };
                let framebuffer = caller.data().graphics.as_ref().unwrap().framebuffer();
                if scalar(caller, binding)? as u32 == framebuffer {
                    let value = scalar(caller, parameter)?;
                    let value = if value == 0x8CE0 { 0x0405 } else { value };
                    output_i32(caller, i(1)?, &[value])?;
                    return Ok(true);
                }
            }
            if matches!(parameter,0x8CA6|0x8CAA){let value=scalar(caller,parameter)?;let value=if value as u32==caller.data().graphics.as_ref().unwrap().framebuffer(){0}else{value};output_i32(caller,i(1)?,&[value])?;return Ok(true);}
            let length=match parameter {
                0x0BA2|0x0C10|0x0C23|0x0C22|0x8005=>4,
                0x0B70|0x0D3A|0x846D|0x846E=>2,
                0x86A3=>count(scalar(caller,0x86A2)?,1)?,
                0x8DF8=>count(scalar(caller,0x8DF9)?,1)?,
                0x87FF=>count(scalar(caller,0x87FE)?,1)?,
                value if value==0x84FF || query::SCALAR.contains(&value)=>1,
                _=>return Err(host_error(format!("Unsupported GL parameter shape: {parameter:#x}"))),
            };
            if length>4096{return Err(host_error("GL parameter array exceeds bound"));}
            range(caller,i(1)?,length*4)?;let mut values=vec![0i32;length.max(1)];
            native!(caller,"glGetIntegerv",unsafe extern "system" fn(u32,*mut i32);parameter,values.as_mut_ptr());output_i32(caller,i(1)?,&values[..length])?;
        }
        "env._glGenBuffers"|"env._glGenFramebuffers"|"env._glGenQueries"|"env._glGenRenderbuffers"|"env._glGenSamplers"|"env._glGenTextures"|"env._glGenTransformFeedbacks"|"env._glGenVertexArrays"|
        "env._glDeleteBuffers"|"env._glDeleteFramebuffers"|"env._glDeleteQueries"|"env._glDeleteRenderbuffers"|"env._glDeleteSamplers"|"env._glDeleteTextures"|"env._glDeleteTransformFeedbacks"|"env._glDeleteVertexArrays"=>{
            if name=="env._glDeleteBuffers"{caller.data_mut().graphics.as_mut().unwrap().index_sizes.clear();}
            let length=count(i(0)?,4)?;if length>4*1024*1024{return Err(host_error("GL object batch exceeds bound"));}
            range(caller,i(1)?,length)?;
            let mut values=if name.contains("Delete"){words(caller,i(1)?,length/4)?}else{vec![0u32;length/4]};
            native!(caller,name.strip_prefix("env._").unwrap(),unsafe extern "system" fn(i32,*mut u32);i(0)?,values.as_mut_ptr());
            if name == "env._glDeleteBuffers" {
                let known = &mut caller.data_mut().graphics.as_mut().unwrap().verified_buffers;
                for id in &values { known.remove(id); }
            }
            if name.contains("Gen"){write(caller,i(1)?,&values.iter().flat_map(|value|value.to_le_bytes()).collect::<Vec<_>>())?;}
        }
        "env._glGetProgramiv"|"env._glGetShaderiv"|"env._glGetTexParameteriv"|"env._glGetRenderbufferParameteriv"|"env._glGetQueryiv"|"env._glGetQueryObjectuiv"|"env._glGetVertexAttribiv"|"env._glGetIntegeri_v"=>{
            let parameter=i(1)? as u32;
            let length=if name=="env._glGetVertexAttribiv" && parameter==0x8626 {4}
                else if name=="env._glGetProgramiv" && parameter==0x8267 {3}
                else if name=="env._glGetTexParameteriv" && matches!(parameter,0x1004|0x8E46){4}else{1};
            range(caller,i(2)?,length*4)?;let mut values=[0i32;4];
            native!(caller,name.strip_prefix("env._").unwrap(),unsafe extern "system" fn(u32,u32,*mut i32);i(0)? as u32,parameter,values.as_mut_ptr());
            output_i32(caller,i(2)?,&values[..length])?;
        }
        "env._glGetFramebufferAttachmentParameteriv"=>{
            let mut value=0;range(caller,i(3)?,4)?;
            native!(caller,"glGetFramebufferAttachmentParameteriv",unsafe extern "system" fn(u32,u32,u32,*mut i32);i(0)? as u32,i(1)? as u32,i(2)? as u32,&mut value);
            output_i32(caller,i(3)?,&[value])?;
        }
        "env._glGetShaderPrecisionFormat"=>{
            let mut limits=[0i32;2];let mut precision=0i32;range(caller,i(2)?,8)?;range(caller,i(3)?,4)?;
            native!(caller,"glGetShaderPrecisionFormat",unsafe extern "system" fn(u32,u32,*mut i32,*mut i32);i(0)? as u32,i(1)? as u32,limits.as_mut_ptr(),&mut precision);
            output_i32(caller,i(2)?,&limits)?;output_i32(caller,i(3)?,&[precision])?;
        }
        "env._glGetInternalformativ"=>{
            let length=count(i(3)?,4)?;if length>16384{return Err(host_error("GL format query exceeds bound"));}
            range(caller,i(4)?,length)?;let mut values=vec![0i32;(length/4).max(1)];
            native!(caller,"glGetInternalformativ",unsafe extern "system" fn(u32,u32,u32,i32,*mut i32);i(0)? as u32,i(1)? as u32,i(2)? as u32,i(3)?,values.as_mut_ptr());
            output_i32(caller,i(4)?,&values[..length/4])?;
        }
        "env._glGetAttribLocation"|"env._glGetUniformLocation"|"env._glGetUniformBlockIndex"=>{
            let text=c_string(caller,i(1)?)?;
            let value=native!(caller,name.strip_prefix("env._").unwrap(),unsafe extern "system" fn(u32,*const u8)->i32;i(0)? as u32,text.as_ptr());results[0]=Val::I32(value);
        }
        "env._glBindAttribLocation"=>{
            let text=c_string(caller,i(2)?)?;
            native!(caller,"glBindAttribLocation",unsafe extern "system" fn(u32,u32,*const u8);i(0)? as u32,i(1)? as u32,text.as_ptr());
        }
        "env._glGetProgramInfoLog"|"env._glGetShaderInfoLog"|"env._glGetShaderSource"=>{
            let length=count(i(1)?,1)?;if length>1024*1024{return Err(host_error("GL text output exceeds bound"));}
            range(caller,i(3)?,length)?;if i(2)?!=0{range(caller,i(2)?,4)?;}
            let mut output=vec![0u8;length];let mut written=0i32;
            native!(caller,name.strip_prefix("env._").unwrap(),unsafe extern "system" fn(u32,i32,*mut i32,*mut u8);i(0)? as u32,i(1)?,&mut written,output.as_mut_ptr());
            if written<0 || written as usize>length.saturating_sub(1){return Err(host_error("Invalid native GL text length"));}
            if i(2)?!=0 {output_i32(caller,i(2)?,&[written])?;}
            if length!=0{write(caller,i(3)?,&output[..written as usize+1])?;}
        }
        "env._glShaderSource"=>{
            let length=count(i(1)?,1)?;if length>4096{return Err(host_error("Too many shader strings"));}
            let addresses=words(caller,i(2)?,length)?;let sizes=if i(3)?==0{vec![u32::MAX;length]}else{words(caller,i(3)?,length)?};
            let mut total=0usize;let mut text=Vec::with_capacity(length);
            for (pointer,size) in addresses.into_iter().zip(sizes){
                let bytes=if (size as i32)<0{let mut bytes=c_string(caller,pointer as i32)?;bytes.pop();bytes}
                    else{let pointer=range(caller,pointer as i32,size as usize)?;unsafe{std::slice::from_raw_parts(pointer.cast::<u8>(),size as usize)}.to_vec()};
                total=total.checked_add(bytes.len()).ok_or_else(||host_error("Shader size overflow"))?;if total>16*1024*1024{return Err(host_error("Shader source exceeds bound"));}text.push(bytes);
            }
            let addresses=text.iter().map(|bytes|bytes.as_ptr()).collect::<Vec<_>>();let lengths=text.iter().map(|bytes|bytes.len() as i32).collect::<Vec<_>>();
            native!(caller,"glShaderSource",unsafe extern "system" fn(u32,i32,*const *const u8,*const i32);i(0)? as u32,i(1)?,addresses.as_ptr(),lengths.as_ptr());
        }
        "env._glBufferData"=>{
            // The same object may be reallocated through a different target.
            // Invalidate its size using the REAL binding, rather than discarding
            // every unrelated index buffer after a vertex/uniform allocation.
            if !caller.data().graphics.as_ref().unwrap().index_sizes.is_empty(){
                let binding=match i(0)? as u32{0x8892=>Some(0x8894),0x8893=>Some(0x8895),0x88EB=>Some(0x88ED),0x88EC=>Some(0x88EF),_=>None};
                if let Some(binding)=binding{let object=scalar(caller,binding)? as u32;caller.data_mut().graphics.as_mut().unwrap().index_sizes.remove(&object);}
                else{caller.data_mut().graphics.as_mut().unwrap().index_sizes.clear();}
            }
            let length=count(i(1)?,1)?;let pointer=if i(2)?==0{ptr::null()}else{range(caller,i(2)?,length)?};
            native!(caller,"glBufferData",unsafe extern "system" fn(u32,isize,*const c_void,u32);i(0)? as u32,i(1)? as isize,pointer,i(3)? as u32);
            if pointer.is_null() && length>0{
                // WebGL allocations are zero initialized; native NULL allocations are undefined.
                // Bound temporary storage independently of the guest's buffer size.
                let zero=vec![0u8;length.min(65536)];
                for offset in (0..length).step_by(zero.len()){
                    let n=(length-offset).min(zero.len());
                    native!(caller,"glBufferSubData",unsafe extern "system" fn(u32,isize,isize,*const c_void);i(0)? as u32,offset as isize,n as isize,zero.as_ptr().cast());
                }
            }
        }
        "env._glBufferSubData"=>{
            buffer_sub_data(caller,i(0)?,i(1)?,i(2)?,i(3)?)?;
        }
        "env._glDrawBuffers"|"env._glInvalidateFramebuffer"=>{
            let (count_index,pointer_index)=if name=="env._glDrawBuffers"{(0,1)}else{(1,2)};
            let length=count(i(count_index)?,4)?;let mut values=words(caller,i(pointer_index)?,length/4)?;
            if name=="env._glDrawBuffers" && scalar(caller,0x8CA6)? as u32==caller.data().graphics.as_ref().unwrap().framebuffer(){for value in &mut values{if *value==0x405{*value=0x8CE0;}}}
            if name=="env._glDrawBuffers"{native!(caller,"glDrawBuffers",unsafe extern "system" fn(i32,*const u32);i(0)?,values.as_ptr());}
            else{
                let binding=if i(0)?==0x8CA8{0x8CAA}else{0x8CA6};
                if scalar(caller,binding)? as u32==caller.data().graphics.as_ref().unwrap().framebuffer(){for v in &mut values{*v=match *v{0x1800=>0x8CE0,0x1801=>0x8D00,0x1802=>0x8D20,x=>x};}}
                native!(caller,"glInvalidateFramebuffer",unsafe extern "system" fn(u32,i32,*const u32);i(0)? as u32,i(1)?,values.as_ptr());}
        }
        _=>{if transfers::dispatch(caller,name,args,results)? || reflection::dispatch(caller,name,args,results)?{return Ok(true);}return sync::dispatch(caller,name,args,results);}
    }
    Ok(true)
}
