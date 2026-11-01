//! Legacy JS exception ABI: guest RTTI and destructors remain guest code.
use super::{Host,host_error,value_i32,read_u32,write_i32};
use std::{collections::BTreeMap,rc::Rc};
use wasmtime::{Caller,Val,Func,TypedFunc};
#[derive(Default)]pub(super) struct Exceptions{last:i32,caught:Vec<i32>,infos:BTreeMap<i32,Info>,scratch:i32,uncaught:u32,helpers:BTreeMap<String,Func>,stack_global:Option<Option<wasmtime::Global>>,stack_save:Option<Option<TypedFunc<(),i32>>>,pub(super) invokes:Vec<Option<Rc<CachedInvoke>>>}
impl Exceptions{pub(super) fn stack_global_active(&self)->bool{self.stack_global.flatten().is_some()}}
pub(super) struct CachedInvoke{function:Func,call:InvokeCall}
enum InvokeCall{
    V(TypedFunc<i32,()>),Vi(TypedFunc<(i32,i32),()>),Vii(TypedFunc<(i32,i32,i32),()>),
    Viii(TypedFunc<(i32,i32,i32,i32),()>),Viiii(TypedFunc<(i32,i32,i32,i32,i32),()>),
    Ii(TypedFunc<(i32,i32),i32>),Iii(TypedFunc<(i32,i32,i32),i32>),
    Iiii(TypedFunc<(i32,i32,i32,i32),i32>),Iiiii(TypedFunc<(i32,i32,i32,i32,i32),i32>),Generic,
    I(TypedFunc<i32,i32>),I5(TypedFunc<(i32,i32,i32,i32,i32,i32),i32>),
    I6(TypedFunc<(i32,i32,i32,i32,i32,i32,i32),i32>),
    V5(TypedFunc<(i32,i32,i32,i32,i32,i32),()>),V6(TypedFunc<(i32,i32,i32,i32,i32,i32,i32),()>),
    F(TypedFunc<i32,f64>),Fi(TypedFunc<(i32,i32),f64>),Fii(TypedFunc<(i32,i32,i32),f64>),Fiii(TypedFunc<(i32,i32,i32,i32),f64>),
    Fffi(TypedFunc<(i32,f64,f64,i32),f64>),

// BEGIN GENERATED ENUM
    Extra0(TypedFunc<(i32,f64,i32),i32>),
    Extra1(TypedFunc<(i32,i32,f64,i32),()>),
    Extra2(TypedFunc<(i32,i32,i32,f64,i32),()>),
    Extra3(TypedFunc<(i32,f64,f64,f64,i32),f64>),
    Extra4(TypedFunc<(i32,i32,i32,i32,i32),i32>),
    Extra5(TypedFunc<(i32,i32,f64,i32),i32>),
    Extra6(TypedFunc<(i32,i32,i32,i32,f64,i32,i32),()>),
    Extra7(TypedFunc<(i32,i32,i32,f64),()>),
    Extra8(TypedFunc<(i32,f64,f64),f64>),
    Extra9(TypedFunc<(i32,i32,f64,f64,f64,f64,i32),()>),
    Extra10(TypedFunc<(i32,i32,i32,i32,f64,i32),()>),
    Extra11(TypedFunc<(i32,f64,i32),f64>),
    Extra12(TypedFunc<(i32,i32,i32,f64,i32,i32,i32),i32>),
    Extra13(TypedFunc<(i32,i32,i32,f64,i32,i32),i32>),
    Extra14(TypedFunc<(i32,i32,i32,i32,f64,i32,i32,i32),i32>),
    Extra15(TypedFunc<(i32,i32,i32,f64,i32,i32,i32,i32),i32>),
    Extra16(TypedFunc<(i32,i32,f64),()>),
    Extra17(TypedFunc<(i32,i32,i32,f64,f64),()>),
    Extra18(TypedFunc<(i32,i32,f64,f64,i32),()>),
    Extra19(TypedFunc<(i32,i32,f64,f64,f64,f64,i32,i32),()>),
    Extra20(TypedFunc<(i32,i32,i32,f64,i32,i32),()>),
    Extra21(TypedFunc<(i32,i32,i32,i32,i32,f64,i32,i32,i32),i32>),
    Extra22(TypedFunc<(i32,f64,f64,f64),()>),
    Extra23(TypedFunc<(i32,f64,f64,f64,i32),()>),
    Extra24(TypedFunc<(i32,i32,i32,i32,i32),f64>),
    Extra25(TypedFunc<(i32,i32,i32,i32,i32,i32,i32,i32),()>),
    Extra26(TypedFunc<(i32,i32,i32,i32,f64,i32,i32),i32>),
    Extra27(TypedFunc<(i32,i32,i32,i32,f64,f64,i32),()>),
    Extra28(TypedFunc<(i32,i32,i32,f64,i32),i32>),
    Extra29(TypedFunc<(i32,f64,f64,i32),()>),
    Extra30(TypedFunc<(i32,i32,i32),i32>),
    Extra31(TypedFunc<(i32,i32,i32,i32,i32,i32,i32,f64,i32,i32,i32),i32>),
// END GENERATED ENUM
}
impl CachedInvoke{
    fn new(caller:&mut Caller<'_,Host>,name:&str)->wasmtime::Result<Self>{
        let function=helper(caller,name)?;
        let call=match name{
            "dynCall_v"=>InvokeCall::V(function.typed(&*caller)?),"dynCall_vi"=>InvokeCall::Vi(function.typed(&*caller)?),
            "dynCall_vii"=>InvokeCall::Vii(function.typed(&*caller)?),"dynCall_viii"=>InvokeCall::Viii(function.typed(&*caller)?),
            "dynCall_viiii"=>InvokeCall::Viiii(function.typed(&*caller)?),"dynCall_ii"=>InvokeCall::Ii(function.typed(&*caller)?),
            "dynCall_iii"=>InvokeCall::Iii(function.typed(&*caller)?),"dynCall_iiii"=>InvokeCall::Iiii(function.typed(&*caller)?),
            "dynCall_iiiii"=>InvokeCall::Iiiii(function.typed(&*caller)?),
            "dynCall_i"=>InvokeCall::I(function.typed(&*caller)?),"dynCall_iiiiii"=>InvokeCall::I5(function.typed(&*caller)?),
            "dynCall_iiiiiii"=>InvokeCall::I6(function.typed(&*caller)?),"dynCall_viiiii"=>InvokeCall::V5(function.typed(&*caller)?),
            "dynCall_viiiiii"=>InvokeCall::V6(function.typed(&*caller)?),"dynCall_f"=>InvokeCall::F(function.typed(&*caller)?),
            "dynCall_fi"=>InvokeCall::Fi(function.typed(&*caller)?),"dynCall_fii"=>InvokeCall::Fii(function.typed(&*caller)?),
            "dynCall_fiii"=>InvokeCall::Fiii(function.typed(&*caller)?),"dynCall_fffi"=>InvokeCall::Fffi(function.typed(&*caller)?),
// BEGIN GENERATED CACHE
            "dynCall_ifi"=>InvokeCall::Extra0(function.typed(&*caller)?),
            "dynCall_vifi"=>InvokeCall::Extra1(function.typed(&*caller)?),
            "dynCall_viifi"=>InvokeCall::Extra2(function.typed(&*caller)?),
            "dynCall_ffffi"=>InvokeCall::Extra3(function.typed(&*caller)?),
            "dynCall_jiji"=>InvokeCall::Extra4(function.typed(&*caller)?),
            "dynCall_iifi"=>InvokeCall::Extra5(function.typed(&*caller)?),
            "dynCall_viiifii"=>InvokeCall::Extra6(function.typed(&*caller)?),
            "dynCall_viif"=>InvokeCall::Extra7(function.typed(&*caller)?),
            "dynCall_fff"=>InvokeCall::Extra8(function.typed(&*caller)?),
            "dynCall_viffffi"=>InvokeCall::Extra9(function.typed(&*caller)?),
            "dynCall_viiifi"=>InvokeCall::Extra10(function.typed(&*caller)?),
            "dynCall_ffi"=>InvokeCall::Extra11(function.typed(&*caller)?),
            "dynCall_iiifiii"=>InvokeCall::Extra12(function.typed(&*caller)?),
            "dynCall_iiifii"=>InvokeCall::Extra13(function.typed(&*caller)?),
            "dynCall_iiiifiii"=>InvokeCall::Extra14(function.typed(&*caller)?),
            "dynCall_iiifiiii"=>InvokeCall::Extra15(function.typed(&*caller)?),
            "dynCall_vif"=>InvokeCall::Extra16(function.typed(&*caller)?),
            "dynCall_viiff"=>InvokeCall::Extra17(function.typed(&*caller)?),
            "dynCall_viffi"=>InvokeCall::Extra18(function.typed(&*caller)?),
            "dynCall_viffffii"=>InvokeCall::Extra19(function.typed(&*caller)?),
            "dynCall_viifii"=>InvokeCall::Extra20(function.typed(&*caller)?),
            "dynCall_iiiiifiii"=>InvokeCall::Extra21(function.typed(&*caller)?),
            "dynCall_vfff"=>InvokeCall::Extra22(function.typed(&*caller)?),
            "dynCall_vfffi"=>InvokeCall::Extra23(function.typed(&*caller)?),
            "dynCall_fiiii"=>InvokeCall::Extra24(function.typed(&*caller)?),
            "dynCall_viiiiiii"=>InvokeCall::Extra25(function.typed(&*caller)?),
            "dynCall_iiiifii"=>InvokeCall::Extra26(function.typed(&*caller)?),
            "dynCall_viiiffi"=>InvokeCall::Extra27(function.typed(&*caller)?),
            "dynCall_iiifi"=>InvokeCall::Extra28(function.typed(&*caller)?),
            "dynCall_vffi"=>InvokeCall::Extra29(function.typed(&*caller)?),
            "dynCall_jii"=>InvokeCall::Extra30(function.typed(&*caller)?),
            "dynCall_iiiiiiifiii"=>InvokeCall::Extra31(function.typed(&*caller)?),
// END GENERATED CACHE
            _=>InvokeCall::Generic,
        };Ok(Self{function,call})
    }
    fn call(&self,caller:&mut Caller<'_,Host>,args:&[Val],results:&mut[Val])->wasmtime::Result<()>{
        let i=|index|value_i32(args,index);
        match &self.call{
            InvokeCall::V(f)=>f.call(caller,i(0)?),InvokeCall::Vi(f)=>f.call(caller,(i(0)?,i(1)?)),
            InvokeCall::Vii(f)=>f.call(caller,(i(0)?,i(1)?,i(2)?)),InvokeCall::Viii(f)=>f.call(caller,(i(0)?,i(1)?,i(2)?,i(3)?)),
            InvokeCall::Viiii(f)=>f.call(caller,(i(0)?,i(1)?,i(2)?,i(3)?,i(4)?)),
            InvokeCall::Ii(f)=>{results[0]=Val::I32(f.call(caller,(i(0)?,i(1)?))?);Ok(())},
            InvokeCall::Iii(f)=>{results[0]=Val::I32(f.call(caller,(i(0)?,i(1)?,i(2)?))?);Ok(())},
            InvokeCall::Iiii(f)=>{results[0]=Val::I32(f.call(caller,(i(0)?,i(1)?,i(2)?,i(3)?))?);Ok(())},
            InvokeCall::Iiiii(f)=>{results[0]=Val::I32(f.call(caller,(i(0)?,i(1)?,i(2)?,i(3)?,i(4)?))?);Ok(())},
            _=>self.function.call(caller,args,results),
        }
    }
}
/// Typed import entry avoids Wasmtime's generic incoming Val conversion too.
/// Instantiation still validates the import type against the actual module.
pub(super) fn bind_typed_import(linker:&mut wasmtime::Linker<Host>,space:&str,name:&str,trampoline:&str,slot:usize,counter:usize)->wasmtime::Result<bool>{
    if space!="env"{return Ok(false);}
    // Keep the typed arguments typed all the way into dynCall. Building Val
    // arrays and selecting an enum again used to happen on every hot invoke.
    macro_rules! bind {
        ($variant:ident, $result:ty, $zero:expr; $($arg:ident:$ty:ty),+) => {{
            let trampoline=trampoline.to_owned();
            linker.func_wrap(space,name,move |mut caller:Caller<'_,Host>,$($arg:$ty),+|->wasmtime::Result<$result>{
                caller.data_mut().calls[counter].1+=1;
                if caller.data().exceptions.invokes[slot].is_none(){
                    let cached=CachedInvoke::new(&mut caller,&trampoline)?;
                    caller.data_mut().exceptions.invokes[slot]=Some(Rc::new(cached));
                }
                // The VM and this cache have one owner thread. Keep immutable
                // handles alive across reentry without cloning FuncType's atomic
                // engine references on every invocation. No cache borrow crosses call.
                let cached=Rc::clone(caller.data().exceptions.invokes[slot].as_ref().unwrap());
                let InvokeCall::$variant(function)=&cached.call else{
                    return Err(host_error("Typed invoke cache signature mismatch"));
                };
                let stack=save_stack(&mut caller)?;
                match function.call(&mut caller,bind!(@tuple $($arg),+)){
                    Ok(value)=>Ok(value),
                    Err(error) if error.downcast_ref::<Thrown>().is_some()=>{
                        restore_throw(&mut caller,stack)?;Ok($zero)
                    },
                    Err(error)=>Err(error),
                }
            })?;
        }};
        (@tuple $a:ident)=>{$a};
        (@tuple $($arg:ident),+)=>{($($arg),+)};
    }
    match name{
        "invoke_v"=>bind!(V,(),();a:i32),"invoke_vi"=>bind!(Vi,(),();a:i32,b:i32),
        "invoke_vii"=>bind!(Vii,(),();a:i32,b:i32,c:i32),"invoke_viii"=>bind!(Viii,(),();a:i32,b:i32,c:i32,d:i32),
        "invoke_viiii"=>bind!(Viiii,(),();a:i32,b:i32,c:i32,d:i32,e:i32),"invoke_ii"=>bind!(Ii,i32,0;a:i32,b:i32),
        "invoke_iii"=>bind!(Iii,i32,0;a:i32,b:i32,c:i32),"invoke_iiii"=>bind!(Iiii,i32,0;a:i32,b:i32,c:i32,d:i32),
        "invoke_iiiii"=>bind!(Iiiii,i32,0;a:i32,b:i32,c:i32,d:i32,e:i32),
        "invoke_i"=>bind!(I,i32,0;a:i32),"invoke_iiiiii"=>bind!(I5,i32,0;a:i32,b:i32,c:i32,d:i32,e:i32,f:i32),
        "invoke_iiiiiii"=>bind!(I6,i32,0;a:i32,b:i32,c:i32,d:i32,e:i32,f:i32,g:i32),
        "invoke_viiiii"=>bind!(V5,(),();a:i32,b:i32,c:i32,d:i32,e:i32,f:i32),
        "invoke_viiiiii"=>bind!(V6,(),();a:i32,b:i32,c:i32,d:i32,e:i32,f:i32,g:i32),
        "invoke_f"=>bind!(F,f64,f64::NAN;a:i32),"invoke_fi"=>bind!(Fi,f64,f64::NAN;a:i32,b:i32),
        "invoke_fii"=>bind!(Fii,f64,f64::NAN;a:i32,b:i32,c:i32),"invoke_fiii"=>bind!(Fiii,f64,f64::NAN;a:i32,b:i32,c:i32,d:i32),
        "invoke_fffi"=>bind!(Fffi,f64,f64::NAN;a:i32,b:f64,c:f64,d:i32),
// BEGIN GENERATED BIND
        "invoke_ifi"=>bind!(Extra0,i32,0;a0:i32,a1:f64,a2:i32),
        "invoke_vifi"=>bind!(Extra1,(),();a0:i32,a1:i32,a2:f64,a3:i32),
        "invoke_viifi"=>bind!(Extra2,(),();a0:i32,a1:i32,a2:i32,a3:f64,a4:i32),
        "invoke_ffffi"=>bind!(Extra3,f64,f64::NAN;a0:i32,a1:f64,a2:f64,a3:f64,a4:i32),
        "invoke_jiji"=>bind!(Extra4,i32,0;a0:i32,a1:i32,a2:i32,a3:i32,a4:i32),
        "invoke_iifi"=>bind!(Extra5,i32,0;a0:i32,a1:i32,a2:f64,a3:i32),
        "invoke_viiifii"=>bind!(Extra6,(),();a0:i32,a1:i32,a2:i32,a3:i32,a4:f64,a5:i32,a6:i32),
        "invoke_viif"=>bind!(Extra7,(),();a0:i32,a1:i32,a2:i32,a3:f64),
        "invoke_fff"=>bind!(Extra8,f64,f64::NAN;a0:i32,a1:f64,a2:f64),
        "invoke_viffffi"=>bind!(Extra9,(),();a0:i32,a1:i32,a2:f64,a3:f64,a4:f64,a5:f64,a6:i32),
        "invoke_viiifi"=>bind!(Extra10,(),();a0:i32,a1:i32,a2:i32,a3:i32,a4:f64,a5:i32),
        "invoke_ffi"=>bind!(Extra11,f64,f64::NAN;a0:i32,a1:f64,a2:i32),
        "invoke_iiifiii"=>bind!(Extra12,i32,0;a0:i32,a1:i32,a2:i32,a3:f64,a4:i32,a5:i32,a6:i32),
        "invoke_iiifii"=>bind!(Extra13,i32,0;a0:i32,a1:i32,a2:i32,a3:f64,a4:i32,a5:i32),
        "invoke_iiiifiii"=>bind!(Extra14,i32,0;a0:i32,a1:i32,a2:i32,a3:i32,a4:f64,a5:i32,a6:i32,a7:i32),
        "invoke_iiifiiii"=>bind!(Extra15,i32,0;a0:i32,a1:i32,a2:i32,a3:f64,a4:i32,a5:i32,a6:i32,a7:i32),
        "invoke_vif"=>bind!(Extra16,(),();a0:i32,a1:i32,a2:f64),
        "invoke_viiff"=>bind!(Extra17,(),();a0:i32,a1:i32,a2:i32,a3:f64,a4:f64),
        "invoke_viffi"=>bind!(Extra18,(),();a0:i32,a1:i32,a2:f64,a3:f64,a4:i32),
        "invoke_viffffii"=>bind!(Extra19,(),();a0:i32,a1:i32,a2:f64,a3:f64,a4:f64,a5:f64,a6:i32,a7:i32),
        "invoke_viifii"=>bind!(Extra20,(),();a0:i32,a1:i32,a2:i32,a3:f64,a4:i32,a5:i32),
        "invoke_iiiiifiii"=>bind!(Extra21,i32,0;a0:i32,a1:i32,a2:i32,a3:i32,a4:i32,a5:f64,a6:i32,a7:i32,a8:i32),
        "invoke_vfff"=>bind!(Extra22,(),();a0:i32,a1:f64,a2:f64,a3:f64),
        "invoke_vfffi"=>bind!(Extra23,(),();a0:i32,a1:f64,a2:f64,a3:f64,a4:i32),
        "invoke_fiiii"=>bind!(Extra24,f64,f64::NAN;a0:i32,a1:i32,a2:i32,a3:i32,a4:i32),
        "invoke_viiiiiii"=>bind!(Extra25,(),();a0:i32,a1:i32,a2:i32,a3:i32,a4:i32,a5:i32,a6:i32,a7:i32),
        "invoke_iiiifii"=>bind!(Extra26,i32,0;a0:i32,a1:i32,a2:i32,a3:i32,a4:f64,a5:i32,a6:i32),
        "invoke_viiiffi"=>bind!(Extra27,(),();a0:i32,a1:i32,a2:i32,a3:i32,a4:f64,a5:f64,a6:i32),
        "invoke_iiifi"=>bind!(Extra28,i32,0;a0:i32,a1:i32,a2:i32,a3:f64,a4:i32),
        "invoke_vffi"=>bind!(Extra29,(),();a0:i32,a1:f64,a2:f64,a3:i32),
        "invoke_jii"=>bind!(Extra30,i32,0;a0:i32,a1:i32,a2:i32),
        "invoke_iiiiiiifiii"=>bind!(Extra31,i32,0;a0:i32,a1:i32,a2:i32,a3:i32,a4:i32,a5:i32,a6:i32,a7:f64,a8:i32,a9:i32,a10:i32),
// END GENERATED BIND
        _=>return Ok(false),
    }Ok(true)
}
struct Info{adjusted:i32,kind:i32,destructor:i32,refs:u32,caught:bool,rethrown:bool}
#[derive(Debug)]pub(super) struct Thrown(pub(super) i32);
impl std::fmt::Display for Thrown{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{write!(f,"Guest C++ exception at {:#x}",self.0)}}
impl std::error::Error for Thrown{}
fn helper(caller:&mut Caller<'_,Host>,name:&str)->wasmtime::Result<Func>{
    if let Some(function)=caller.data().exceptions.helpers.get(name){return Ok(*function);}
    let function=caller.get_export(name).and_then(|value|value.into_func()).ok_or_else(||host_error(format!("Guest exception helper missing: {name}")))?;
    caller.data_mut().exceptions.helpers.insert(name.into(),function);Ok(function)
}
fn call(caller:&mut Caller<'_,Host>,name:&str,args:&[i32])->wasmtime::Result<i32>{
    let function=helper(caller,name)?;
    let returns=function.ty(&*caller).results().len();let mut results=vec![Val::I32(0);returns];
    function.call(&mut *caller,&args.iter().copied().map(Val::I32).collect::<Vec<_>>(),&mut results)?;
    Ok(if returns==0{0}else{results[0].i32().ok_or_else(||host_error("Invalid exception helper return type"))?})
}
fn original(host:&Host,pointer:i32)->i32{
    if pointer==0 || host.exceptions.infos.contains_key(&pointer){return pointer;}
    host.exceptions.infos.iter().find_map(|(key,info)|(info.adjusted==pointer).then_some(*key)).unwrap_or(pointer)
}
fn save_stack(caller:&mut Caller<'_,Host>)->wasmtime::Result<Option<i32>> {
    // Present only in our hash-pinned AOT transform, which proves stackSave is
    // exactly global.get 7. Cache the identity, never the mutable stack value.
    if caller.data().exceptions.stack_global.is_none(){
        let global=caller.get_export("__raven_stack_pointer_v1").and_then(|e|e.into_global());
        if let Some(g)=global{
            let ty=g.ty(&*caller);
            if !matches!(ty.content(),wasmtime::ValType::I32) || ty.mutability()!=wasmtime::Mutability::Var{
                return Err(host_error("Invalid verified stack global type"));
            }
        }
        caller.data_mut().exceptions.stack_global=Some(global);
    }
    if let Some(global)=caller.data().exceptions.stack_global.unwrap(){
        return Ok(Some(global.get(&mut *caller).i32().ok_or_else(||host_error("Invalid stack global"))?));
    }
    if caller.data().exceptions.stack_save.is_none(){
        let save=caller.get_export("stackSave").and_then(|value|value.into_func()).map(|function|function.typed::<(),i32>(&*caller)).transpose()?;
        caller.data_mut().exceptions.stack_save=Some(save);
    }
    let save=caller.data().exceptions.stack_save.as_ref().unwrap().clone();
    Ok(if let Some(save)=save{Some(save.call(&mut *caller,())?)}else{None})
}
fn restore_throw(caller:&mut Caller<'_,Host>,stack:Option<i32>)->wasmtime::Result<()> {
    let stack=stack.ok_or_else(||host_error("Guest exception requires stackSave/stackRestore"))?;
    call(caller,"stackRestore",&[stack])?;call(caller,"setThrew",&[1,0])?;Ok(())
}
pub(super) fn invoke(caller:&mut Caller<'_,Host>,trampoline:&str,slot:usize,args:&[Val],results:&mut[Val])->wasmtime::Result<()> {
    if caller.data().exceptions.invokes[slot].is_none(){let cached=CachedInvoke::new(caller,trampoline)?;caller.data_mut().exceptions.invokes[slot]=Some(Rc::new(cached));}
    let cached=caller.data().exceptions.invokes[slot].as_ref().unwrap().clone();
    // Small fixtures that cannot throw need no stack exports. A production
    // thrown exception requires the full pinned stackSave/stackRestore ABI.
    // Export identities are immutable for this single guest instance. Cache
    // handles, never stack values or table entries. The current stack value is read on every
    // invoke, preserving the exact exception restore boundary.
    let stack=save_stack(caller)?;
    match cached.call(&mut *caller,args,results){
        Ok(())=>Ok(()),
        Err(error) if error.downcast_ref::<Thrown>().is_some()=>{
            restore_throw(caller,stack)?;
            for (result,kind) in results.iter_mut().zip(cached.function.ty(&*caller).results()){
                *result=match kind{wasmtime::ValType::I32=>Val::I32(0),wasmtime::ValType::F64=>Val::F64(f64::NAN.to_bits()),_=>return Err(host_error("Unsupported legacy exception return ABI"))};
            }Ok(())
        }
        Err(error)=>Err(error),
    }
}
fn throw_cpp(caller:&mut Caller<'_,Host>,pointer:i32)->wasmtime::Result<bool>{
    #[cfg(feature="guest-eh-prototype")]{
        use wasmtime::{AsContextMut,ExnType,ExnRefPre,ExnRef,RootScope};
        if let Some(tag)=caller.get_export("__raven_cxx_tag_v1").and_then(|v|v.into_tag()){
            // Only the reviewed transformed module exports this private ABI tag.
            // Pending-exception rooting survives the scope and nested host calls.
            let ty=ExnType::new(caller.engine(),std::iter::empty::<wasmtime::ValType>())?;
            let mut scope=RootScope::new(&mut *caller);let allocator=ExnRefPre::new(&mut scope,ty);
            let exception=ExnRef::new(&mut scope,&allocator,&tag,&[])?;
            return scope.as_context_mut().throw(exception);
        }
    }
    Err(Thrown(pointer).into())
}
pub(super) fn dispatch(caller:&mut Caller<'_,Host>,name:&str,args:&[Val],results:&mut[Val])->wasmtime::Result<bool>{
    match name{
        "env.___cxa_allocate_exception"=>results[0]=Val::I32(call(caller,"_malloc",&[value_i32(args,0)?])?),
        "env.___cxa_free_exception"=>{call(caller,"_free",&[value_i32(args,0)?])?;},
        "env.___cxa_throw"=>{
            let pointer=value_i32(args,0)?;let kind=value_i32(args,1)?;let destructor=value_i32(args,2)?;
            let state=&mut caller.data_mut().exceptions;
            if pointer==0 || state.infos.len()>=1024 || state.infos.contains_key(&pointer){return Err(host_error("Invalid or excessive live guest exceptions"));}
            state.infos.insert(pointer,Info{adjusted:pointer,kind,destructor,refs:0,caught:false,rethrown:false});state.last=pointer;state.uncaught+=1;return throw_cpp(caller,pointer);
        }
        "env.___resumeException"=>{let pointer=value_i32(args,0)?;if caller.data().exceptions.last==0{caller.data_mut().exceptions.last=pointer;}return throw_cpp(caller,pointer);},
        "env.___cxa_begin_catch"=>{
            let pointer=value_i32(args,0)?;let key=original(caller.data(),pointer);let state=&mut caller.data_mut().exceptions;
            if state.caught.len()>=4096{return Err(host_error("Guest catch nesting exceeds bound"));}
            let info=state.infos.get_mut(&key).ok_or_else(||host_error("Catch of an unknown guest exception"))?;
            if !info.caught{info.caught=true;state.uncaught=state.uncaught.checked_sub(1).ok_or_else(||host_error("Exception count underflow"))?;}
            info.rethrown=false;info.refs+=1;state.caught.push(pointer);results[0]=Val::I32(pointer);
        }
        "env.___cxa_end_catch"=>{
            call(caller,"setThrew",&[0,0])?;
            if let Some(pointer)=caller.data_mut().exceptions.caught.pop(){
                let key=original(caller.data(),pointer);
                let info=caller.data_mut().exceptions.infos.get_mut(&key).ok_or_else(||host_error("End catch of an unknown guest exception"))?;
                info.refs=info.refs.checked_sub(1).ok_or_else(||host_error("Guest exception reference underflow"))?;
                if info.refs==0 && !info.rethrown{
                    let destructor=info.destructor;if destructor!=0{call(caller,"dynCall_vi",&[destructor,key])?;}
                    caller.data_mut().exceptions.infos.remove(&key);call(caller,"_free",&[key])?;
                }
                caller.data_mut().exceptions.last=0;
            }
        }
        "env.___cxa_rethrow"=>{
            let pointer=caller.data_mut().exceptions.caught.pop().ok_or_else(||host_error("Rethrow without active exception"))?;let key=original(caller.data(),pointer);
            let state=&mut caller.data_mut().exceptions;let info=state.infos.get_mut(&key).ok_or_else(||host_error("Rethrow of unknown exception"))?;
            if !info.rethrown{state.caught.push(key);info.rethrown=true;}state.last=key;return throw_cpp(caller,key);
        }
        "env.___cxa_find_matching_catch_2"|"env.___cxa_find_matching_catch_3"|"env.___cxa_find_matching_catch_4"=>{
            let pointer=caller.data().exceptions.last;
            if pointer==0{call(caller,"setTempRet0",&[0])?;results[0]=Val::I32(0);return Ok(true);}
            let kind=caller.data().exceptions.infos.get(&pointer).ok_or_else(||host_error("Missing thrown exception metadata"))?.kind;
            if kind==0{call(caller,"setTempRet0",&[0])?;results[0]=Val::I32(pointer);return Ok(true);}
            call(caller,"___cxa_is_pointer_type",&[kind])?; // Retained pinned RTTI side effects.
            if caller.data().exceptions.scratch==0{let scratch=call(caller,"_malloc",&[4])?;if scratch==0{return Err(host_error("Exception scratch allocation failed"));}caller.data_mut().exceptions.scratch=scratch;}
            let scratch=caller.data().exceptions.scratch;write_i32(caller,scratch as u32 as usize,pointer)?;
            let mut matched=kind;let mut adjusted=pointer;
            for argument in args{
                let catch_type=argument.i32().ok_or_else(||host_error("Expected exception RTTI address"))?;
                if catch_type!=0 && call(caller,"___cxa_can_catch",&[catch_type,kind,scratch])?!=0{
                    adjusted=read_u32(caller,scratch as u32 as usize)? as i32;matched=catch_type;
                    caller.data_mut().exceptions.infos.get_mut(&pointer).unwrap().adjusted=adjusted;break;
                }
            }
            if matched==kind{adjusted=read_u32(caller,scratch as u32 as usize)? as i32;}
            call(caller,"setTempRet0",&[matched])?;results[0]=Val::I32(adjusted);
        }
        "env.___cxa_pure_virtual"=>return Err(host_error("Guest called a pure virtual function")),
        _=>return Ok(false),
    }Ok(true)
}
