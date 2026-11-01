//! Virtual controls deliver the pinned keyboard/mouse ABI on the VM owner.
use super::{Host,input::Key,session::GameEvent};
use wasmtime::{Store,Instance};
#[derive(Default)]pub(super) struct State{keys:std::collections::BTreeSet<u32>,pub events:u64}
pub(super) fn shift(host:&Host)->bool{host.game_input.keys.contains(&16)}
pub(super) fn report(host:&Host)->serde_json::Value{serde_json::json!({"operations":host.game_input.events,"held_keys":host.game_input.keys})}
pub(crate) fn supported(code:u32)->bool{matches!(code,13|16|17|27|32|49..=53|65|67|68|69|70|77|81|82|83|87|88|112..=118)}
fn key(code:u32,down:bool,shift:bool)->Key{
    let(name,physical,location)=match code{
        13=>("Enter".into(),"Enter".into(),0),16=>("Shift".into(),"ShiftLeft".into(),1),
        17=>("Control".into(),"ControlLeft".into(),1),27=>("Escape".into(),"Escape".into(),0),32=>(" ".into(),"Space".into(),0),
        112..=118=>(format!("F{}",code-111),format!("F{}",code-111),0),
        49..=53=>((code as u8 as char).to_string(),format!("Digit{}",code as u8 as char),0),
        _=>((code as u8 as char).to_ascii_lowercase().to_string(),format!("Key{}",code as u8 as char),0),
    };Key{event:if down{2}else{3},key:name,code:physical,location,key_code:code,modifiers:[false,shift,false,false],..Default::default()}
}
pub(super) fn deliver(store:&mut Store<Host>,instance:&Instance,event:GameEvent)->anyhow::Result<()>{
    match event{
        GameEvent::Reset=>{
            let keys=store.data().game_input.keys.iter().copied().collect::<Vec<_>>();
            for code in keys{deliver(store,instance,GameEvent::Key{code,down:false})?;}
            super::pointer::button(store,instance,0,false)?;super::pointer::button(store,instance,2,false)?;
        }
        GameEvent::Key{code,down}=>{
            anyhow::ensure!(supported(code),"Unsupported virtual key");
            let changed=if down{store.data_mut().game_input.keys.insert(code)}else{store.data_mut().game_input.keys.remove(&code)};
            if changed{let shift=store.data().game_input.keys.contains(&16);let mut event=key(code,down,shift);
                event.modifiers[0]=store.data().game_input.keys.contains(&17);
                super::input::deliver(store,instance,event)?;}
        }
        GameEvent::Button{button,down}=>super::pointer::button(store,instance,button,down)?,
        GameEvent::Look{dx,dy}=>super::pointer::relative(store,instance,dx,dy)?,
    }store.data_mut().game_input.events+=1;Ok(())
}
#[cfg(all(test,feature="compiler"))]mod tests{
    use super::*;
    #[test]fn vehicle_function_keys_use_real_keyboard_names_and_codes(){
        assert!(supported(13));assert!(supported(17));
        let enter=key(13,true,false);assert_eq!(enter.key,"Enter");assert_eq!(enter.code,"Enter");assert_eq!(enter.key_code,13);
        let crouch=key(17,true,false);assert_eq!(crouch.key,"Control");assert_eq!(crouch.code,"ControlLeft");assert_eq!(crouch.location,1);
        for code in 112..=118{assert!(supported(code));let k=key(code,true,false);assert_eq!(k.key,format!("F{}",code-111));assert_eq!(k.code,k.key);assert_eq!(k.key_code,code);}
        assert!(supported(88));assert!(!supported(119));
    }
    #[test]fn keyboard_and_two_mouse_buttons_share_real_callbacks_and_reset_releases_everything(){
        let names=["keydown","keyup","mousedown","mouseup","mousemove"];
        let mut wat=String::from(r#"(module (type $cb (func (param i32 i32 i32) (result i32))) (import "env" "memory" (memory 512)) (import "env" "table" (table 2 funcref))"#);
        for (i,name) in names.iter().enumerate(){wat.push_str(&format!(r#"(import "env" "_emscripten_set_{name}_callback_on_thread" (func $reg{i} (param i32 i32 i32 i32 i32) (result i32)))"#));}
        wat.push_str(r#"(global $heap (mut i32) (i32.const 4096))
            (func (export "_malloc") (param i32) (result i32) (local i32) global.get $heap local.tee 1 local.get 0 i32.add global.set $heap local.get 1)
            (func $callback (type $cb) (param i32 i32 i32) (result i32)
                i32.const 64 local.get 0 i32.store
                i32.const 68 local.get 1 i32.load16_u offset=40 i32.store
                i32.const 72 local.get 1 i32.load16_u offset=42 i32.store
                i32.const 76 local.get 1 i32.load offset=44 i32.store
                i32.const 80 local.get 1 i32.load offset=48 i32.store
                i32.const 84 local.get 1 i32.load offset=156 i32.store
                i32.const 88 local.get 1 i32.load i32.store
                i32.const 96 local.get 1 i32.load offset=68 i32.store i32.const 0)
            (elem (i32.const 1) $callback)
            (func (export "dynCall_iiii") (param i32 i32 i32 i32) (result i32) local.get 1 local.get 2 local.get 3 local.get 0 call_indirect (type $cb))
            (func (export "register")"#);
        for i in 0..names.len(){wat.push_str(&format!("i32.const 0 i32.const 0 i32.const 0 i32.const 1 i32.const 2 call $reg{i} drop "));}wat.push_str("))");
        let(mut store,instance)=crate::tests::fixture(&wat);store.data_mut().input.enabled=true;store.data_mut().pointer.enabled=true;
        instance.get_typed_func::<(),()>(&mut store,"register").unwrap().call(&mut store,()).unwrap();
        for event in [GameEvent::Key{code:87,down:true},GameEvent::Key{code:16,down:true},GameEvent::Key{code:87,down:true},GameEvent::Button{button:0,down:true},GameEvent::Button{button:2,down:true},GameEvent::Look{dx:32,dy:-12}]{deliver(&mut store,&instance,event).unwrap();}
        let memory=store.data().memory.unwrap();let word=|s:&Store<Host>,address|i32::from_le_bytes(memory.data(s)[address..address+4].try_into().unwrap());
        assert_eq!(word(&store,64),8);assert_eq!(word(&store,72),3);assert_eq!(word(&store,76),32);assert_eq!(word(&store,80),-12);
        deliver(&mut store,&instance,GameEvent::Button{button:0,down:false}).unwrap();assert_eq!(word(&store,72),2);
        deliver(&mut store,&instance,GameEvent::Reset).unwrap();assert!(store.data().game_input.keys.is_empty());assert_eq!(word(&store,64),6);assert_eq!(word(&store,68),2);assert_eq!(word(&store,72),0);
        assert_eq!(super::super::input::report(store.data())["callbacks"],4);
        for code in 112..=118{deliver(&mut store,&instance,GameEvent::Key{code,down:true}).unwrap();assert_eq!(word(&store,84),code as i32);assert_eq!(word(&store,88),i32::from_le_bytes([b'F',b'1'+(code-112) as u8,0,0]));deliver(&mut store,&instance,GameEvent::Key{code,down:false}).unwrap();}
        deliver(&mut store,&instance,GameEvent::Key{code:17,down:true}).unwrap();assert_eq!(word(&store,84),17);assert_eq!(word(&store,96),1);
        deliver(&mut store,&instance,GameEvent::Key{code:87,down:true}).unwrap();assert_eq!(word(&store,96),1);
        deliver(&mut store,&instance,GameEvent::Key{code:17,down:false}).unwrap();assert_eq!(word(&store,96),0);
        deliver(&mut store,&instance,GameEvent::Reset).unwrap();
        deliver(&mut store,&instance,GameEvent::Key{code:13,down:true}).unwrap();assert_eq!(word(&store,84),13);assert_eq!(word(&store,88),i32::from_le_bytes(*b"Ente"));
        deliver(&mut store,&instance,GameEvent::Key{code:13,down:false}).unwrap();
        assert!(store.data().game_input.keys.is_empty());
    }
}
