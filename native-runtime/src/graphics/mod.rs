//! Wasm32 GLES bridge. A missing backend or unimplemented import still traps.
use super::{Host,host_error,value_i32,read_u32,string_at,write_i32};
use wasmtime::{Caller,Val};
#[cfg(windows)]mod windows;
#[cfg(target_os="android")]mod android;
#[cfg(target_os="android")]pub(crate) use android::NativeWindow;
#[cfg(windows)]use windows as platform;
#[cfg(target_os="android")]use android as platform;
mod scalar;
mod pointers;
mod slots;
mod png;
#[cfg(all(test,windows,feature="compiler"))]mod tests;

fn s3tc_capabilities(formats:&std::collections::BTreeSet<u32>)->(bool,bool){
    ((0x83F0..=0x83F3).all(|format|formats.contains(&format)),
     (0x8C4C..=0x8C4F).all(|format|formats.contains(&format)))
}
#[cfg(test)]mod capability_tests {
    use super::*;
    #[test]fn compression_extensions_require_every_native_format(){
        let mut formats=(0x83F0..=0x83F3).collect();assert_eq!(s3tc_capabilities(&formats),(true,false));
        for format in 0x8C4C..=0x8C4F{formats.insert(format);}
        assert_eq!(s3tc_capabilities(&formats),(true,true));
        formats.remove(&0x8C4D);formats.remove(&0x83F2);
        assert_eq!(s3tc_capabilities(&formats),(false,false));
    }
}
pub(super) struct Graphics {
    #[cfg(any(windows,target_os="android"))]context:platform::Context,
    pub(super) guest_context:bool,
    current:bool,
    pub(super) strings:std::collections::BTreeMap<(u32,u32),i32>,
    pub(super) draw_calls:u64,
    index_sizes:std::collections::BTreeMap<u32,usize>,index_size_hits:u64,index_size_queries:u64,
    extensions:std::collections::BTreeSet<String>,
    compressed_formats:std::collections::BTreeSet<u32>,
    interface_extensions:Vec<String>,
    requested_extensions:std::collections::BTreeMap<String,bool>,
    error:u32,
    failures:Vec<String>,consumed_errors:std::collections::BTreeMap<u32,u64>,
    syncs:std::collections::BTreeMap<u32,usize>,next_sync:u32,
    #[cfg(any(windows,target_os="android"))]capture:Option<serde_json::Value>,
    diagnostic_calls:std::collections::BTreeMap<String,String>,
    procedures:[usize;slots::COUNT],
    state_cache:[Option<i32>;16],state_hits:u64,state_queries:u64,
    active_texture:Option<u32>,texture_units:u32,redundant_state_calls:u64,
}

impl Graphics {
    #[cfg(windows)]pub(super) fn post_space(&self,down:bool)->anyhow::Result<()>{self.context.post_space(down)}
    #[cfg(windows)]pub(super) fn next_key(&mut self)->anyhow::Result<Option<(super::input::Key,windows_sys::Win32::UI::WindowsAndMessaging::MSG)>>{self.context.next_key()}
    #[cfg(windows)]pub(super) fn finish_key(&self,message:&windows_sys::Win32::UI::WindowsAndMessaging::MSG,prevent_default:bool){self.context.finish_key(message,prevent_default);}
    #[cfg(any(windows,target_os="android"))]
    pub(super) fn offscreen(width:i32,height:i32)->anyhow::Result<Self>{
        Self::with_context(platform::Context::new(width,height)?)
    }
    #[cfg(target_os="android")]pub(crate) fn window(window:std::sync::Arc<NativeWindow>,width:i32,height:i32)->anyhow::Result<Self>{Self::with_context(platform::Context::with_window(width,height,Some(window))?)}
    #[cfg(any(windows,target_os="android"))]
    fn with_context(mut context:platform::Context)->anyhow::Result<Self>{
        let extensions=context.extensions()?;
        // WebGL2 always enables fixed-index primitive restart. GLES defaults to off.
        let enable:unsafe extern "system" fn(u32)=unsafe{std::mem::transmute(context.procedure("glEnable")?)};
        unsafe{enable(0x8D69)};
        let get:unsafe extern "system" fn(u32,*mut i32)=unsafe{std::mem::transmute(context.procedure("glGetIntegerv")?)};
        let mut texture_units=0;unsafe{get(0x8B4D,&mut texture_units)};
        let mut format_count=0;unsafe{get(0x86A2,&mut format_count)};
        anyhow::ensure!((0..=256).contains(&format_count),"Invalid compressed texture format count");
        let mut formats=vec![0i32;format_count as usize];
        if format_count>0{unsafe{get(0x86A3,formats.as_mut_ptr())};}
        let compressed_formats=formats.into_iter().map(|v|v as u32).collect();

        // Expose the WebGL2 subset this ABI can route, rather than native-only
        // extensions (mapbuffer/buffer_storage) whose guest pointers are stubs.
        let candidates=[("EXT_color_buffer_float","GL_EXT_color_buffer_float"),("EXT_color_buffer_half_float","GL_EXT_color_buffer_half_float"),
            ("EXT_texture_filter_anisotropic","GL_EXT_texture_filter_anisotropic"),("OES_texture_float_linear","GL_OES_texture_float_linear"),
            ("OES_texture_half_float_linear","GL_OES_texture_half_float_linear"),
            ("WEBGL_compressed_texture_etc1","GL_OES_compressed_ETC1_RGB8_texture"),("WEBGL_compressed_texture_astc","GL_KHR_texture_compression_astc_ldr")];
        let mut interface_extensions=candidates.into_iter().filter(|(_,native)|extensions.contains(*native)).map(|(name,_)|name.to_string()).collect::<Vec<_>>();
        interface_extensions.push("WEBGL_debug_renderer_info".into());
        let (s3tc,s3tc_srgb)=s3tc_capabilities(&compressed_formats);
        if s3tc{interface_extensions.push("WEBGL_compressed_texture_s3tc".into());}
        if s3tc_srgb{interface_extensions.push("WEBGL_compressed_texture_s3tc_srgb".into());}
        Ok(Self{context,guest_context:false,current:false,strings:Default::default(),draw_calls:0,index_sizes:Default::default(),index_size_hits:0,index_size_queries:0,extensions,compressed_formats,interface_extensions,requested_extensions:Default::default(),error:0,failures:Vec::new(),consumed_errors:Default::default(),syncs:Default::default(),next_sync:1,capture:None,diagnostic_calls:Default::default(),procedures:[0;slots::COUNT],state_cache:[None;16],state_hits:0,state_queries:0,active_texture:None,texture_units:texture_units.max(0) as u32,redundant_state_calls:0})
    }
    #[cfg(target_os="android")]pub(crate) fn present(&mut self)->anyhow::Result<bool>{
        if self.draw_calls==0{return Ok(false);}let errors=self.context.present()?;if self.error==0{self.error=errors.first().copied().unwrap_or(0);}Ok(true)
    }
    pub(super) fn procedure(&mut self,name:&str)->wasmtime::Result<usize>{
        // The VM and context have a single owner. Guest make-current, resize,
        // presentation and readback explicitly bind the context. No external GL
        // context is allowed to steal it between guest service calls.
        if let Some(slot)=slots::slot(name){if self.procedures[slot]!=0{return Ok(self.procedures[slot]);}}
        #[cfg(any(windows,target_os="android"))]{let address=self.context.procedure(name).map_err(|error|host_error(format!("{error:#}")))?;
            if let Some(slot)=slots::slot(name){self.procedures[slot]=address;}Ok(address)}
        #[cfg(not(any(windows,target_os="android")))]{let _=name;Err(host_error("Native GLES surface backend is not connected on this platform"))}
    }
    pub(super) fn dimensions(&self)->(i32,i32){
        #[cfg(any(windows,target_os="android"))]{self.context.dimensions()}
        #[cfg(not(any(windows,target_os="android")))]{(0,0)}
    }
    fn framebuffer(&self)->u32{
        #[cfg(any(windows,target_os="android"))]{self.context.framebuffer()}
        #[cfg(not(any(windows,target_os="android")))]{0}
    }
    pub(super) fn report(&self)->serde_json::Value{
        #[cfg(any(windows,target_os="android"))]{
            #[cfg(windows)]let backend="native-wgl-es";
            #[cfg(target_os="android")]let backend=if self.context.is_window(){"native-egl-es-window"}else{"native-egl-es-pbuffer"};
            #[cfg(target_os="android")]let swap_interval=self.context.is_window().then_some(self.context.swap_interval());
            #[cfg(windows)]let swap_interval:Option<i32>=None;
            serde_json::json!({"backend":backend,"swap_interval":swap_interval,"version":self.context.version,"vendor":self.context.vendor,"renderer":self.context.renderer,"width":self.dimensions().0,"height":self.dimensions().1,"draw_calls":self.draw_calls,"index_size_cache_hits":self.index_size_hits,"index_size_queries":self.index_size_queries,"state_cache_hits":self.state_hits,"state_queries":self.state_queries,"redundant_state_calls":self.redundant_state_calls,"requested_extensions":self.requested_extensions,"native_compressed_formats":self.compressed_formats,"robust_buffer_access":true,"pending_gl_error":self.error,"consumed_gl_errors":self.consumed_errors,"shader_program_failures":self.failures,"webgl2_fixed_restart":true,"zero_initialized_buffers":true,"capture":self.capture})}
        #[cfg(not(any(windows,target_os="android")))]{serde_json::json!({"backend":"unavailable"})}
    }
    pub(super) fn capture(&mut self,path:&std::path::Path)->anyhow::Result<bool>{
        #[cfg(any(windows,target_os="android"))]{
            anyhow::ensure!(self.draw_calls>0,"No guest draw calls to capture");
            let (pixels,errors)=self.context.read_surface()?;
            if self.error==0{self.error=errors.first().copied().unwrap_or(0);}
            let (width,height)=self.dimensions();let mut colors=std::collections::BTreeSet::new();
            let mut rgb=Vec::with_capacity(width as usize*height as usize*3);
            for row in pixels.chunks_exact(width as usize*4).rev(){for pixel in row.chunks_exact(4){rgb.extend_from_slice(&pixel[..3]);if colors.len()<65536{colors.insert([pixel[0],pixel[1],pixel[2]]);}}}
            let output=if path.extension().is_some_and(|extension|extension=="png"){png::encode(width as u32,height as u32,&rgb)?}
                else{let mut output=format!("P6\n{width} {height}\n255\n").into_bytes();output.extend(rgb);output};
            std::fs::write(path,&output)?;
            use sha2::Digest;
            self.capture=Some(serde_json::json!({"path":path,"width":width,"height":height,"distinct_rgb_colors_capped":colors.len(),"sha256":format!("{:x}",sha2::Sha256::digest(&output)),"preexisting_gl_errors":errors,"last_calls":self.diagnostic_calls}));
            Ok(colors.len()>1)
        }
        #[cfg(not(any(windows,target_os="android")))]{let _=path;anyhow::bail!("Surface readback backend unavailable")}
    }
}

fn procedure(caller:&mut Caller<'_,Host>,name:&str)->wasmtime::Result<usize>{
    let graphics=caller.data_mut().graphics.as_mut().ok_or_else(||host_error("Native GLES backend missing"))?;
    if !graphics.current{return Err(host_error("GL import called without a current guest context"));}
    graphics.procedure(name)
}

// Typed entry points share all pointer/context/driver checks with generic calls.
macro_rules! uniform_entry {
    ($entry:ident,$name:literal,$components:literal)=>{
        pub(super) fn $entry(caller:&mut Caller<'_,Host>,location:i32,rows:i32,address:i32)->wasmtime::Result<()>{
            pointers::uniform_fv(caller,$name,$components,location,rows,address)
        }
    }
}
uniform_entry!(uniform1fv_typed,"glUniform1fv",1);
uniform_entry!(uniform2fv_typed,"glUniform2fv",2);
uniform_entry!(uniform3fv_typed,"glUniform3fv",3);
pub(super) fn buffer_sub_data_typed(caller:&mut Caller<'_,Host>,target:i32,offset:i32,size:i32,address:i32)->wasmtime::Result<()>{
    pointers::buffer_sub_data(caller,target,offset,size,address)
}
macro_rules! scalar_entry {
    ($entry:ident,$name:literal)=>{
        pub(super) fn $entry(caller:&mut Caller<'_,Host>,value:i32)->wasmtime::Result<()>{
            let function:unsafe extern "system" fn(u32)=unsafe{std::mem::transmute(procedure(caller,$name)?)};
            unsafe{function(value as u32)};Ok(())
        }
    }
}
scalar_entry!(use_program_typed,"glUseProgram");
scalar_entry!(enable_vertex_attrib_typed,"glEnableVertexAttribArray");
scalar_entry!(disable_vertex_attrib_typed,"glDisableVertexAttribArray");
pub(super) fn bind_texture_typed(caller:&mut Caller<'_,Host>,target:i32,object:i32)->wasmtime::Result<()>{
    let function:unsafe extern "system" fn(u32,u32)=unsafe{std::mem::transmute(procedure(caller,"glBindTexture")?)};
    unsafe{function(target as u32,object as u32)};Ok(())
}
pub(super) fn bind_buffer_typed(caller:&mut Caller<'_,Host>,target:i32,object:i32)->wasmtime::Result<()>{
    let slot=match target{0x8892=>Some(0),0x8893=>Some(1),0x88EB=>Some(2),0x88EC=>Some(3),_=>None};
    if let Some(slot)=slot{let cached=&mut caller.data_mut().graphics.as_mut().unwrap().state_cache[slot];if *cached!=Some(object){*cached=None;}}
    let function:unsafe extern "system" fn(u32,u32)=unsafe{std::mem::transmute(procedure(caller,"glBindBuffer")?)};
    unsafe{function(target as u32,object as u32)};Ok(())
}
pub(super) fn uniform4fv_typed(caller:&mut Caller<'_,Host>,location:i32,rows:i32,address:i32)->wasmtime::Result<()>{
    pointers::uniform4fv(caller,location,rows,address)
}
pub(super) fn vertex_attrib_pointer_typed(caller:&mut Caller<'_,Host>,index:i32,components:i32,kind:i32,normalized:i32,stride:i32,offset:i32)->wasmtime::Result<()>{
    pointers::vertex_attrib_pointer(caller,index,components,kind,normalized,stride,offset)
}
pub(super) fn draw_elements_typed(caller:&mut Caller<'_,Host>,mode:i32,elements:i32,kind:i32,offset:i32)->wasmtime::Result<()>{
    pointers::draw_elements(caller,mode,elements,kind,offset,None)
}
pub(super) fn active_texture_typed(caller:&mut Caller<'_,Host>,target:i32)->wasmtime::Result<()>{
    let function:unsafe extern "system" fn(u32)=unsafe{std::mem::transmute(procedure(caller,"glActiveTexture")?)};
    let target=target as u32;let graphics=caller.data_mut().graphics.as_mut().unwrap();
    if graphics.active_texture==Some(target){graphics.redundant_state_calls+=1;return Ok(());}
    unsafe{function(target)};
    if target.wrapping_sub(0x84C0)<graphics.texture_units{graphics.active_texture=Some(target);}
    Ok(())
}
pub(super) fn dispatch(caller:&mut Caller<'_,Host>,name:&str,args:&[Val],results:&mut[Val])->wasmtime::Result<bool>{
    if caller.data().graphics.is_none(){return Ok(false);}
    // Query real driver state lazily after each potential mutation. Rebinding
    // the already driver-verified object cannot change that binding, even if
    // the setter fails. Still call the driver to preserve all error behavior.
    match name{
        "env._glDeleteBuffers"=>caller.data_mut().graphics.as_mut().unwrap().state_cache[..4].fill(None),
        "env._glBindVertexArray"|"env._glDeleteVertexArrays"=>caller.data_mut().graphics.as_mut().unwrap().state_cache[1]=None,
        "env._glPixelStorei"=>caller.data_mut().graphics.as_mut().unwrap().state_cache[4..].fill(None),
        _=>{}
    }
    if caller.data().trace_gl_calls && matches!(name,"env._glViewport"|"env._glScissor"|"env._glDrawElements"|"env._glBindFramebuffer"|"env._glUseProgram"|"env._glColorMask"|"env._glVertexAttribPointer"){
        caller.data_mut().graphics.as_mut().unwrap().diagnostic_calls.insert(name.into(),format!("{args:?}"));
    }
    match name {
        "env._glBindBuffer"=>bind_buffer_typed(caller,value_i32(args,0)?,value_i32(args,1)?)?,
        "env._glActiveTexture"=>{
            // Only valid enums can establish the cache; invalid setters still
            // reach the driver every time. procedure also checks current context.
            active_texture_typed(caller,value_i32(args,0)?)?;
        }
        "env._glUniform4fv"=>uniform4fv_typed(caller,value_i32(args,0)?,value_i32(args,1)?,value_i32(args,2)?)?,
        "env._JS_Cursor_SetShow"=>{
            #[cfg(any(windows,target_os="android"))]caller.data().graphics.as_ref().unwrap().context.cursor_visible(value_i32(args,0)?!=0).map_err(|error|host_error(format!("{error:#}")))?;
            #[cfg(not(any(windows,target_os="android")))]return Err(host_error("Native cursor surface backend unavailable"));
        }
        "env._glCompileShader"|"env._glLinkProgram"=>{
            scalar::dispatch(caller,name,args,results)?;
            let shader=name=="env._glCompileShader";let object=value_i32(args,0)? as u32;
            let get:unsafe extern "system" fn(u32,u32,*mut i32)=unsafe{std::mem::transmute(procedure(caller,if shader{"glGetShaderiv"}else{"glGetProgramiv"})?)};
            let mut ok=0;unsafe{get(object,if shader{0x8B81}else{0x8B82},&mut ok)};
            if ok==0 && caller.data().graphics.as_ref().unwrap().failures.len()<16{
                let info:unsafe extern "system" fn(u32,i32,*mut i32,*mut u8)=unsafe{std::mem::transmute(procedure(caller,if shader{"glGetShaderInfoLog"}else{"glGetProgramInfoLog"})?)};
                let mut bytes=[0u8;4096];let mut length=0;unsafe{info(object,4096,&mut length,bytes.as_mut_ptr())};
                caller.data_mut().graphics.as_mut().unwrap().failures.push(format!("{name} {object}: {}",String::from_utf8_lossy(&bytes[..(length.max(0) as usize).min(4096)])));
            }
        }
        "env._glGetError"=>{
            let error=std::mem::take(&mut caller.data_mut().graphics.as_mut().unwrap().error);
            if error!=0{results[0]=Val::I32(error as i32);}else{scalar::dispatch(caller,name,args,results)?;}
            if let Some(code)=results.first().and_then(Val::i32).filter(|v|*v!=0){*caller.data_mut().graphics.as_mut().unwrap().consumed_errors.entry(code as u32).or_default()+=1;}
        }
        "env._JS_SystemInfo_HasWebGL"=>results[0]=Val::I32(2),
        "env._JS_SystemInfo_HasCursorLock"|"env._JS_SystemInfo_HasFullscreen"=>results[0]=Val::I32(0),
        "env._emscripten_exit_pointerlock"|"env._emscripten_request_pointerlock"=>{
            // This surface advertises no browser/relative pointer-lock service.
            caller.data_mut().unavailable_services.insert(name.into());results[0]=Val::I32(-1);
        }
        "env._JS_SystemInfo_GetMatchWebGLToCanvasSize"=>results[0]=Val::I32(1),
        "env._JS_SystemInfo_GetLanguage"=>results[0]=Val::I32(super::host_string(caller,args,"en")?),
        "env._JS_SystemInfo_GetGPUInfo"=>{
            #[cfg(any(windows,target_os="android"))]{let text=caller.data().graphics.as_ref().unwrap().context.renderer.clone();results[0]=Val::I32(super::host_string(caller,args,&text)?);}
            #[cfg(not(any(windows,target_os="android")))]{return Err(host_error("Native GPU information backend unavailable"));}
        }
        "env._emscripten_webgl_enable_extension"=>{
            let handle=value_i32(args,0)?;
            if handle!=1 || !caller.data().graphics.as_ref().unwrap().guest_context {results[0]=Val::I32(0);return Ok(true);}
            let requested=string_at(caller,value_i32(args,1)? as u32 as usize)?;
            let requested=requested.strip_prefix("GL_").unwrap_or(&requested).to_string();
            let requested=requested.strip_prefix("WEBKIT_").or_else(||requested.strip_prefix("MOZ_")).unwrap_or(&requested).to_string();
            let graphics=caller.data_mut().graphics.as_mut().unwrap();
            let canonical=match requested.as_str(){
                "WEBGL_compressed_texture_s3tc"=>"GL_EXT_texture_compression_s3tc".into(),
                "WEBGL_compressed_texture_etc1"=>"GL_OES_compressed_ETC1_RGB8_texture".into(),
                "WEBGL_compressed_texture_pvrtc"=>"GL_IMG_texture_compression_pvrtc".into(),
                "WEBGL_compressed_texture_astc"=>"GL_KHR_texture_compression_astc_ldr".into(),
                value=>format!("GL_{value}"),
            };
            let available=graphics.interface_extensions.contains(&requested) && (graphics.extensions.contains(&canonical) || matches!(requested.as_str(),"WEBGL_debug_renderer_info"|"WEBGL_compressed_texture_s3tc_srgb"));
            graphics.requested_extensions.insert(requested,available);results[0]=Val::I32(available as i32);
        }
        "env._emscripten_is_webgl_context_lost"=>{
            // This pinned framework ignores its target argument and queries
            // Module.ctx (the current context), unlike newer handle-based APIs.
            if !caller.data().graphics.as_ref().unwrap().current{results[0]=Val::I32(1);return Ok(true);}
            let graphics=caller.data_mut().graphics.as_mut().unwrap();
            let address=graphics.procedure("glGetGraphicsResetStatus").or_else(|_|graphics.procedure("glGetGraphicsResetStatusKHR")).or_else(|_|graphics.procedure("glGetGraphicsResetStatusEXT"))?;
            let function:unsafe extern "system" fn()->u32=unsafe{std::mem::transmute(address)};
            let lost=unsafe{function()}!=0;
            #[cfg(any(windows,target_os="android"))]if !graphics.current{graphics.context.make_current(false).map_err(|error|host_error(format!("{error:#}")))?;}
            results[0]=Val::I32(lost as i32);
        }
        "env._JS_SystemInfo_GetCanvasClientSize"=>{
            let target=string_at(caller,value_i32(args,0)? as u32 as usize)?;
            let (width,height)=if matches!(target.as_str(),"#canvas"|"canvas"){caller.data().graphics.as_ref().unwrap().dimensions()}else{(0,0)};
            let memory=caller.data().memory.unwrap();
            memory.write(&mut *caller,value_i32(args,1)? as u32 as usize,&(width as f64).to_le_bytes()).map_err(|error|host_error(error.to_string()))?;
            memory.write(&mut *caller,value_i32(args,2)? as u32 as usize,&(height as f64).to_le_bytes()).map_err(|error|host_error(error.to_string()))?;
        }
        "env._JS_SystemInfo_GetScreenSize"=>{
            let (width,height)=caller.data().graphics.as_ref().unwrap().dimensions();
            let width_pointer=value_i32(args,0)? as u32 as usize;let height_pointer=value_i32(args,1)? as u32 as usize;
            let memory=caller.data().memory.unwrap();
            memory.write(&mut *caller,width_pointer,&(width as f64).to_le_bytes()).map_err(|error|host_error(error.to_string()))?;
            memory.write(&mut *caller,height_pointer,&(height as f64).to_le_bytes()).map_err(|error|host_error(error.to_string()))?;
        }
        "env._emscripten_webgl_init_context_attributes"=>{
            let defaults=[1i32,1,0,1,1,0,0,0,1,0,1,0,0,0];
            let pointer=value_i32(args,0)? as u32 as usize;let bytes=defaults.iter().flat_map(|value|value.to_le_bytes()).collect::<Vec<_>>();
            caller.data().memory.unwrap().write(&mut *caller,pointer,&bytes).map_err(|error|host_error(error.to_string()))?;
        }
        "env._emscripten_webgl_create_context"=>{
            let target=value_i32(args,0)?;let target=if target==0{"#canvas".into()}else{string_at(caller,target as u32 as usize)?};
            if !matches!(target.as_str(),"#canvas"|"canvas"){results[0]=Val::I32(0);return Ok(true);}
            let attributes=value_i32(args,1)? as u32 as usize;let version=read_u32(caller,attributes+32)?;
            let graphics=caller.data_mut().graphics.as_mut().unwrap();
            if !matches!(version,1|2) || graphics.guest_context {results[0]=Val::I32(0);}
            else{graphics.guest_context=true;results[0]=Val::I32(1);}
        }
        "env._emscripten_webgl_make_context_current"=>{
            let handle=value_i32(args,0)?;let valid=handle==0 || (handle==1 && caller.data().graphics.as_ref().unwrap().guest_context);
            if valid {
                let graphics=caller.data_mut().graphics.as_mut().unwrap();
                #[cfg(any(windows,target_os="android"))]graphics.context.make_current(handle==1).map_err(|error|host_error(format!("{error:#}")))?;
                graphics.current=handle==1;
            }
            results[0]=Val::I32(if valid{0}else{-5});
        }
        "env._emscripten_webgl_get_current_context"=>results[0]=Val::I32(caller.data().graphics.as_ref().unwrap().current as i32),
        "env._emscripten_set_canvas_element_size"=>{
            let target=value_i32(args,0)?;let target=if target==0{"#canvas".into()}else{string_at(caller,target as u32 as usize)?};
            if !matches!(target.as_str(),"#canvas"|"canvas"){results[0]=Val::I32(-4);return Ok(true);}
            let width=value_i32(args,1)?;let height=value_i32(args,2)?;
            #[cfg(any(windows,target_os="android"))]{results[0]=Val::I32(if caller.data_mut().graphics.as_mut().unwrap().context.resize(width,height).is_ok(){0}else{-5});}
            #[cfg(not(any(windows,target_os="android")))]{let _=(width,height);return Err(host_error("Native canvas resize backend unavailable"));}
        }
        "env._emscripten_get_canvas_element_size"=>{
            let target=value_i32(args,0)?;let target=if target==0{"#canvas".into()}else{string_at(caller,target as u32 as usize)?};
            if !matches!(target.as_str(),"#canvas"|"canvas"){results[0]=Val::I32(-4);return Ok(true);}
            let (width,height)=caller.data().graphics.as_ref().unwrap().dimensions();
            write_i32(caller,value_i32(args,1)? as u32 as usize,width)?;write_i32(caller,value_i32(args,2)? as u32 as usize,height)?;results[0]=Val::I32(0);
        }
        "env._emscripten_asm_const_i"|"env._emscripten_asm_const_sync_on_main_thread_i"=>{
            results[0]=Val::I32(match value_i32(args,0)? {
                0=>0, // Native GL color storage is not browser-composited/premultiplied.
                1=>1, // Hidden offscreen back buffer is retained; no implicit swap.
                2=>0, // No quit request has been installed by this diagnostic host.
                code=>return Err(host_error(format!("Unimplemented pinned ASM constant: {code}"))),
            });
        }
        _=>{
            if pointers::dispatch(caller,name,args,results)?{return Ok(true);}
            return scalar::dispatch(caller,name,args,results);
        }
    }
    Ok(true)
}
