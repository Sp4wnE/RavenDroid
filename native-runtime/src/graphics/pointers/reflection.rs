use super::*;
pub(super) fn dispatch(caller:&mut Caller<'_,Host>,name:&str,args:&[Val],_results:&mut[Val])->wasmtime::Result<bool>{
    let i=|index|value_i32(args,index);
    match name{
        "env._glGetActiveUniform"|"env._glGetActiveAttrib"=>{
            let length=count(i(2)?,1)?;if length>1024*1024{return Err(host_error("Active GL variable name exceeds bound"));}
            range(caller,i(6)?,length)?;for index in 3..6{if i(index)?!=0{range(caller,i(index)?,4)?;}}
            let mut text=vec![0u8;length.max(1)];let(mut written,mut size,mut kind)=(0i32,0i32,0u32);
            native!(caller,name.strip_prefix("env._").unwrap(),unsafe extern "system" fn(u32,u32,i32,*mut i32,*mut i32,*mut u32,*mut u8);i(0)? as u32,i(1)? as u32,i(2)?,&mut written,&mut size,&mut kind,text.as_mut_ptr());
            if written<0 || written as usize>length.saturating_sub(1){return Err(host_error("Invalid GL variable name length"));}
            for(index,value) in [(3,written),(4,size),(5,kind as i32)]{if i(index)?!=0{output_i32(caller,i(index)?,&[value])?;}}
            if length>0{write(caller,i(6)?,&text[..written as usize+1])?;}
        }
        "env._glGetActiveUniformBlockName"=>{
            let length=count(i(2)?,1)?;if length>1024*1024{return Err(host_error("Uniform block name exceeds bound"));}
            range(caller,i(4)?,length)?;if i(3)?!=0{range(caller,i(3)?,4)?;}
            let mut text=vec![0u8;length.max(1)];let mut written=0;
            native!(caller,"glGetActiveUniformBlockName",unsafe extern "system" fn(u32,u32,i32,*mut i32,*mut u8);i(0)? as u32,i(1)? as u32,i(2)?,&mut written,text.as_mut_ptr());
            if written<0 || written as usize>length.saturating_sub(1){return Err(host_error("Invalid uniform block name length"));}
            if i(3)?!=0{output_i32(caller,i(3)?,&[written])?;}if length>0{write(caller,i(4)?,&text[..written as usize+1])?;}
        }
        "env._glGetActiveUniformBlockiv"=>{
            let parameter=i(2)? as u32;let length=if parameter==0x8A43{
                let mut length=0;native!(caller,"glGetActiveUniformBlockiv",unsafe extern "system" fn(u32,u32,u32,*mut i32);i(0)? as u32,i(1)? as u32,0x8A42,&mut length);count(length,1)?
            }else{1};
            if length>65536{return Err(host_error("Too many uniforms in block"));}range(caller,i(3)?,length*4)?;
            let mut values=vec![0i32;length.max(1)];native!(caller,"glGetActiveUniformBlockiv",unsafe extern "system" fn(u32,u32,u32,*mut i32);i(0)? as u32,i(1)? as u32,parameter,values.as_mut_ptr());
            output_i32(caller,i(3)?,&values[..length])?;
        }
        "env._glGetActiveUniformsiv"=>{
            let length=count(i(1)?,4)?;if length>262144{return Err(host_error("Uniform query batch exceeds bound"));}
            let indices=words(caller,i(2)?,length/4)?;range(caller,i(4)?,length)?;let mut values=vec![0i32;(length/4).max(1)];
            native!(caller,"glGetActiveUniformsiv",unsafe extern "system" fn(u32,i32,*const u32,u32,*mut i32);i(0)? as u32,i(1)?,indices.as_ptr(),i(3)? as u32,values.as_mut_ptr());output_i32(caller,i(4)?,&values[..length/4])?;
        }
        "env._glGetUniformIndices"|"env._glTransformFeedbackVaryings"=>{
            let length=count(i(1)?,1)?;if length>4096{return Err(host_error("GL name batch exceeds bound"));}
            let addresses=words(caller,i(2)?,length)?;let mut total=0usize;let mut texts=Vec::with_capacity(length);
            for address in addresses{let text=c_string(caller,address as i32)?;total+=text.len();if total>16*1024*1024{return Err(host_error("GL name batch text exceeds bound"));}texts.push(text);}
            let names=texts.iter().map(|text|text.as_ptr()).collect::<Vec<_>>();
            if name=="env._glGetUniformIndices"{
                range(caller,i(3)?,length*4)?;let mut indices=vec![u32::MAX;length.max(1)];
                native!(caller,"glGetUniformIndices",unsafe extern "system" fn(u32,i32,*const *const u8,*mut u32);i(0)? as u32,i(1)?,names.as_ptr(),indices.as_mut_ptr());
                write(caller,i(3)?,&indices[..length].iter().flat_map(|value|value.to_le_bytes()).collect::<Vec<_>>())?;
            }else{native!(caller,"glTransformFeedbackVaryings",unsafe extern "system" fn(u32,i32,*const *const u8,u32);i(0)? as u32,i(1)?,names.as_ptr(),i(3)? as u32);}
        }
        _=>return Ok(false),
    }Ok(true)
}
