//! Pinned 164-byte keyboard ABI; events run only on the Store owner thread.
use super::{Host,host_error,value_i32,string_at};
use wasmtime::{Caller,Val,Store,Instance};
#[derive(Clone)]struct Handler{id:u64,event:i32,function:i32,user:i32,capture:bool}
#[derive(Default)]pub(super) struct Input{
    pub(super) enabled:bool,handlers:Vec<Handler>,next:u64,buffer:i32,
    delivered:u64,callbacks:u64,prevented:u64,
}
#[derive(Default,Clone)]pub(super) struct Key{
    pub(super) event:i32,pub(super) key:String,pub(super) code:String,pub(super) location:i32,
    pub(super) modifiers:[bool;4],pub(super) repeat:bool,pub(super) char_code:u32,pub(super) key_code:u32,
}
fn kind(name:&str)->Option<i32>{match name{
    "env._emscripten_set_keypress_callback_on_thread"=>Some(1),
    "env._emscripten_set_keydown_callback_on_thread"=>Some(2),
    "env._emscripten_set_keyup_callback_on_thread"=>Some(3),_=>None}}
pub(super) fn dispatch(caller:&mut Caller<'_,Host>,name:&str,args:&[Val],results:&mut[Val])->wasmtime::Result<bool>{
    if !caller.data().input.enabled{return Ok(false);}
    let Some(event)=kind(name) else{return Ok(false);};
    let target=value_i32(args,0)?;
    // This provider represents the owned native window, not a fabricated DOM.
    if target!=0{
        let target=string_at(caller,target as u32 as usize)?;
        if target!="#window"{results[0]=Val::I32(-4);return Ok(true);}
    }
    let function=value_i32(args,3)?;
    if function==0{caller.data_mut().input.handlers.retain(|handler|handler.event!=event);results[0]=Val::I32(0);return Ok(true);}
    let table=caller.data().table.ok_or_else(||host_error("Keyboard callback requires guest table"))?;
    if function<0||function as u64>=table.size(&*caller)||!matches!(table.get(&mut *caller,function as u64),Some(wasmtime::Ref::Func(Some(_)))){
        results[0]=Val::I32(-5);return Ok(true);
    }
    if caller.data().input.handlers.len()>=256{results[0]=Val::I32(-6);return Ok(true);}
    if caller.data().input.buffer==0{
        let malloc=caller.get_export("_malloc").and_then(|export|export.into_func()).ok_or_else(||host_error("Keyboard event allocation requires guest malloc"))?;
        let mut result=[Val::I32(0)];malloc.call(&mut *caller,&[Val::I32(164)],&mut result)?;
        let buffer=result[0].i32().ok_or_else(||host_error("Invalid keyboard event allocation"))?;
        if buffer==0{results[0]=Val::I32(-6);return Ok(true);}
        let memory=caller.data().memory.unwrap();memory.write(&mut *caller,buffer as u32 as usize,&[0;164]).map_err(|error|host_error(error.to_string()))?;
        caller.data_mut().input.buffer=buffer;
    }
    let (user,capture)=(value_i32(args,1)?,value_i32(args,2)?!=0);
    let input=&mut caller.data_mut().input;input.next=input.next.checked_add(1).ok_or_else(||host_error("Keyboard handler ID overflow"))?;
    input.handlers.push(Handler{id:input.next,event,function,user,capture});results[0]=Val::I32(0);Ok(true)
}
fn bytes(key:&Key)->[u8;164]{
    fn text(output:&mut[u8],value:&str){let mut length=value.len().min(output.len()-1);while !value.is_char_boundary(length){length-=1;}output[..length].copy_from_slice(&value.as_bytes()[..length]);}
    let mut output=[0;164];text(&mut output[0..32],&key.key);text(&mut output[32..64],&key.code);
    for(offset,value)in [(64,key.location),(68,key.modifiers[0] as i32),(72,key.modifiers[1] as i32),(76,key.modifiers[2] as i32),(80,key.modifiers[3] as i32),(84,key.repeat as i32),(152,key.char_code as i32),(156,key.key_code as i32),(160,key.key_code as i32)]{output[offset..offset+4].copy_from_slice(&value.to_le_bytes());}
    // DOM KeyboardEvent.char and locale are absent on this native provider.
    output
}
pub(super) fn deliver(store:&mut Store<Host>,instance:&Instance,key:Key)->anyhow::Result<bool>{
    anyhow::ensure!(matches!(key.event,1..=3),"Invalid keyboard event type");
    let mut handlers=store.data().input.handlers.iter().filter(|handler|handler.event==key.event).cloned().collect::<Vec<_>>();
    handlers.sort_by_key(|handler|!handler.capture);
    store.data_mut().input.delivered+=1;
    if handlers.is_empty(){return Ok(false);}
    let buffer=store.data().input.buffer;let memory=store.data().memory.unwrap();
    let data=bytes(&key);
    let callback=instance.get_typed_func::<(i32,i32,i32,i32),i32>(&mut *store,"dynCall_iiii").map_err(anyhow::Error::from)?;
    let mut prevent_default=false;
    for handler in handlers{
        if !store.data().input.handlers.iter().any(|current|current.id==handler.id){continue;}
        memory.write(&mut *store,buffer as u32 as usize,&data).map_err(anyhow::Error::from)?;
        let prevented=callback.call(&mut *store,(handler.function,key.event,buffer,handler.user)).map_err(anyhow::Error::from)?;
        store.data_mut().input.callbacks+=1;if prevented!=0{store.data_mut().input.prevented+=1;prevent_default=true;}
    }Ok(prevent_default)
}
pub(super) fn report(host:&Host)->serde_json::Value{serde_json::json!({"backend":if host.input.enabled{if host.live{"android-touch-keyboard"}else{"native-keyboard-callbacks"}}else{"unavailable"},"handlers":host.input.handlers.len(),"events":host.input.delivered,"callbacks":host.input.callbacks,"prevent_default_requests":host.input.prevented,"physical_keyboard_validated":false,"virtual_controls":super::game_input::report(host)})}

#[cfg(all(test,feature="compiler"))]
mod tests{
    use super::*;
    #[test]fn keyboard_abi_truncates_utf8_on_character_boundaries(){
        let key=Key{event:2,key:"ş".repeat(20),code:"Space".into(),modifiers:[true,false,true,false],repeat:true,key_code:32,..Default::default()};
        let data=bytes(&key);assert_eq!(&data[..30],"ş".repeat(15).as_bytes());assert_eq!(&data[30..32],&[0,0]);
        assert_eq!(&data[68..72],&1i32.to_le_bytes());assert_eq!(&data[72..76],&0i32.to_le_bytes());
        assert_eq!(&data[156..164],&[32,0,0,0,32,0,0,0]);assert_eq!(&data[88..152],&[0;64]);
    }
    #[test]fn real_guest_callback_can_unregister_remaining_handlers(){
        let(mut store,instance)=crate::tests::fixture(r#"(module
          (type $callback (func (param i32 i32 i32) (result i32)))
          (import "env" "memory" (memory 512)) (import "env" "table" (table 4 funcref))
          (import "env" "_emscripten_set_keydown_callback_on_thread" (func $register (param i32 i32 i32 i32 i32) (result i32)))
          (func (export "_malloc") (param i32) (result i32) i32.const 256)
          (func $first (type $callback) (param i32 i32 i32) (result i32)
            i32.const 64 local.get 0 i32.store
            i32.const 68 local.get 2 i32.store
            i32.const 72 local.get 1 i32.load offset=156 i32.store
            i32.const 0 i32.const 0 i32.const 0 i32.const 0 i32.const 2 call $register drop i32.const 1)
          (func $second (type $callback) (param i32 i32 i32) (result i32) unreachable)
          (elem (i32.const 1) $first $second)
          (func (export "dynCall_iiii") (param i32 i32 i32 i32) (result i32) local.get 1 local.get 2 local.get 3 local.get 0 call_indirect (type $callback))
          (func (export "register") (param i32) (result i32) i32.const 0 i32.const 99 i32.const 0 local.get 0 i32.const 2 call $register))"#);
        let register=instance.get_typed_func::<i32,i32>(&mut store,"register").unwrap();
        assert!(register.call(&mut store,1).is_err());store.data_mut().input.enabled=true;
        assert_eq!(register.call(&mut store,3).unwrap(),-5);
        assert_eq!(register.call(&mut store,1).unwrap(),0);assert_eq!(register.call(&mut store,2).unwrap(),0);
        let key=Key{event:2,key:" ".into(),code:"Space".into(),key_code:32,..Default::default()};
        deliver(&mut store,&instance,key.clone()).unwrap();
        let memory=store.data().memory.unwrap();assert_eq!(&memory.data(&store)[64..76],&[2,0,0,0,99,0,0,0,32,0,0,0]);
        assert_eq!(store.data().input.callbacks,1);assert_eq!(store.data().input.prevented,1);
        assert!(store.data().input.handlers.is_empty());deliver(&mut store,&instance,key).unwrap();
        assert_eq!(store.data().input.callbacks,1);
    }
}

#[cfg(windows)]
pub(super) fn native_key(message:&windows_sys::Win32::UI::WindowsAndMessaging::MSG)->Option<Key>{
    use windows_sys::Win32::UI::{WindowsAndMessaging::*,Input::KeyboardAndMouse::*};
    let event=match message.message{WM_CHAR|WM_SYSCHAR=>1,WM_KEYDOWN|WM_SYSKEYDOWN=>2,WM_KEYUP|WM_SYSKEYUP=>3,_=>return None};
    let vk=message.wParam as u32;let scan=((message.lParam as u64>>16)&255) as u32;
    let extended=message.lParam as u64&(1<<24)!=0;
    let down=|key:u16|unsafe{GetKeyState(key as i32) as u16&0x8000!=0};
    let modifiers=[down(VK_CONTROL),down(VK_SHIFT),down(VK_MENU),down(VK_LWIN)||down(VK_RWIN)];
    let mut code=match scan{
        0x39=>"Space",0x1C=>if extended{"NumpadEnter"}else{"Enter"},0x01=>"Escape",0x0F=>"Tab",0x0E=>"Backspace",
        0x1E=>"KeyA",0x30=>"KeyB",0x2E=>"KeyC",0x20=>"KeyD",0x12=>"KeyE",0x21=>"KeyF",0x22=>"KeyG",0x23=>"KeyH",0x17=>"KeyI",0x24=>"KeyJ",0x25=>"KeyK",0x26=>"KeyL",0x32=>"KeyM",0x31=>"KeyN",0x18=>"KeyO",0x19=>"KeyP",0x10=>"KeyQ",0x13=>"KeyR",0x1F=>"KeyS",0x14=>"KeyT",0x16=>"KeyU",0x2F=>"KeyV",0x11=>"KeyW",0x2D=>"KeyX",0x15=>"KeyY",0x2C=>"KeyZ",_=>"Unidentified"}.to_string();
    if (0x02..=0x0B).contains(&scan){code=format!("Digit{}",(scan-1)%10);}
    let location=if (96..=111).contains(&vk)||(vk==13&&extended){3}else if matches!(vk,16..=18){if extended||scan==0x36{2}else{1}}else{0};
    let key=if event==1{char::from_u32(vk)?.to_string()}else{match vk{
        8=>"Backspace".into(),9=>"Tab".into(),13=>"Enter".into(),16=>"Shift".into(),17=>"Control".into(),18=>"Alt".into(),27=>"Escape".into(),32=>" ".into(),37=>"ArrowLeft".into(),38=>"ArrowUp".into(),39=>"ArrowRight".into(),40=>"ArrowDown".into(),46=>"Delete".into(),112..=135=>format!("F{}",vk-111),
        _=>{let mut state=[0u8;256];let mut output=[0u16;16];let layout=unsafe{GetKeyboardLayout(0)};
            if unsafe{GetKeyboardState(state.as_mut_ptr())}==0{return None;}
            let count=unsafe{ToUnicodeEx(vk,scan,state.as_ptr(),output.as_mut_ptr(),16,4,layout)};
            if count<0{"Dead".into()}else if count>0{String::from_utf16_lossy(&output[..count as usize])}else{"Unidentified".into()}}
    }};
    Some(Key{event,key,code,location,modifiers,repeat:event!=3&&message.lParam as u64&(1<<30)!=0,char_code:if event==1{vk}else{0},key_code:vk})
}
