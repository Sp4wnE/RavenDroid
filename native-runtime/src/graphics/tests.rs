use super::*;

#[test]
#[ignore="Requires Windows GPU; actual uploads for advertised compressed formats"]
fn advertised_s3tc_formats_accept_real_compressed_uploads(){
    let mut g=Graphics::offscreen(16,16).unwrap();
    let generate:unsafe extern "system" fn(i32,*mut u32)=unsafe{std::mem::transmute(g.procedure("glGenTextures").unwrap())};
    let bind:unsafe extern "system" fn(u32,u32)=unsafe{std::mem::transmute(g.procedure("glBindTexture").unwrap())};
    let upload:unsafe extern "system" fn(u32,i32,u32,i32,i32,i32,i32,*const std::ffi::c_void)=unsafe{std::mem::transmute(g.procedure("glCompressedTexImage2D").unwrap())};
    let error:unsafe extern "system" fn()->u32=unsafe{std::mem::transmute(g.procedure("glGetError").unwrap())};
    let delete:unsafe extern "system" fn(i32,*const u32)=unsafe{std::mem::transmute(g.procedure("glDeleteTextures").unwrap())};
    let mut texture=0;unsafe{generate(1,&mut texture);bind(0xDE1,texture);}
    let (linear,srgb)=s3tc_capabilities(&g.compressed_formats);
    assert_eq!(g.interface_extensions.iter().any(|s|s=="WEBGL_compressed_texture_s3tc"),linear);
    assert_eq!(g.interface_extensions.iter().any(|s|s=="WEBGL_compressed_texture_s3tc_srgb"),srgb);
    let block=[0u8;16];
    for (enabled,base) in [(linear,0x83F0),(srgb,0x8C4C)]{
        if enabled{for offset in 0..4{
            unsafe{upload(0xDE1,0,base+offset,4,4,0,if offset<2{8}else{16},block.as_ptr().cast());}
            assert_eq!(unsafe{error()},0,"Advertised compressed format {:#x}",base+offset);
        }}
    }
    unsafe{delete(1,&texture);}
}

#[test]
#[ignore="Requires Windows GPU; direct buffer upload bounds and native errors"]
fn typed_buffer_upload_preserves_guest_bounds_and_driver_errors(){
    let(mut store,instance)=crate::tests::fixture(r#"(module
      (import "env" "memory" (memory 512))
      (import "env" "_glGenBuffers" (func $gen (param i32 i32)))
      (import "env" "_glBindBuffer" (func $bind (param i32 i32)))
      (import "env" "_glBufferData" (func $allocate (param i32 i32 i32 i32)))
      (import "env" "_glBufferSubData" (func $upload (param i32 i32 i32 i32)))
      (import "env" "_glGetError" (func $error (result i32)))
      (func (export "setup") i32.const 1 i32.const 64 call $gen
        i32.const 34962 i32.const 64 i32.load call $bind
        i32.const 34962 i32.const 16 i32.const 0 i32.const 35048 call $allocate)
      (func (export "upload") (param i32 i32 i32 i32)
        local.get 0 local.get 1 local.get 2 local.get 3 call $upload)
      (func (export "error") (result i32) call $error))"#);
    let mut g=Graphics::offscreen(16,16).unwrap();g.current=true;g.guest_context=true;store.data_mut().graphics=Some(g);
    instance.get_typed_func::<(),()>(&mut store,"setup").unwrap().call(&mut store,()).unwrap();
    let upload=instance.get_typed_func::<(i32,i32,i32,i32),()>(&mut store,"upload").unwrap();
    let error=instance.get_typed_func::<(),i32>(&mut store,"error").unwrap();
    upload.call(&mut store,(0x8892,0,4,65)).unwrap();assert_eq!(error.call(&mut store,()).unwrap(),0);
    upload.call(&mut store,(0xFFFF,0,4,65)).unwrap();assert_eq!(error.call(&mut store,()).unwrap(),0x500);
    upload.call(&mut store,(0x8892,15,4,65)).unwrap();assert_eq!(error.call(&mut store,()).unwrap(),0x501);
    assert!(upload.call(&mut store,(0x8892,0,-1,65)).is_err());
    assert!(upload.call(&mut store,(0x8892,0,4,512*65536-1)).is_err());
}

#[test]
#[ignore="Requires Windows GPU; redundant state must not hide invalid enums or missing context"]
fn active_texture_coalesces_valid_state_and_preserves_driver_errors(){
 let(mut store,instance)=crate::tests::fixture(r#"(module
  (import "env" "memory" (memory 512))
  (import "env" "_glActiveTexture" (func $active (param i32)))
  (import "env" "_glGetIntegerv" (func $get (param i32 i32)))
  (import "env" "_glGetError" (func $error (result i32)))
  (func (export "active") (param i32) local.get 0 call $active)
  (func (export "query") i32.const 34016 i32.const 64 call $get)
  (func (export "error") (result i32) call $error))"#);
 let mut g=Graphics::offscreen(16,16).unwrap();g.current=true;g.guest_context=true;store.data_mut().graphics=Some(g);
 let active=instance.get_typed_func::<i32,()>(&mut store,"active").unwrap();let error=instance.get_typed_func::<(),i32>(&mut store,"error").unwrap();
 active.call(&mut store,0x84C0).unwrap();active.call(&mut store,0x84C0).unwrap();assert_eq!(store.data().graphics.as_ref().unwrap().redundant_state_calls,1);
 for _ in 0..2{active.call(&mut store,0xffff).unwrap();assert_eq!(error.call(&mut store,()).unwrap(),0x500);}
 active.call(&mut store,0x84C1).unwrap();instance.get_typed_func::<(),()>(&mut store,"query").unwrap().call(&mut store,()).unwrap();
 assert_eq!(&store.data().memory.unwrap().data(&store)[64..68],&0x84C1i32.to_le_bytes());
 store.data_mut().graphics.as_mut().unwrap().current=false;assert!(active.call(&mut store,0x84C1).is_err());
}

#[test]
#[ignore="Requires Windows GPU; invalid bindings and VAO changes must not stale cached state"]
fn cached_binding_queries_follow_real_driver_state_after_failed_setters_and_deletion(){
    let(mut store,instance)=crate::tests::fixture(r#"(module
      (import "env" "memory" (memory 512))
      (import "env" "_glGenBuffers" (func $gen (param i32 i32)))
      (import "env" "_glBindBuffer" (func $bind (param i32 i32)))
      (import "env" "_glDeleteBuffers" (func $delete (param i32 i32)))
      (import "env" "_glBindVertexArray" (func $vao (param i32)))
      (import "env" "_glVertexAttribPointer" (func $attrib (param i32 i32 i32 i32 i32 i32)))
      (import "env" "_glGetError" (func $error (result i32)))
      (func (export "setup") i32.const 1 i32.const 64 call $gen i32.const 34962 i32.const 64 i32.load call $bind)
      (func (export "rebind") i32.const 34962 i32.const 64 i32.load call $bind)
      (func (export "attrib") i32.const 0 i32.const 2 i32.const 5126 i32.const 0 i32.const 0 i32.const 0 call $attrib)
      (func (export "bad_bind") i32.const 65535 i32.const 0 call $bind)
      (func (export "delete") i32.const 1 i32.const 64 call $delete)
      (func (export "vao") i32.const 0 call $vao)
      (func (export "error") (result i32) call $error))"#);
    let mut graphics=Graphics::offscreen(16,16).unwrap();graphics.guest_context=true;graphics.current=true;store.data_mut().graphics=Some(graphics);
    let setup=instance.get_typed_func::<(),()>(&mut store,"setup").unwrap();let attrib=instance.get_typed_func::<(),()>(&mut store,"attrib").unwrap();let error=instance.get_typed_func::<(),i32>(&mut store,"error").unwrap();
    setup.call(&mut store,()).unwrap();attrib.call(&mut store,()).unwrap();let binding=store.data().graphics.as_ref().unwrap().state_cache[0];assert!(binding.is_some_and(|i|i>0));
    let queries=store.data().graphics.as_ref().unwrap().state_queries;attrib.call(&mut store,()).unwrap();assert_eq!(store.data().graphics.as_ref().unwrap().state_queries,queries);
    instance.get_typed_func::<(),()>(&mut store,"rebind").unwrap().call(&mut store,()).unwrap();attrib.call(&mut store,()).unwrap();assert_eq!(store.data().graphics.as_ref().unwrap().state_queries,queries);
    instance.get_typed_func::<(),()>(&mut store,"bad_bind").unwrap().call(&mut store,()).unwrap();assert_eq!(error.call(&mut store,()).unwrap(),0x500);
    attrib.call(&mut store,()).unwrap();assert_eq!(store.data().graphics.as_ref().unwrap().state_cache[0],binding);
    store.data_mut().graphics.as_mut().unwrap().state_cache[1]=Some(123);
    instance.get_typed_func::<(),()>(&mut store,"vao").unwrap().call(&mut store,()).unwrap();assert_eq!(store.data().graphics.as_ref().unwrap().state_cache[1],None);
    instance.get_typed_func::<(),()>(&mut store,"delete").unwrap().call(&mut store,()).unwrap();attrib.call(&mut store,()).unwrap();assert_eq!(store.data().graphics.as_ref().unwrap().state_cache[0],Some(0));assert_eq!(error.call(&mut store,()).unwrap(),0x502);
}

#[test]
#[ignore="Requires Windows GPU; real guest shader/uniform/indexed draw and sync handles"]
fn guest_indexed_triangle_renders_and_sync_handles_roundtrip(){
    let inventory:serde_json::Value=serde_json::from_str(include_str!("../../abi-inventory.json")).unwrap();
    let names=["_glCreateShader","_glShaderSource","_glCompileShader","_glGetShaderiv","_glCreateProgram","_glAttachShader","_glLinkProgram","_glUseProgram","_glGetUniformLocation","_glUniform4fv","_glGenBuffers","_glBindBuffer","_glBufferData","_glVertexAttribPointer","_glEnableVertexAttribArray","_glDrawElements","_glReadPixels","_glGetError","_glBindFramebuffer","_glGetIntegerv","_glFenceSync","_glFinish","_glClientWaitSync","_glDeleteSync"];
    let mut source="(module (import \"env\" \"memory\" (memory 512))".to_string();
    for(index,name)in names.iter().enumerate(){
        let item=inventory["imports"].as_array().unwrap().iter().find(|item|item["name"]==*name).unwrap();
        let params=item["params"].as_array().unwrap().iter().map(|value|value.as_str().unwrap()).collect::<Vec<_>>();
        let result=item["results"].as_array().unwrap().iter().map(|value|value.as_str().unwrap()).collect::<Vec<_>>();
        let signature=format!("{}{}",if params.is_empty(){String::new()}else{format!("(param {})",params.join(" "))},if result.is_empty(){String::new()}else{format!("(result {})",result.join(" "))});
        source+=&format!("(import \"env\" \"{name}\" (func $f{index} {signature}))");
    }
    for(index,name)in names.iter().enumerate(){
        let item=inventory["imports"].as_array().unwrap().iter().find(|item|item["name"]==*name).unwrap();
        let params=item["params"].as_array().unwrap().iter().map(|value|value.as_str().unwrap()).collect::<Vec<_>>();
        let result=item["results"].as_array().unwrap().iter().map(|value|value.as_str().unwrap()).collect::<Vec<_>>();
        source+=&format!("(func (export \"{name}\") {}{} {} call $f{index})",if params.is_empty(){String::new()}else{format!("(param {})",params.join(" "))},if result.is_empty(){String::new()}else{format!("(result {})",result.join(" "))},(0..params.len()).map(|n|format!("local.get {n}")).collect::<Vec<_>>().join(" "));
    }source+=")";
    let(mut store,instance)=crate::tests::fixture(&source);let mut graphics=Graphics::offscreen(16,16).unwrap();graphics.guest_context=true;graphics.current=true;store.data_mut().graphics=Some(graphics);
    fn gl(store:&mut wasmtime::Store<Host>,instance:&wasmtime::Instance,name:&str,args:&[i32])->i32{
        let function=instance.get_func(&mut *store,name).unwrap();let mut results=vec![Val::I32(0);function.ty(&*store).results().len()];
        function.call(&mut *store,&args.iter().copied().map(Val::I32).collect::<Vec<_>>(),&mut results).unwrap();results.first().and_then(Val::i32).unwrap_or(0)
    }
    let memory=store.data().memory.unwrap();let read=|store:&wasmtime::Store<Host>,pointer:usize|i32::from_le_bytes(memory.data(store)[pointer..pointer+4].try_into().unwrap());
    let mut shaders=Vec::new();
    for(kind,text)in [(0x8B31,"#version 300 es\nlayout(location=0) in vec2 pos;void main(){gl_Position=vec4(pos,0.,1.);}"),(0x8B30,"#version 300 es\nprecision highp float;uniform vec4 color;out vec4 target;void main(){target=color;}")]{
        let shader=gl(&mut store,&instance,"_glCreateShader",&[kind]);memory.write(&mut store,1024,format!("{text}\0").as_bytes()).unwrap();memory.write(&mut store,128,&1024i32.to_le_bytes()).unwrap();
        gl(&mut store,&instance,"_glShaderSource",&[shader,1,128,0]);gl(&mut store,&instance,"_glCompileShader",&[shader]);gl(&mut store,&instance,"_glGetShaderiv",&[shader,0x8B81,132]);assert_eq!(read(&store,132),1);shaders.push(shader);
    }
    let program=gl(&mut store,&instance,"_glCreateProgram",&[]);for shader in shaders{gl(&mut store,&instance,"_glAttachShader",&[program,shader]);}gl(&mut store,&instance,"_glLinkProgram",&[program]);gl(&mut store,&instance,"_glUseProgram",&[program]);
    memory.write(&mut store,1024,b"color\0").unwrap();let location=gl(&mut store,&instance,"_glGetUniformLocation",&[program,1024]);assert!(location>=0);
    memory.write(&mut store,2048,&[0.25f32,0.5,0.75,1.].iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>()).unwrap();gl(&mut store,&instance,"_glUniform4fv",&[location,1,2048]);
    // Unaligned input takes the staging fallback; aligned input above is borrowed.
    memory.write(&mut store,2051,&[0.25f32,0.5,0.75,1.].iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>()).unwrap();
    gl(&mut store,&instance,"_glUniform4fv",&[location,1,2051]);
    let uniform=instance.get_func(&mut store,"_glUniform4fv").unwrap();
    let end=memory.data_size(&store) as i32;
    for pointer in [end-4,end-3]{assert!(uniform.call(&mut store,&[Val::I32(location),Val::I32(1),Val::I32(pointer)],&mut []).is_err());}
    gl(&mut store,&instance,"_glGenBuffers",&[2,64]);let vertex=read(&store,64);let index=read(&store,68);
    gl(&mut store,&instance,"_glBindBuffer",&[0x8892,vertex]);memory.write(&mut store,3072,&[-1f32,-1.,3.,-1.,-1.,3.].iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>()).unwrap();gl(&mut store,&instance,"_glBufferData",&[0x8892,24,3072,0x88E4]);
    gl(&mut store,&instance,"_glBindBuffer",&[0x8893,index]);memory.write(&mut store,4096,&[0u16,1,2].iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>()).unwrap();gl(&mut store,&instance,"_glBufferData",&[0x8893,6,4096,0x88E4]);
    gl(&mut store,&instance,"_glVertexAttribPointer",&[0,2,0x1406,0,0,0]);gl(&mut store,&instance,"_glEnableVertexAttribArray",&[0]);
    gl(&mut store,&instance,"_glBindFramebuffer",&[0x8D40,0]);gl(&mut store,&instance,"_glGetIntegerv",&[0x8CA6,128]);assert_eq!(read(&store,128),0);
    gl(&mut store,&instance,"_glDrawElements",&[4,3,0x1403,0]);gl(&mut store,&instance,"_glReadPixels",&[8,8,1,1,0x1908,0x1401,8192]);
    assert!(memory.data(&store)[8192..8196].iter().zip([64u8,128,191,255]).all(|(a,b)|a.abs_diff(b)<=1));
    let queries=store.data().graphics.as_ref().unwrap().index_size_queries;
    gl(&mut store,&instance,"_glDrawElements",&[4,3,0x1403,0]);
    assert_eq!(store.data().graphics.as_ref().unwrap().index_size_queries,queries);
    assert_eq!(store.data().graphics.as_ref().unwrap().index_size_hits,1);
    // Trace mode takes the generic adapter but must render and count identically.
    store.data_mut().trace_gl_calls=true;
    gl(&mut store,&instance,"_glVertexAttribPointer",&[0,2,0x1406,0,0,0]);
    gl(&mut store,&instance,"_glDrawElements",&[4,3,0x1403,0]);
    assert_eq!(store.data().graphics.as_ref().unwrap().draw_calls,3);
    assert!(store.data().graphics.as_ref().unwrap().diagnostic_calls.contains_key("env._glVertexAttribPointer"));
    assert!(store.data().graphics.as_ref().unwrap().diagnostic_calls.contains_key("env._glDrawElements"));
    store.data_mut().trace_gl_calls=false;
    // Resizing must invalidate the cache, including allocations through other targets.
    gl(&mut store,&instance,"_glBindBuffer",&[0x8892,index]);
    gl(&mut store,&instance,"_glBufferData",&[0x8892,2,4096,0x88E4]);
    let draw=instance.get_func(&mut store,"_glDrawElements").unwrap();
    assert!(draw.call(&mut store,&[Val::I32(4),Val::I32(3),Val::I32(0x1403),Val::I32(0)],&mut []).is_err());
    assert_eq!(store.data().graphics.as_ref().unwrap().index_size_queries,queries+1);
    let sync=gl(&mut store,&instance,"_glFenceSync",&[0x9117,0]);assert!(sync!=0);gl(&mut store,&instance,"_glFinish",&[]);
    assert!(matches!(gl(&mut store,&instance,"_glClientWaitSync",&[sync,0,0,0]),0x911A|0x911C));gl(&mut store,&instance,"_glDeleteSync",&[sync]);
    assert_eq!(gl(&mut store,&instance,"_glClientWaitSync",&[sync,0,0,0]),0x911D);assert_eq!(gl(&mut store,&instance,"_glGetError",&[]),0x501);assert_eq!(gl(&mut store,&instance,"_glGetError",&[]),0);
}

#[test]
#[ignore="Requires a Windows GPU; real wasm32-to-native texture/FBO roundtrip"]
fn guest_texture_upload_readback_and_pointer_bounds(){
    let (mut store,instance)=crate::tests::fixture(r#"(module
      (import "env" "memory" (memory 512))
      (import "env" "_glGenTextures" (func $gen (param i32 i32)))
      (import "env" "_glBindTexture" (func $bind (param i32 i32)))
      (import "env" "_glTexImage2D" (func $image (param i32 i32 i32 i32 i32 i32 i32 i32 i32)))
      (import "env" "_glGenFramebuffers" (func $genfb (param i32 i32)))
      (import "env" "_glBindFramebuffer" (func $bindfb (param i32 i32)))
      (import "env" "_glFramebufferTexture2D" (func $attach (param i32 i32 i32 i32 i32)))
      (import "env" "_glCheckFramebufferStatus" (func $status (param i32) (result i32)))
      (import "env" "_glReadPixels" (func $read (param i32 i32 i32 i32 i32 i32 i32)))
      (import "env" "_glVertexAttribPointer" (func $attrib (param i32 i32 i32 i32 i32 i32)))
      (import "env" "_glGetError" (func $error (result i32)))
      (func (export "texture") (param $pointer i32)
        i32.const 1 i32.const 32 call $gen
        i32.const 3553 i32.const 32 i32.load call $bind
        i32.const 3553 i32.const 0 i32.const 32849 i32.const 3 i32.const 2 i32.const 0
        i32.const 6407 i32.const 5121 local.get $pointer call $image)
      (func (export "read") (result i32)
        i32.const 1 i32.const 36 call $genfb
        i32.const 36160 i32.const 36 i32.load call $bindfb
        i32.const 36160 i32.const 36064 i32.const 3553 i32.const 32 i32.load i32.const 0 call $attach
        i32.const 0 i32.const 0 i32.const 3 i32.const 2 i32.const 6408 i32.const 5121 i32.const 256 call $read
        i32.const 36160 call $status)
      (func (export "unsafe_client_pointer") (result i32)
        i32.const 0 i32.const 3 i32.const 5126 i32.const 0 i32.const 0 i32.const 1234 call $attrib call $error)
      (func (export "error") (result i32) call $error))"#);
    let mut graphics=Graphics::offscreen(16,16).unwrap();graphics.guest_context=true;graphics.current=true;
    store.data_mut().graphics=Some(graphics);
    let memory=store.data().memory.unwrap();
    let pixels=[255,0,0, 0,255,0, 0,0,255, 0xAA,0xAA,0xAA, 12,34,56, 78,90,123, 210,220,230];
    memory.write(&mut store,1024,&pixels).unwrap();
    let texture=instance.get_typed_func::<i32,()>(&mut store,"texture").unwrap();texture.call(&mut store,1024).unwrap();
    assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"read").unwrap().call(&mut store,()).unwrap(),0x8CD5);
    assert_eq!(&memory.data(&store)[256..280],&[255,0,0,255,0,255,0,255,0,0,255,255,12,34,56,255,78,90,123,255,210,220,230,255]);
    assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"error").unwrap().call(&mut store,()).unwrap(),0);
    assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"unsafe_client_pointer").unwrap().call(&mut store,()).unwrap(),0x502);
    let end=memory.data_size(&store) as i32;
    assert!(texture.call(&mut store,end-5).is_err());
    assert_eq!(store.data().graphics.as_ref().unwrap().draw_calls,0);
}

#[test]
#[ignore="Requires a Windows GPU; validates WebGL allocation and restart compatibility"]
fn webgl_null_buffer_is_zeroed_and_restart_is_enabled(){
 let (mut store,instance)=crate::tests::fixture(r#"(module
 (import "env" "memory" (memory 512))
 (import "env" "_glGenBuffers" (func $gen (param i32 i32)))
 (import "env" "_glBindBuffer" (func $bind (param i32 i32)))
 (import "env" "_glBufferData" (func $data (param i32 i32 i32 i32)))
 (func (export "run") i32.const 1 i32.const 64 call $gen
 i32.const 34962 i32.const 64 i32.load call $bind
 i32.const 34962 i32.const 131075 i32.const 0 i32.const 35044 call $data))"#);
 let mut g=Graphics::offscreen(16,16).unwrap();g.current=true;g.guest_context=true;
 let enabled:unsafe extern "system" fn(u32)->u8=unsafe{std::mem::transmute(g.procedure("glIsEnabled").unwrap())};assert_eq!(unsafe{enabled(0x8D69)},1);
 store.data_mut().graphics=Some(g);instance.get_typed_func::<(),()>(&mut store,"run").unwrap().call(&mut store,()).unwrap();
 let g=store.data_mut().graphics.as_mut().unwrap();let map:unsafe extern "system" fn(u32,isize,isize,u32)->*const u8=unsafe{std::mem::transmute(g.procedure("glMapBufferRange").unwrap())};let unmap:unsafe extern "system" fn(u32)->u8=unsafe{std::mem::transmute(g.procedure("glUnmapBuffer").unwrap())};
 unsafe{let ptr=map(0x8892,0,131075,1);assert!(!ptr.is_null());assert!(std::slice::from_raw_parts(ptr,131075).iter().all(|v|*v==0));assert_eq!(unmap(0x8892),1);}
}
