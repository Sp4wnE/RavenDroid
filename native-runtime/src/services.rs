//! Typed hot scalar services. Same ABI and math semantics as the generic route.
use super::{Host,Caller};
use std::time::Instant;
pub(super) fn bind(linker:&mut wasmtime::Linker<Host>,space:&str,name:&str,counter:usize)->wasmtime::Result<bool>{
 if (space,name)==("env","_emscripten_get_now"){
  linker.func_wrap(space,name,move |mut caller:Caller<'_,Host>|->f64{
   caller.data_mut().calls[counter].1+=1;
   let clock=caller.data().profile_imports.then(Instant::now);
   let value=caller.data().started.unwrap().elapsed().as_micros() as f64/1000.0;
   if let Some(clock)=clock{let elapsed=clock.elapsed().as_nanos();
    let entry=caller.data_mut().import_timings.entry("env._emscripten_get_now".into()).or_default();entry.0+=1;entry.1+=elapsed;}
   value
  })?;return Ok(true);
 }
 macro_rules! math{($ret:ty, $body:expr; $($arg:ident:$ty:ty),*)=>{{
  let full=format!("{space}.{name}");
  linker.func_wrap(space,name,move |mut c:Caller<'_,Host>,$($arg:$ty),*|->$ret{
   c.data_mut().calls[counter].1+=1;
   let clock=c.data().profile_imports.then(Instant::now);
   let result=($body)($($arg),*);
   if let Some(clock)=clock{let elapsed=clock.elapsed().as_nanos();let timings=&mut c.data_mut().import_timings;
    if let Some((count,time))=timings.get_mut(&full){*count+=1;*time+=elapsed;}else{timings.insert(full.clone(),(1,elapsed));}}
   result
  })?;
 }};}
 match (space,name){
  ("env","_llvm_trunc_f32")=>math!(f64,|a:f64|a.trunc();a:f64),
  ("env","_llvm_sin_f32")=>math!(f64,|a:f64|a.sin();a:f64),
  ("env","_llvm_cos_f32")=>math!(f64,|a:f64|a.cos();a:f64),
  ("env","_llvm_log2_f32")=>math!(f64,|a:f64|a.ln()/std::f64::consts::LN_2;a:f64),
  ("env","_llvm_log10_f32"|"_llvm_log10_f64")=>math!(f64,|a:f64|a.ln()/std::f64::consts::LN_10;a:f64),
  ("env","_llvm_exp2_f32")=>math!(f64,|a:f64|2f64.powf(a);a:f64),
  ("asm2wasm","f64-rem")=>math!(f64,|a:f64,b:f64|a%b;a:f64,b:f64),
  ("global.Math","pow")=>math!(f64,|a:f64,b:f64|a.powf(b);a:f64,b:f64),
  _=>return bind_graphics(linker,space,name,counter)
 }Ok(true)
}

// Use Wasmtime's typed entry trampoline for the measured hot GL imports.
// Stack Val adapters reuse the existing checked GL implementation; no buffer,
// context, invalidation, tracing or driver-error semantics are bypassed.
fn bind_graphics(linker:&mut wasmtime::Linker<Host>,space:&str,name:&str,counter:usize)->wasmtime::Result<bool>{
 if space!="env" {return Ok(false);}
 // Hot GL imports avoid Val packing and string routing. Tracing retains the
 // generic route so its diagnostics and validation remain identical.
 macro_rules! direct{($function:path; $($arg:ident),+)=>{{
  let full=format!("{space}.{name}");
  linker.func_wrap(space,name,move |mut caller:Caller<'_,Host>,$($arg:i32),+|->wasmtime::Result<()>{
   caller.data_mut().calls[counter].1+=1;
   let clock=caller.data().profile_imports.then(Instant::now);
   let result=if caller.data().graphics.is_some()&&!caller.data().trace_gl_calls{$function(&mut caller,$($arg),+)}
    else{super::dispatch(&mut caller,&full,None,&[$(wasmtime::Val::I32($arg)),+],&mut [])};
   if let Some(clock)=clock{let elapsed=clock.elapsed().as_nanos();let timings=&mut caller.data_mut().import_timings;
    if let Some((count,time))=timings.get_mut(&full){*count+=1;*time+=elapsed;}else{timings.insert(full.clone(),(1,elapsed));}}
   result
  })?;return Ok(true);
 }};}
 match name{
  "_glUniform1fv"=>direct!(super::graphics::uniform1fv_typed;a,b,c),
  "_glUniform2fv"=>direct!(super::graphics::uniform2fv_typed;a,b,c),
  "_glUniform3fv"=>direct!(super::graphics::uniform3fv_typed;a,b,c),
  "_glBufferSubData"=>direct!(super::graphics::buffer_sub_data_typed;a,b,c,d),
  "_glBindBuffer"=>direct!(super::graphics::bind_buffer_typed;a,b),
  "_glBindTexture"=>direct!(super::graphics::bind_texture_typed;a,b),
  "_glUseProgram"=>direct!(super::graphics::use_program_typed;a),
  "_glEnableVertexAttribArray"=>direct!(super::graphics::enable_vertex_attrib_typed;a),
  "_glDisableVertexAttribArray"=>direct!(super::graphics::disable_vertex_attrib_typed;a),
  "_glUniform4fv"=>direct!(super::graphics::uniform4fv_typed;a,b,c),
  "_glActiveTexture"=>direct!(super::graphics::active_texture_typed;a),
  "_glVertexAttribPointer"=>direct!(super::graphics::vertex_attrib_pointer_typed;a,b,c,d,e,f),
  "_glDrawElements"=>direct!(super::graphics::draw_elements_typed;a,b,c,d),
  _=>{}
 }
 macro_rules! gl{($($arg:ident),+)=>{{
  let full=format!("{space}.{name}");
  linker.func_wrap(space,name,move |mut c:Caller<'_,Host>,$($arg:i32),+|->wasmtime::Result<()>{
   c.data_mut().calls[counter].1+=1;
   let clock=c.data().profile_imports.then(Instant::now);
   let args=[$(wasmtime::Val::I32($arg)),+];
   let result=match super::graphics::dispatch(&mut c,&full,&args,&mut []){
    Ok(true)=>Ok(()),Ok(false)=>super::dispatch(&mut c,&full,None,&args,&mut []),Err(error)=>Err(error)};
   if let Some(clock)=clock{let elapsed=clock.elapsed().as_nanos();let timings=&mut c.data_mut().import_timings;
    if let Some((count,time))=timings.get_mut(&full){*count+=1;*time+=elapsed;}else{timings.insert(full.clone(),(1,elapsed));}}
   result
  })?;
 }};}
 match name{
  "_glActiveTexture"|"_glUseProgram"|"_glBindVertexArray"|"_glEnableVertexAttribArray"|"_glDisableVertexAttribArray"=>gl!(a),
  "_glBindBuffer"|"_glBindTexture"=>gl!(a,b),
  "_glUniform1fv"|"_glUniform2fv"|"_glUniform3fv"|"_glUniform4fv"=>gl!(a,b,c),
  "_glDrawElements"|"_glUniformMatrix4fv"=>gl!(a,b,c,d),
  "_glVertexAttribPointer"=>gl!(a,b,c,d,e,f),
  _=>return Ok(false)
 }Ok(true)
}
#[cfg(all(test,feature="compiler"))]mod tests{
 #[test]fn typed_trunc_preserves_signed_zero_nonfinite_values_and_optional_timing(){
  let(mut store,instance)=crate::tests::fixture(r#"(module (import "env" "_llvm_trunc_f32" (func $trunc (param f64) (result f64))) (func (export "run") (param f64) (result f64) local.get 0 call $trunc))"#);
  store.data_mut().profile_imports=true;let run=instance.get_typed_func::<f64,f64>(&mut store,"run").unwrap();
  for value in [-0.,-0.75,f64::INFINITY,f64::NAN]{let actual=run.call(&mut store,value).unwrap();if value.is_nan(){assert!(actual.is_nan());}else{assert_eq!(actual.to_bits(),value.trunc().to_bits());}}
  assert_eq!(store.data().import_timings["env._llvm_trunc_f32"].0,4);store.data_mut().profile_imports=false;assert_eq!(run.call(&mut store,19.75).unwrap(),19.);
  assert_eq!(store.data().import_timings["env._llvm_trunc_f32"].0,4);assert_eq!(store.data().calls[0].1,5);
 }
}
