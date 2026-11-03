//! Primary touch -> pinned Emscripten mouse ABI. No synthetic keyboard events.
use super::{Host,host_error,value_i32,string_at,session::Pointer};
use wasmtime::{Caller,Val,Store,Instance};
#[derive(Clone)]struct Handler{id:u64,event:i32,target:i32,function:i32,user:i32,capture:bool}
#[derive(Default)]pub(super) struct Input{pub enabled:bool,handlers:Vec<Handler>,next:u64,buffer:i32,held:bool,secondary:bool,x:i32,y:i32,pub callbacks:u64,pub events:u64}
fn kind(name:&str)->Option<i32>{match name{"env._emscripten_set_mousedown_callback_on_thread"=>Some(5),"env._emscripten_set_mouseup_callback_on_thread"=>Some(6),"env._emscripten_set_mousemove_callback_on_thread"=>Some(8),_=>None}}
pub(super) fn dispatch(caller:&mut Caller<'_,Host>,name:&str,args:&[Val],results:&mut[Val])->wasmtime::Result<bool>{
    if !caller.data().pointer.enabled{return Ok(false);}let Some(event)=kind(name)else{return Ok(false);};
    let raw=value_i32(args,0)?;
    let target=if raw==0{0}else{match string_at(caller,raw as u32 as usize)?.as_str(){"#window"=>0,"#document"=>1,"#canvas"|"canvas"=>2,_=>{results[0]=Val::I32(-4);return Ok(true);}}};
    let function=value_i32(args,3)?;
    if function==0{caller.data_mut().pointer.handlers.retain(|handler|handler.event!=event||handler.target!=target);results[0]=Val::I32(0);return Ok(true);}
    let table=caller.data().table.ok_or_else(||host_error("Mouse callback requires guest table"))?;
    if function<0||function as u64>=table.size(&*caller)||!matches!(table.get(&mut *caller,function as u64),Some(wasmtime::Ref::Func(Some(_)))){results[0]=Val::I32(-5);return Ok(true);}
    if caller.data().pointer.handlers.len()>=256{results[0]=Val::I32(-6);return Ok(true);}
    if caller.data().pointer.buffer==0{
        let malloc=caller.get_export("_malloc").and_then(|export|export.into_func()).ok_or_else(||host_error("Mouse ABI needs guest malloc"))?;
        let mut result=[Val::I32(0)];malloc.call(&mut *caller,&[Val::I32(72)],&mut result)?;
        let buffer=result[0].i32().ok_or_else(||host_error("Invalid mouse event allocation"))?;
        if buffer==0{results[0]=Val::I32(-6);return Ok(true);}
        caller.data().memory.unwrap().write(&mut *caller,buffer as u32 as usize,&[0;72]).map_err(|e|host_error(e.to_string()))?;
        caller.data_mut().pointer.buffer=buffer;
    }
    let(user,capture)=(value_i32(args,1)?,value_i32(args,2)?!=0);let input=&mut caller.data_mut().pointer;
    input.next=input.next.checked_add(1).ok_or_else(||host_error("Mouse handler ID overflow"))?;
    input.handlers.push(Handler{id:input.next,event,target,function,user,capture});results[0]=Val::I32(0);Ok(true)
}
fn encode(x:i32,y:i32,dx:i32,dy:i32,held:bool,timestamp:f64)->[u8;72]{
    let mut data=[0;72];data[..8].copy_from_slice(&timestamp.to_le_bytes());
    for(offset,value)in [(8,x),(12,y),(16,x),(20,y),(44,dx),(48,dy),(52,x),(56,y),(60,x),(64,y)]{data[offset..offset+4].copy_from_slice(&value.to_le_bytes());}
    data[42..44].copy_from_slice(&(held as u16).to_le_bytes());data
}
pub(super) fn deliver(store:&mut Store<Host>,instance:&Instance,event:Pointer)->anyhow::Result<()>{
    anyhow::ensure!(matches!(event.kind,0..=3),"Invalid pointer action");
    // Unity updates absolute mouse position from mousemove, independently of
    // its button handler. A primary touch must move the compatibility pointer
    // before pressing it, just like a browser's touch-to-mouse event route.
    if event.kind==0||(event.kind==2&&(store.data().pointer.x!=event.x||store.data().pointer.y!=event.y)){
        deliver(store,instance,Pointer{kind:1,x:event.x,y:event.y})?;
    }
    let input=&mut store.data_mut().pointer;
    let(x,y)=if event.kind==3{(input.x,input.y)}else{(event.x,event.y)};
    let(dx,dy)=(x-input.x,y-input.y);input.x=x;input.y=y;
    let kind=match event.kind{0=>{if input.held{return Ok(());}input.held=true;5},1=>8,_=>{if !input.held{return Ok(());}input.held=false;6}};
    send(store,instance,kind,x,y,dx,dy,0)
}
pub(super) fn button(store:&mut Store<Host>,instance:&Instance,button:u8,down:bool)->anyhow::Result<()>{
    anyhow::ensure!(matches!(button,0|2),"Invalid gameplay mouse button");
    let input=&mut store.data_mut().pointer;
    let held=if button==0{&mut input.held}else{&mut input.secondary};
    if *held==down{return Ok(());}*held=down;
    let(x,y)=(input.x,input.y);send(store,instance,if down{5}else{6},x,y,0,0,button)
}
pub(super) fn relative(store:&mut Store<Host>,instance:&Instance,dx:i32,dy:i32)->anyhow::Result<()>{
    let (width,height)=store.data().graphics.as_ref().map(|graphics|graphics.dimensions()).unwrap_or((1280,720));
    let input=&mut store.data_mut().pointer;
    input.x=input.x.saturating_add(dx).clamp(0,width-1);input.y=input.y.saturating_add(dy).clamp(0,height-1);
    let(x,y)=(input.x,input.y);send(store,instance,8,x,y,dx,dy,0)
}
fn send(store:&mut Store<Host>,instance:&Instance,kind:i32,x:i32,y:i32,dx:i32,dy:i32,button:u8)->anyhow::Result<()>{
    store.data_mut().pointer.events+=1;
    let input=&store.data().pointer;let held=input.held;let buttons=(input.held as u16)|((input.secondary as u16)<<1);
    let mut handlers=input.handlers.iter().filter(|handler|handler.event==kind).cloned().collect::<Vec<_>>();
    // Native canvas target, then document, then window: DOM-compatible route.
    handlers.sort_by_key(|handler|if handler.capture{handler.target}else{5-handler.target});
    if handlers.is_empty(){return Ok(());}
    let buffer=store.data().pointer.buffer;let memory=store.data().memory.unwrap();
    let timestamp=store.data().started.map(|clock|clock.elapsed().as_secs_f64()*1000.0).unwrap_or(0.0);
    let mut data=encode(x,y,dx,dy,held,timestamp);data[40..42].copy_from_slice(&(button as u16).to_le_bytes());data[42..44].copy_from_slice(&buttons.to_le_bytes());
    data[28..32].copy_from_slice(&(super::game_input::shift(store.data()) as i32).to_le_bytes());
    let callback=instance.get_typed_func::<(i32,i32,i32,i32),i32>(&mut *store,"dynCall_iiii").map_err(anyhow::Error::from)?;
    for handler in handlers{
        if !store.data().pointer.handlers.iter().any(|current|current.id==handler.id){continue;}
        memory.write(&mut *store,buffer as u32 as usize,&data).map_err(anyhow::Error::from)?;
        callback.call(&mut *store,(handler.function,kind,buffer,handler.user)).map_err(anyhow::Error::from)?;
        store.data_mut().pointer.callbacks+=1;
    }Ok(())
}
pub(super) fn report(host:&Host)->serde_json::Value{serde_json::json!({"enabled":host.pointer.enabled,"primary_pointer_only":false,"buttons":(host.pointer.held as u16)|((host.pointer.secondary as u16)<<1),"handlers":host.pointer.handlers.len(),"events":host.pointer.events,"callbacks":host.pointer.callbacks})}

#[cfg(all(test,feature="compiler"))]mod tests{
    use super::*;
    #[test]fn pinned_mouse_layout_has_timestamp_coordinates_and_u16_buttons(){
        let data=encode(640,320,4,-3,true,12.5);assert_eq!(data.len(),72);
        assert_eq!(&data[..8],&12.5f64.to_le_bytes());assert_eq!(&data[42..44],&1u16.to_le_bytes());
        assert_eq!(&data[48..52],&(-3i32).to_le_bytes());assert_eq!(&data[60..64],&640i32.to_le_bytes());assert_eq!(&data[64..68],&320i32.to_le_bytes());
    }
    #[test]fn pointer_callbacks_receive_press_release_and_cancel_without_keyboard_injection(){
        let(mut store,instance)=crate::tests::fixture(r##"(module
          (type $cb (func (param i32 i32 i32) (result i32)))
          (import "env" "memory" (memory 512)) (import "env" "table" (table 2 funcref))
          (import "env" "_emscripten_set_mousedown_callback_on_thread" (func $down (param i32 i32 i32 i32 i32) (result i32)))
          (import "env" "_emscripten_set_mouseup_callback_on_thread" (func $up (param i32 i32 i32 i32 i32) (result i32)))
          (func (export "_malloc") (param i32) (result i32) i32.const 256)
          (func $callback (type $cb) (param i32 i32 i32) (result i32)
            i32.const 80 local.get 0 i32.store
            i32.const 84 local.get 1 i32.load offset=60 i32.store
            i32.const 88 local.get 1 i32.load16_u offset=42 i32.store
            i32.const 92 local.get 2 i32.store i32.const 1)
          (elem (i32.const 1) $callback)
          (func (export "dynCall_iiii") (param i32 i32 i32 i32) (result i32) local.get 1 local.get 2 local.get 3 local.get 0 call_indirect (type $cb))
          (func (export "register") (result i32)
            i32.const 128 i32.const 99 i32.const 0 i32.const 1 i32.const 2 call $down drop
            i32.const 128 i32.const 99 i32.const 0 i32.const 1 i32.const 2 call $up)
          (data (i32.const 128) "#canvas\00"))"##);
        store.data_mut().pointer.enabled=true;assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"register").unwrap().call(&mut store,()).unwrap(),0);
        deliver(&mut store,&instance,Pointer{kind:0,x:640,y:320}).unwrap();
        let memory=store.data().memory.unwrap();assert_eq!(&memory.data(&store)[80..96],&[5,0,0,0,128,2,0,0,1,0,0,0,99,0,0,0]);
        deliver(&mut store,&instance,Pointer{kind:3,x:0,y:0}).unwrap();assert_eq!(&memory.data(&store)[80..92],&[6,0,0,0,128,2,0,0,0,0,0,0]);
        deliver(&mut store,&instance,Pointer{kind:2,x:0,y:0}).unwrap();assert_eq!(store.data().pointer.callbacks,2);
    }
}
