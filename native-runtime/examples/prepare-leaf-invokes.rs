//! Hash-pinned offline transform: keep proven import-free invokes inside WASM.
//! Unknown/indirect/import-calling targets retain the original host exception boundary.
#![allow(dead_code)]
use anyhow::{Result,ensure,bail};
use sha2::{Sha256,Digest};
use std::collections::{BTreeMap,VecDeque};
use wasmparser::{Parser,Payload,TypeRef,Operator,ExternalKind,ElementKind,ElementItems};
use wasm_encoder::{Encode,Instruction as I,Function,BlockType,MemArg};
#[path="support/guest_intrinsics.rs"]mod intrinsics;

fn leb(mut n:u32)->Vec<u8>{let mut b=Vec::new();loop{let v=(n&127) as u8;n>>=7;b.push(v|if n!=0{128}else{0});if n==0{return b;}}}
fn uint(b:&[u8],p:&mut usize)->Result<u32>{let mut n=0;for shift in (0..35).step_by(7){let v=*b.get(*p).ok_or_else(||anyhow::anyhow!("Truncated LEB"))?;*p+=1;n|=((v&127) as u32)<<shift;if v<128{return Ok(n);}}bail!("Invalid LEB")}
fn section(out:&mut Vec<u8>,id:u8,payload:&[u8]){out.push(id);out.extend(leb(payload.len() as u32));out.extend(payload);}
#[derive(Clone)]struct Wrapper{import:u32,ty:u32,trampoline:u32,mask:i32,base:i32,params:u32}

// Reviewed against this host's services.rs, lib.rs and graphics implementation.
// These paths never reenter guest code or return exceptions::Thrown. Ordinary
// bounds/driver/unsupported errors still propagate; this is not an error bypass.
fn nonthrowing_host_import(space:&str,name:&str)->bool{
 if space=="asm2wasm"&&name=="f64-rem"||space=="global.Math"&&name=="pow"{return true;}
 if space!="env"{return false;}
 if name.starts_with("_gl")&&!matches!(name,"_glGetString"|"_glGetStringi"){return true;}
 matches!(name,"_llvm_trunc_f32"|"_llvm_sin_f32"|"_llvm_cos_f32"|"_llvm_log2_f32"|
  "_llvm_log10_f32"|"_llvm_log10_f64"|"_llvm_exp2_f32"|"_llvm_eh_typeid_for"|
  "_llvm_trap"|"_emscripten_memcpy_big"|"_emscripten_get_now"|"___lock"|"___unlock"|
  "getTotalMemory"|"enlargeMemory"|"abort"|"_abort"|"abortOnCannotGrowMemory"|
  "___atomic_compare_exchange_8"|"_getpagesize"|"_clock"|"_gettimeofday"|"_time"|"_difftime")
}

pub(crate) fn transform(bytes:&[u8])->Result<(Vec<u8>,serde_json::Value)>{
 transform_selected(bytes,None)
}
fn transform_selected(bytes:&[u8],selected:Option<&std::collections::BTreeSet<String>>)->Result<(Vec<u8>,serde_json::Value)>{
 transform_mode(bytes,selected,false)
}
pub(crate) fn transform_mode(bytes:&[u8],selected:Option<&std::collections::BTreeSet<String>>,caught_invokes:bool)->Result<(Vec<u8>,serde_json::Value)>{
 let mut types=Vec::new();let mut funcs=Vec::new();let mut imports=Vec::new();let mut exports=BTreeMap::new();let mut bodies=Vec::new();let mut table=Vec::new();let mut memories=0;let mut globals=Vec::new();let mut has_data=false;
 for p in Parser::new(0).parse_all(bytes){match p?{
  Payload::TypeSection(s)=>for t in s.into_iter_err_on_gc_types(){types.push(t?);},
  Payload::ImportSection(s)=>for i in s.into_imports(){let i=i?;match i.ty{TypeRef::Func(t)=>{imports.push((i.module.to_string(),i.name.to_string()));funcs.push(t);},TypeRef::Table(t)=>{ensure!(table.is_empty()&&t.initial<=400_000,"Expected one bounded table");table.resize(t.initial as usize,None);},TypeRef::Memory(t)=>{ensure!(!t.shared&&!t.memory64,"Expected nonshared wasm32 memory");memories+=1;},TypeRef::Global(t)=>{globals.push((i.module,i.name,t.mutable));},_=>{}}},
  Payload::FunctionSection(s)=>for t in s{funcs.push(t?);},
  Payload::ExportSection(s)=>for e in s{let e=e?;if e.kind==ExternalKind::Func{exports.insert(e.name.to_string(),e.index);}},
  Payload::CodeSectionEntry(b)=>bodies.push(b),
  Payload::MemorySection(_)|Payload::TableSection(_)=>bail!("Unexpected defined memory/table"),
  Payload::ElementSection(s)=>for e in s{let e=e?;let ElementKind::Active{table_index,offset_expr}=e.kind else{bail!("Nonactive element segment")};ensure!(table_index.unwrap_or(0)==0,"Unexpected table");let mut r=offset_expr.get_operators_reader();let value=match r.read()?{Operator::I32Const{value}=>value,Operator::GlobalGet{global_index} if globals.get(global_index as usize)==Some(&("env","tableBase",false))=>0,other=>bail!("Unsupported table offset {other:?}")};ensure!(value>=0&&matches!(r.read()?,Operator::End)&&r.eof(),"Invalid element offset");let ElementItems::Functions(items)=e.items else{bail!("Expression elements")};for (j,f) in items.into_iter().enumerate(){let slot=table.get_mut(value as usize+j).ok_or_else(||anyhow::anyhow!("Element bounds"))?;*slot=Some(f?);}},
  Payload::DataCountSection{..}=>bail!("Unexpected bulk-memory data count"),
  Payload::DataSection(_)=>has_data=true,
  _=>{}
 }}
 ensure!(memories==1&&!table.is_empty()&&has_data&&funcs.len()==imports.len()+bodies.len(),"Unexpected module layout (requires existing data section)");
 // Conservative proof: ALL imports and ALL indirect calls may throw. Propagate
 // unsafety through direct calls (including recursive SCCs) in a linear worklist.
 let mut bad=vec![false;funcs.len()];let mut reverse=vec![Vec::new();funcs.len()];let mut queue=VecDeque::new();
 for n in 0..imports.len(){bad[n]=true;queue.push_back(n);}
 for (n,body) in bodies.iter().enumerate(){let index=imports.len()+n;let mut ops=body.get_operators_reader()?;while !ops.eof(){match ops.read()?{
  Operator::Call{function_index}=>reverse[function_index as usize].push(index),
  Operator::CallIndirect{..}=>{if !bad[index]{bad[index]=true;queue.push_back(index);}},
  Operator::TableSet{..}|Operator::TableInit{..}|Operator::TableCopy{..}|Operator::TableGrow{..}|Operator::TableFill{..}=>bail!("Mutable table invalidates leaf proof"),
  Operator::CallRef{..}|Operator::ReturnCall{..}|Operator::ReturnCallIndirect{..}|Operator::ReturnCallRef{..}|Operator::Throw{..}|Operator::ThrowRef|Operator::Rethrow{..}=>bail!("Unsupported call/throw instruction"),
  _=>{}
 }}}
 let intrinsic_bad=bad.clone();
 while let Some(index)=queue.pop_front(){for &parent in &reverse[index]{if !bad[parent]{bad[parent]=true;queue.push_back(parent);}}}
 let mut caught_imports=0;let mut reviewed_imports=Vec::new();
 if caught_invokes{
  // A nested host invoke absorbs Thrown, restores stack and calls setThrew.
  // Those helpers must independently pass the ORIGINAL no-import/no-indirect
  // proof, otherwise treating that host boundary as nonthrowing is unsound.
  for helper in ["stackSave","stackRestore","setThrew"]{
   let index=*exports.get(helper).ok_or_else(||anyhow::anyhow!("Missing exception helper {helper}"))?;
   ensure!(!bad[index as usize],"Exception helper is not independently pure: {helper}");
  }
  let atomic_setter=imports.iter().any(|(s,n)|s=="env"&&n=="___atomic_fetch_add_8");
  if atomic_setter{
   let index=*exports.get("setTempRet0").ok_or_else(||anyhow::anyhow!("Missing atomic return helper"))?;
   ensure!(!bad[index as usize],"Atomic return helper is not independently pure");
  }
  bad=intrinsic_bad;
  for (n,(space,name)) in imports.iter().enumerate(){
   if space=="env"&&name.starts_with("invoke_"){bad[n]=false;caught_imports+=1;}
   else if nonthrowing_host_import(space,name)||(atomic_setter&&space=="env"&&name=="___atomic_fetch_add_8"){bad[n]=false;reviewed_imports.push(format!("{space}.{name}"));}
  }
  queue.extend(bad.iter().enumerate().filter_map(|(n,b)|b.then_some(n)));
  while let Some(index)=queue.pop_front(){for &parent in &reverse[index]{if !bad[parent]{bad[parent]=true;queue.push_back(parent);}}}
 }
 let mut flags=vec![0u8;table.len().div_ceil(8)];for (n,f) in table.iter().enumerate(){if f.is_some_and(|i|!bad[i as usize]){flags[n/8]|=1<<(n%8);}}
 let mut wrappers=Vec::new();
 for (index,(space,name)) in imports.iter().enumerate(){if space!="env"||!name.starts_with("invoke_"){continue;}
  if selected.is_some_and(|names|!names.contains(&name[7..])){continue;}
  let Some(&target)=exports.get(&format!("dynCall_{}",&name[7..])) else{continue;};
  if target<imports.len() as u32||funcs[index]!=funcs[target as usize]{continue;}
  let ty=&types[funcs[index] as usize];let count=ty.params().len() as u32;
  let body=&bodies[target as usize-imports.len()];if body.get_locals_reader()?.get_count()!=0{continue;}
  let mut r=body.get_operators_reader()?;let mut valid=true;
  for local in 1..count{if !matches!(r.read()?,Operator::LocalGet{local_index} if local_index==local){valid=false;break;}}if !valid{continue;}
  if !matches!(r.read()?,Operator::LocalGet{local_index:0}){continue;}
  let Operator::I32Const{value:mask}=r.read()? else{continue;};if !matches!(r.read()?,Operator::I32And){continue;}
  let Operator::I32Const{value:base}=r.read()? else{continue;};if !matches!(r.read()?,Operator::I32Add){continue;}
  if !matches!(r.read()?,Operator::CallIndirect{table_index:0,..})||!matches!(r.read()?,Operator::End)||!r.eof(){continue;}
  ensure!(mask>=0&&base>=0&&(mask as usize+base as usize)<table.len(),"Trampoline target outside fixed table");
  if !(base as usize..=base as usize+mask as usize).any(|n|flags[n/8]&(1<<(n%8))!=0){continue;}
  wrappers.push(Wrapper{import:index as u32,ty:funcs[index],trampoline:target,mask,base,params:count});
 }
 ensure!(!wrappers.is_empty(),"No verified simple trampolines");
 let replacements:BTreeMap<_,_>=wrappers.iter().enumerate().map(|(n,w)|(w.import,funcs.len() as u32+n as u32)).collect();
 let mut code=leb((bodies.len()+wrappers.len()) as u32);let mut changed=0u64;
 for body in &bodies{let range=body.range();let mut out=Vec::new();let mut cursor=range.start as usize;let mut r=body.get_operators_reader()?;
  while !r.eof(){let start=r.original_position() as usize;let op=r.read()?;if let Operator::Call{function_index}=op{if let Some(&replacement)=replacements.get(&function_index){out.extend(&bytes[cursor..start]);out.push(0x10);out.extend(leb(replacement));cursor=r.original_position() as usize;changed+=1;}}}
  out.extend(&bytes[cursor..range.end as usize]);code.extend(leb(out.len() as u32));code.extend(out);
 }
 for w in &wrappers{
  let mut f=Function::new([(1,wasm_encoder::ValType::I32)]);
  let ops=[I::LocalGet(0),I::I32Const(w.mask),I::I32And,I::I32Const(w.base),I::I32Add,I::LocalSet(w.params),
   I::LocalGet(w.params),I::I32Const(3),I::I32ShrU,I::I32Load8U(MemArg{offset:0,align:0,memory_index:1}),
   I::LocalGet(w.params),I::I32Const(7),I::I32And,I::I32ShrU,I::I32Const(1),I::I32And,I::If(BlockType::Empty)];
  for op in ops{f.instruction(&op);}for n in 0..w.params{f.instruction(&I::LocalGet(n));}f.instruction(&I::Call(w.trampoline));f.instruction(&I::Return);f.instruction(&I::End);
  for n in 0..w.params{f.instruction(&I::LocalGet(n));}f.instruction(&I::Call(w.import));f.instruction(&I::End);f.encode(&mut code);
 }
 // Keep all original indices and bytes except call operands and extended sections.
 // The bitset lives in a NEW unexported memory, never in the game's heap/data.
 let mut out=bytes[..8].to_vec();let mut p=8;let mut inserted=false;
 while p<bytes.len(){let id=bytes[p];p+=1;let len=uint(bytes,&mut p)? as usize;let payload=&bytes[p..p+len];p+=len;
  if !inserted&&id!=0&&id>5{let mut m=wasm_encoder::MemorySection::new();m.memory(wasm_encoder::MemoryType{minimum:flags.len().div_ceil(65536) as u64,maximum:Some(flags.len().div_ceil(65536) as u64),memory64:false,shared:false,page_size_log2:None});out.push(5);m.encode(&mut out);inserted=true;}
  match id{
   3=>{let mut q=0;let count=uint(payload,&mut q)?;let mut b=leb(count+wrappers.len() as u32);b.extend(&payload[q..]);for w in &wrappers{b.extend(leb(w.ty));}section(&mut out,3,&b);},
   10=>section(&mut out,10,&code),
   11=>{let mut q=0;let count=uint(payload,&mut q)?;let mut b=leb(count+1);b.extend(&payload[q..]);b.extend([2,1,0x41,0,0x0b]);b.extend(leb(flags.len() as u32));b.extend(&flags);section(&mut out,11,&b);},
   _=>section(&mut out,id,payload)
  }
 }
 wasmparser::Validator::new().validate_all(&out)?;
 let report=serde_json::json!({"proof":if caught_invokes{"transitive no-uncaught-import/no-indirect-call; nested host invokes absorb Thrown; reviewed non-reentrant GL/math/memory imports; exception helpers independently import-free; immutable table; original dynCall and host fallback retained"}else{"transitive no-import/no-indirect-call; table immutable; original dynCall retained; other targets use host exception boundary"},"caught_invoke_imports":caught_imports,"reviewed_nonthrowing_imports":reviewed_imports,"functions":funcs.len(),"proven_leaf_functions":bad.iter().filter(|b|!**b).count(),"table_entries":table.len(),"safe_table_entries":table.iter().filter(|i|i.is_some_and(|n|!bad[n as usize])).count(),"private_bitset_bytes":flags.len(),"rewritten_call_sites":changed,"wrappers":wrappers.iter().map(|w|serde_json::json!({"import":imports[w.import as usize].1,"mask":w.mask,"base":w.base,"safe_slots":(0..=w.mask as usize).filter(|n|flags[(w.base as usize+n)/8]&(1<<((w.base as usize+n)%8))!=0).count()})).collect::<Vec<_>>()});
 Ok((out,report))
}
#[cfg(test)]mod tests{
 use super::*;
 fn fixture(mutation:&str)->Vec<u8>{wat::parse_str(format!(r#"(module
  (type $target (func (param i32) (result i32)))
  (import "env" "memory" (memory 1)) (import "env" "table" (table 4 4 funcref))
  (import "env" "invoke_ii" (func $invoke (param i32 i32) (result i32)))
  (import "env" "external" (func $external (param i32) (result i32)))
  (func $leaf (type $target) local.get 0 i32.const 7 i32.add)
  (func $parent (type $target) local.get 0 call $leaf)
  (func $unsafe (type $target) local.get 0 call $external)
  (func $transitive (type $target) local.get 0 call $unsafe)
  (elem (i32.const 0) $leaf $parent $unsafe $transitive)
  (func (export "dynCall_ii") (param i32 i32) (result i32)
   local.get 1 local.get 0 i32.const 3 i32.and i32.const 0 i32.add call_indirect (type $target))
  (func (export "run") (param i32 i32) (result i32) local.get 0 local.get 1 call $invoke)
  (func {mutation}) (data (i32.const 0) "original"))"#)).unwrap()}
 #[test]fn leaf_and_transitive_paths_preserve_mask_aliases_and_host_fallback(){
  let original=fixture("");let (bytes,report)=transform(&original).unwrap();assert_eq!(report["safe_table_entries"],2);
  let engine=wasmtime::Engine::default();let module=wasmtime::Module::new(&engine,bytes).unwrap();
  let mut store=wasmtime::Store::new(&engine,0u32);let mut linker=wasmtime::Linker::new(&engine);
  let memory=wasmtime::Memory::new(&mut store,wasmtime::MemoryType::new(1,None)).unwrap();let table=wasmtime::Table::new(&mut store,wasmtime::TableType::new(wasmtime::RefType::FUNCREF,4,Some(4)),wasmtime::Ref::Func(None)).unwrap();
  linker.define(&mut store,"env","memory",memory).unwrap();linker.define(&mut store,"env","table",table).unwrap();
  linker.func_wrap("env","external",|_:i32|->wasmtime::Result<i32>{Err(wasmtime::Error::msg("sentinel"))}).unwrap();
  linker.func_wrap("env","invoke_ii",|mut c:wasmtime::Caller<'_,u32>,index:i32,value:i32|->wasmtime::Result<i32>{*c.data_mut()+=1;let f=c.get_export("dynCall_ii").unwrap().into_func().unwrap().typed::<(i32,i32),i32>(&c)?;f.call(&mut c,(index,value))}).unwrap();
  let instance=linker.instantiate(&mut store,&module).unwrap();let run=instance.get_typed_func::<(i32,i32),i32>(&mut store,"run").unwrap();
  for index in [0,1,4,5,i32::MIN,i32::MIN+1]{assert_eq!(run.call(&mut store,(index,20)).unwrap(),27);}assert_eq!(*store.data(),0);
  for index in [2,3,-1,i32::MAX]{let error=run.call(&mut store,(index,20)).unwrap_err();assert!(format!("{error:#}").contains("sentinel"));}assert_eq!(*store.data(),4);
  assert_eq!(&memory.data(&store)[..8],b"original");assert_eq!(instance.exports(&mut store).count(),2);
 }
 #[test]fn table_mutation_rejects_entire_transform(){let source=fixture("i32.const 0 ref.null func table.set 0");assert!(transform(&source).unwrap_err().to_string().contains("Mutable table"));}
 #[test]fn only_verified_simple_trampolines_are_accepted(){let source=fixture("");let (bytes,report)=transform(&source).unwrap();assert!(bytes.len()>source.len());assert_eq!(report["wrappers"].as_array().unwrap().len(),1);assert_eq!(report["wrappers"][0]["safe_slots"],2);}
 #[test]fn selection_keeps_unselected_imports_untouched(){let source=fixture("");let names=std::collections::BTreeSet::from([String::from("ii")]);let (selected,_)=transform_selected(&source,Some(&names)).unwrap();assert_eq!(selected,transform(&source).unwrap().0);let names=std::collections::BTreeSet::from([String::from("v")]);assert!(transform_selected(&source,Some(&names)).unwrap_err().to_string().contains("No verified"));}
 fn nested_fixture(set_threw:&str)->Vec<u8>{wat::parse_str(format!(r#"(module
  (type $target (func (param i32) (result i32)))
  (import "env" "memory" (memory 1)) (import "env" "table" (table 4 4 funcref))
  (import "env" "invoke_ii" (func $invoke (param i32 i32) (result i32)))
  (import "env" "external" (func $external (param i32) (result i32)))
  (func $leaf (type $target) local.get 0 i32.const 7 i32.add)
  (func $nested (type $target) i32.const 2 local.get 0 call $invoke)
  (func $unsafe (type $target) local.get 0 call $external)
  (elem (i32.const 0) $leaf $nested $unsafe $unsafe)
  (func (export "dynCall_ii") (param i32 i32) (result i32)
   local.get 1 local.get 0 i32.const 3 i32.and i32.const 0 i32.add call_indirect (type $target))
  (func (export "run") (param i32 i32) (result i32) local.get 0 local.get 1 call $invoke)
  (func (export "stackSave") (result i32) i32.const 0)
  (func (export "stackRestore") (param i32))
  (func (export "setThrew") (param i32 i32) {set_threw})
  (data (i32.const 0) "original"))"#)).unwrap()}
 #[test]fn caught_nested_invoke_preserves_inner_boundary_and_unknown_import_fallback(){
  let source=nested_fixture("");let (_,old)=transform(&source).unwrap();assert_eq!(old["safe_table_entries"],1);
  let (bytes,report)=transform_mode(&source,None,true).unwrap();assert_eq!(report["safe_table_entries"],2);assert_eq!(report["caught_invoke_imports"],1);
  let engine=wasmtime::Engine::default();let module=wasmtime::Module::new(&engine,bytes).unwrap();
  let mut store=wasmtime::Store::new(&engine,0u32);let mut linker=wasmtime::Linker::new(&engine);
  let memory=wasmtime::Memory::new(&mut store,wasmtime::MemoryType::new(1,None)).unwrap();
  let table=wasmtime::Table::new(&mut store,wasmtime::TableType::new(wasmtime::RefType::FUNCREF,4,Some(4)),wasmtime::Ref::Func(None)).unwrap();
  linker.define(&mut store,"env","memory",memory).unwrap();linker.define(&mut store,"env","table",table).unwrap();
  linker.func_wrap("env","external",|_:i32|->wasmtime::Result<i32>{Err(wasmtime::Error::msg("caught sentinel"))}).unwrap();
  linker.func_wrap("env","invoke_ii",|mut c:wasmtime::Caller<'_,u32>,index:i32,value:i32|->wasmtime::Result<i32>{
   *c.data_mut()+=1;let f=c.get_export("dynCall_ii").unwrap().into_func().unwrap().typed::<(i32,i32),i32>(&c)?;
   match f.call(&mut c,(index,value)){Ok(v)=>Ok(v),Err(e) if format!("{e:#}").contains("caught sentinel")=>Ok(-7),Err(e)=>Err(e)}
  }).unwrap();
  let instance=linker.instantiate(&mut store,&module).unwrap();let run=instance.get_typed_func::<(i32,i32),i32>(&mut store,"run").unwrap();
  assert_eq!(run.call(&mut store,(0,20)).unwrap(),27);assert_eq!(*store.data(),0);
  for index in [1,5,i32::MIN+1]{let before=*store.data();assert_eq!(run.call(&mut store,(index,20)).unwrap(),-7);assert_eq!(*store.data(),before+1);}
  for index in [2,3,-1]{let before=*store.data();assert_eq!(run.call(&mut store,(index,20)).unwrap(),-7);assert_eq!(*store.data(),before+1);}
 }
 #[test]fn caught_mode_requires_independently_pure_exception_helpers(){
  assert!(transform_mode(&fixture(""),None,true).unwrap_err().to_string().contains("Missing exception helper"));
  assert!(transform_mode(&nested_fixture("i32.const 0 call $external drop"),None,true).unwrap_err().to_string().contains("not independently pure"));
 }
 #[test]fn host_whitelist_excludes_guest_reentry_and_unknown_imports(){
  assert!(nonthrowing_host_import("env","_glUniform4fv"));assert!(nonthrowing_host_import("env","_llvm_sin_f32"));
  for name in ["_glGetString","_glGetStringi","___cxa_throw","___resumeException","_pthread_once","_malloc","external"]{assert!(!nonthrowing_host_import("env",name));}
  assert!(!nonthrowing_host_import("other","_glUniform4fv"));
  let source=nested_fixture("");
  // Rename the unknown import to a reviewed host GL service. Its wrapper can
  // now be direct, while the exact same unknown-import fixture stays unsafe.
  let original=wat::parse_str(r#"(module
   (import "env" "memory" (memory 1)) (import "env" "table" (table 1 1 funcref))
   (import "env" "invoke_vi" (func $invoke (param i32 i32)))
   (import "env" "_glActiveTexture" (func $active (param i32)))
   (type $target (func (param i32)))
   (func $leaf (type $target) local.get 0 call $active) (elem (i32.const 0) $leaf)
   (func (export "dynCall_vi") (param i32 i32) local.get 1 local.get 0 i32.const 0 i32.and i32.const 0 i32.add call_indirect (type $target))
   (func (export "run") i32.const 0 i32.const 33984 call $invoke)
   (func (export "stackSave") (result i32) i32.const 0)
   (func (export "stackRestore") (param i32)) (func (export "setThrew") (param i32 i32))
   (data (i32.const 0) "original"))"#).unwrap();
  assert!(transform(&original).is_err());let (_,report)=transform_mode(&original,None,true).unwrap();assert_eq!(report["safe_table_entries"],1);
  assert_eq!(transform_mode(&source,None,true).unwrap().1["safe_table_entries"],2);
 }
}
fn main()->Result<()>{
 let args:Vec<_>=std::env::args().skip(1).collect();ensure!(args.len()>=3,"Usage: prepare-leaf-invokes STACK_EXPORT.wasm OUTPUT.wasm REPORT.json [--signatures v,ii,...] [--caught-invokes]");
 let mut selected=None;let mut caught_invokes=false;let mut lower_intrinsics=false;let mut arg=3;
 while arg<args.len(){match args[arg].as_str(){
  "--caught-invokes"=>{ensure!(!caught_invokes,"Duplicate caught-invokes flag");caught_invokes=true;arg+=1;},
  "--lower-intrinsics"=>{ensure!(!lower_intrinsics,"Duplicate lower-intrinsics flag");lower_intrinsics=true;arg+=1;},
  "--signatures"=>{ensure!(selected.is_none()&&arg+1<args.len(),"Invalid signatures option");selected=Some(args[arg+1].split(',').map(String::from).collect::<std::collections::BTreeSet<_>>());arg+=2;},
  _=>bail!("Unknown option: {}",args[arg])
 }}
 if let Some(names)=&selected{ensure!(!names.is_empty()&&names.iter().all(|s|!s.is_empty()&&s.bytes().all(|b|b.is_ascii_lowercase())),"Invalid signature selection");}
 let source=std::fs::read(&args[0])?;
 ensure!(format!("{:x}",Sha256::digest(&source))=="067c84adee62060e934b5d18430a89085c58a4b6a42fcc05a75e332d9f4c261c","Requires exact reviewed stack-export source");
 let lowered=if lower_intrinsics{Some(intrinsics::lower(&source)?)}else{None};
 let input=lowered.as_ref().map(|(bytes,_)|bytes.as_slice()).unwrap_or(&source);
 let (output,mut report)=transform_mode(input,selected.as_ref(),caught_invokes)?;
 if let Some((bytes,details))=lowered{report["intrinsic_lowering"]=details;report["post_intrinsic_sha256"]=format!("{:x}",Sha256::digest(&bytes)).into();}
 report["selected_signatures"]=serde_json::to_value(&selected)?;report["transformed_sha256"]=format!("{:x}",Sha256::digest(&output)).into();report["source_sha256"]=format!("{:x}",Sha256::digest(&source)).into();
 std::fs::write(&args[1],output)?;std::fs::write(&args[2],serde_json::to_vec_pretty(&report)?)?;println!("{report}");Ok(())
}
