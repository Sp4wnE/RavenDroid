//! CPU transfers are bounded linear-memory slices; GPU offsets require a bound buffer.
use super::*;

#[derive(Clone,Copy)]
struct Layout {alignment:usize,row:usize,image:usize,skip_pixels:usize,skip_rows:usize,skip_images:usize}
fn footprint(width:usize,height:usize,depth:usize,bpp:usize,layout:Layout)->wasmtime::Result<usize>{
    if width==0 || height==0 || depth==0{return Ok(0);}
    let calculate=||{
        let row=if layout.row==0{width}else{layout.row}.checked_mul(bpp)?;
        let pitch=row.checked_add(layout.alignment-1)? / layout.alignment * layout.alignment;
        let image=if layout.image==0{height}else{layout.image}.checked_mul(pitch)?;
        layout.skip_images.checked_add(depth-1)?.checked_mul(image)?
            .checked_add(layout.skip_rows.checked_add(height-1)?.checked_mul(pitch)?)?
            .checked_add(layout.skip_pixels.checked_add(width)?.checked_mul(bpp)?)
    };
    calculate().filter(|length|*length<=512*1024*1024).ok_or_else(||host_error("Pixel transfer exceeds bounds"))
}
fn pixel_size(format:u32,kind:u32)->wasmtime::Result<usize>{
    let components=match format{0x1906|0x1909|0x1902|0x1903|0x8D94=>1,0x190A|0x8227|0x8228=>2,0x1907|0x8D98=>3,0x1908|0x8D99=>4,0x84F9=>1,_=>return Err(host_error(format!("Unsupported pixel format {format:#x}")))};
    Ok(match kind{
        0x1400|0x1401=>components,0x1402|0x1403|0x140B|0x8D61=>components*2,
        0x1404|0x1405|0x1406=>components*4,
        0x8363|0x8033|0x8034=>2,0x8368|0x8C3B|0x8C3E|0x84FA=>4,0x8DAD=>8,
        _=>return Err(host_error(format!("Unsupported pixel type {kind:#x}"))),
    })
}
fn pixel_length(caller:&mut Caller<'_,Host>,width:i32,height:i32,depth:i32,format:u32,kind:u32,pack:bool,three:bool)->wasmtime::Result<usize>{
    let width=count(width,1)?;let height=count(height,1)?;let depth=count(depth,1)?;
    let base=if pack{0xD00}else{0xCF0};
    let alignment=scalar(caller,base+5)? as usize;
    if !matches!(alignment,1|2|4|8){return Err(host_error("Invalid driver pixel alignment"));}
    let layout=Layout{alignment,row:count(scalar(caller,base+2)?,1)?,skip_rows:count(scalar(caller,base+3)?,1)?,skip_pixels:count(scalar(caller,base+4)?,1)?,
        image:if three{count(scalar(caller,0x806E)?,1)?}else{0},skip_images:if three{count(scalar(caller,0x806D)?,1)?}else{0}};
    footprint(width,height,depth,pixel_size(format,kind)?,layout)
}
fn buffer_range(caller:&mut Caller<'_,Host>,target:u32,offset:i32,length:usize)->wasmtime::Result<*const c_void>{
    let mut size=0i32;native!(caller,"glGetBufferParameteriv",unsafe extern "system" fn(u32,u32,*mut i32);target,0x8764,&mut size);
    let offset=offset as u32 as usize;
    if size<0 || offset.checked_add(length).is_none_or(|end|end>size as usize){return Err(host_error("GPU buffer transfer exceeds allocation"));}
    Ok(offset as *const c_void)
}
fn index_range(caller:&mut Caller<'_,Host>,buffer:u32,offset:i32,length:usize)->wasmtime::Result<*const c_void>{
    let cached=caller.data().graphics.as_ref().unwrap().index_sizes.get(&buffer).copied();
    let size=if let Some(size)=cached{
        caller.data_mut().graphics.as_mut().unwrap().index_size_hits+=1;size
    }else{
        let mut size=0i32;
        native!(caller,"glGetBufferParameteriv",unsafe extern "system" fn(u32,u32,*mut i32);0x8893,0x8764,&mut size);
        if size<0{return Err(host_error("Invalid index buffer size"));}
        let graphics=caller.data_mut().graphics.as_mut().unwrap();graphics.index_size_queries+=1;
        if graphics.index_sizes.len()>=16384{graphics.index_sizes.clear();}
        graphics.index_sizes.insert(buffer,size as usize);size as usize
    };
    let offset=offset as u32 as usize;
    if offset.checked_add(length).is_none_or(|end|end>size){return Err(host_error("GPU buffer transfer exceeds allocation"));}
    Ok(offset as *const c_void)
}
pub(in crate::graphics) fn vertex_attrib_pointer(caller:&mut Caller<'_,Host>,index:i32,components:i32,kind:i32,normalized:i32,stride:i32,offset:i32)->wasmtime::Result<()>{
    if scalar(caller,0x8894)?==0 {gl_error(caller,0x502);return Ok(());}
    native!(caller,"glVertexAttribPointer",unsafe extern "system" fn(u32,i32,u32,u8,i32,*const c_void);index as u32,components,kind as u32,(normalized!=0) as u8,stride,offset as u32 as usize as *const c_void);
    Ok(())
}
pub(in crate::graphics) fn draw_elements(caller:&mut Caller<'_,Host>,mode:i32,elements:i32,kind:i32,offset:i32,instances:Option<i32>)->wasmtime::Result<()>{
    let element_buffer=scalar(caller,0x8895)? as u32;
    if element_buffer==0 {gl_error(caller,0x502);return Ok(());}
    let size=match kind as u32{0x1401=>1,0x1403=>2,0x1405=>4,_=>{gl_error(caller,0x500);return Ok(());}};
    if elements<0 || instances.is_some_and(|n|n<0){gl_error(caller,0x501);return Ok(());}
    if (offset as u32 as usize)%size!=0{gl_error(caller,0x502);return Ok(());}
    let pointer=index_range(caller,element_buffer,offset,count(elements,size)?)?;
    if let Some(instances)=instances{native!(caller,"glDrawElementsInstanced",unsafe extern "system" fn(u32,i32,u32,*const c_void,i32);mode as u32,elements,kind as u32,pointer,instances);}
    else{native!(caller,"glDrawElements",unsafe extern "system" fn(u32,i32,u32,*const c_void);mode as u32,elements,kind as u32,pointer);}
    caller.data_mut().graphics.as_mut().unwrap().draw_calls+=1;
    Ok(())
}
fn pixels(caller:&mut Caller<'_,Host>,pointer:i32,length:usize,null_allocation:bool)->wasmtime::Result<*const c_void>{
    if scalar(caller,0x88EF)?!=0 {buffer_range(caller,0x88EC,pointer,length)}
    else if pointer==0 && null_allocation{Ok(ptr::null())}
    else{range(caller,pointer,length)}
}

pub(super) fn dispatch(caller:&mut Caller<'_,Host>,name:&str,args:&[Val],_results:&mut[Val])->wasmtime::Result<bool>{
    let i=|index|value_i32(args,index);
    match name {
        "env._glTexImage2D"|"env._glTexSubImage2D"|"env._glTexImage3D"|"env._glTexSubImage3D"=>{
            let image=name.contains("TexImage");let three=name.ends_with("3D");
            let (w,h,d,f,t,p)=match (image,three){(true,false)=>(3,4,None,6,7,8),(false,false)=>(4,5,None,6,7,8),(true,true)=>(3,4,Some(5),7,8,9),(false,true)=>(5,6,Some(7),8,9,10)};
            let length=pixel_length(caller,i(w)?,i(h)?,if let Some(d)=d{i(d)?}else{1},i(f)? as u32,i(t)? as u32,false,three)?;
            let pointer=pixels(caller,i(p)?,length,image)?;
            match (image,three){
                (true,false)=>native!(caller,"glTexImage2D",unsafe extern "system" fn(u32,i32,i32,i32,i32,i32,u32,u32,*const c_void);i(0)? as u32,i(1)?,i(2)?,i(3)?,i(4)?,i(5)?,i(6)? as u32,i(7)? as u32,pointer),
                (false,false)=>native!(caller,"glTexSubImage2D",unsafe extern "system" fn(u32,i32,i32,i32,i32,i32,u32,u32,*const c_void);i(0)? as u32,i(1)?,i(2)?,i(3)?,i(4)?,i(5)?,i(6)? as u32,i(7)? as u32,pointer),
                (true,true)=>native!(caller,"glTexImage3D",unsafe extern "system" fn(u32,i32,i32,i32,i32,i32,i32,u32,u32,*const c_void);i(0)? as u32,i(1)?,i(2)?,i(3)?,i(4)?,i(5)?,i(6)?,i(7)? as u32,i(8)? as u32,pointer),
                (false,true)=>native!(caller,"glTexSubImage3D",unsafe extern "system" fn(u32,i32,i32,i32,i32,i32,i32,i32,u32,u32,*const c_void);i(0)? as u32,i(1)?,i(2)?,i(3)?,i(4)?,i(5)?,i(6)?,i(7)?,i(8)? as u32,i(9)? as u32,pointer),
            }
        }
        "env._glCompressedTexImage2D"|"env._glCompressedTexSubImage2D"|"env._glCompressedTexImage3D"|"env._glCompressedTexSubImage3D"=>{
            let image=name.contains("TexImage");let three=name.ends_with("3D");let size=args.len()-2;
            let length=count(i(size)?,1)?;let pointer=pixels(caller,i(size+1)?,length,image)?;
            match (image,three){
                (true,false)=>native!(caller,"glCompressedTexImage2D",unsafe extern "system" fn(u32,i32,u32,i32,i32,i32,i32,*const c_void);i(0)? as u32,i(1)?,i(2)? as u32,i(3)?,i(4)?,i(5)?,i(6)?,pointer),
                (false,false)=>native!(caller,"glCompressedTexSubImage2D",unsafe extern "system" fn(u32,i32,i32,i32,i32,i32,u32,i32,*const c_void);i(0)? as u32,i(1)?,i(2)?,i(3)?,i(4)?,i(5)?,i(6)? as u32,i(7)?,pointer),
                (true,true)=>native!(caller,"glCompressedTexImage3D",unsafe extern "system" fn(u32,i32,u32,i32,i32,i32,i32,i32,*const c_void);i(0)? as u32,i(1)?,i(2)? as u32,i(3)?,i(4)?,i(5)?,i(6)?,i(7)?,pointer),
                (false,true)=>native!(caller,"glCompressedTexSubImage3D",unsafe extern "system" fn(u32,i32,i32,i32,i32,i32,i32,i32,u32,i32,*const c_void);i(0)? as u32,i(1)?,i(2)?,i(3)?,i(4)?,i(5)?,i(6)?,i(7)?,i(8)? as u32,i(9)?,pointer),
            }
        }
        "env._glReadPixels"=>{
            let length=pixel_length(caller,i(2)?,i(3)?,1,i(4)? as u32,i(5)? as u32,true,false)?;
            if scalar(caller,0x88ED)?!=0 {
                let pointer=buffer_range(caller,0x88EB,i(6)?,length)? as *mut c_void;
                native!(caller,"glReadPixels",unsafe extern "system" fn(i32,i32,i32,i32,u32,u32,*mut c_void);i(0)?,i(1)?,i(2)?,i(3)?,i(4)? as u32,i(5)? as u32,pointer);
            }else{
                let pointer=range(caller,i(6)?,length)?;let mut bytes=unsafe{std::slice::from_raw_parts(pointer.cast::<u8>(),length)}.to_vec();
                native!(caller,"glReadPixels",unsafe extern "system" fn(i32,i32,i32,i32,u32,u32,*mut c_void);i(0)?,i(1)?,i(2)?,i(3)?,i(4)? as u32,i(5)? as u32,bytes.as_mut_ptr().cast());write(caller,i(6)?,&bytes)?;
            }
        }
        "env._glVertexAttribPointer"=>vertex_attrib_pointer(caller,i(0)?,i(1)?,i(2)?,i(3)?,i(4)?,i(5)?)?,
        "env._glVertexAttribIPointer"=>{
            if scalar(caller,0x8894)?==0 {gl_error(caller,0x502);return Ok(true);}
            native!(caller,"glVertexAttribIPointer",unsafe extern "system" fn(u32,i32,u32,i32,*const c_void);i(0)? as u32,i(1)?,i(2)? as u32,i(3)?,i(4)? as u32 as usize as *const c_void);
        }
        "env._glDrawElements"|"env._glDrawElementsInstanced"=>{
            draw_elements(caller,i(0)?,i(1)?,i(2)?,i(3)?,if name.ends_with("Instanced"){Some(i(4)?)}else{None})?;
        }
        name if name.starts_with("env._glUniform") && (name.ends_with("fv") || name.ends_with("iv"))=>{
            let short=name.strip_prefix("env._glUniform").unwrap();let matrix=short.starts_with("Matrix");
            let digit=short.as_bytes()[if matrix{6}else{0}];
            if !(b'1'..=b'4').contains(&digit){return Ok(false);}
            let components=(digit-b'0') as usize;let components=if matrix{components*components}else{components};
            let length=count(i(1)?,components)?;
            let address=i(if matrix{3}else{2})?;
            // Uniform uploads consume their input synchronously. Keep guest range
            // validation, and stage only unaligned data; no callback can grow memory.
            let staged;
            let pointer=if address as u32 % 4 == 0 {
                range(caller,address,count(length as i32,4)?)?.cast::<u32>()
            }else{staged=words(caller,address,length)?;staged.as_ptr()};
            if matrix{native!(caller,name.strip_prefix("env._").unwrap(),unsafe extern "system" fn(i32,i32,u8,*const u32);i(0)?,i(1)?,(i(2)?!=0) as u8,pointer);}
            else{native!(caller,name.strip_prefix("env._").unwrap(),unsafe extern "system" fn(i32,i32,*const u32);i(0)?,i(1)?,pointer);}
        }
        "env._glVertexAttrib4fv"=>{
            let values=words(caller,i(1)?,4)?;native!(caller,"glVertexAttrib4fv",unsafe extern "system" fn(u32,*const u32);i(0)? as u32,values.as_ptr());
        }
        "env._glClearBufferfv"|"env._glClearBufferuiv"|"env._glTexParameteriv"=>{
            let selector=i(if name=="env._glTexParameteriv"{1}else{0})? as u32;
            let length=if name=="env._glTexParameteriv"{if matches!(selector,0x1004|0x8E46){4}else{1}}else if selector==0x1800{4}else{1};
            let values=words(caller,i(2)?,length)?;
            native!(caller,name.strip_prefix("env._").unwrap(),unsafe extern "system" fn(u32,i32,*const u32);i(0)? as u32,i(1)?,values.as_ptr());
        }
        _=>return Ok(false),
    }
    Ok(true)
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn pixel_footprints_include_padding_skips_and_final_row_only(){
        let base=Layout{alignment:4,row:0,image:0,skip_pixels:0,skip_rows:0,skip_images:0};
        assert_eq!(footprint(3,2,1,3,base).unwrap(),21);
        assert_eq!(footprint(3,2,2,3,Layout{row:5,image:4,skip_pixels:2,skip_rows:1,skip_images:1,..base}).unwrap(),175);
        assert_eq!(footprint(0,2,1,4,base).unwrap(),0);
        assert!(footprint(usize::MAX,2,1,4,base).is_err());
        assert_eq!(pixel_size(0x1908,0x1406).unwrap(),16);
        assert_eq!(pixel_size(0x84F9,0x8DAD).unwrap(),8);
    }
}
