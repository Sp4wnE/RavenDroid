//! Lower the pinned single-owner legacy i64 add ABI without a host roundtrip.
use anyhow::{Result,ensure,bail};
use sha2::{Digest,Sha256};
use wasm_encoder::{Encode,Function,Instruction as I,MemArg,ValType as V};
use wasmparser::{Parser,Payload,TypeRef,Operator,ExternalKind,ElementItems,ValType};
const SOURCE_SHA:&str="3b3fa150910e126fb0439b6c3f4a345aff4828c627eba10bb9c8ee521ce77e40";
fn hash(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
fn leb(mut n:u32)->Vec<u8>{let mut b=Vec::new();loop{let v=(n&127) as u8;n>>=7;b.push(v|if n>0{128}else{0});if n==0{return b;}}}
fn uint(bytes:&[u8],p:&mut usize)->Result<u32>{let mut n=0;for s in (0..35).step_by(7){let b=*bytes.get(*p).ok_or_else(||anyhow::anyhow!("Truncated LEB"))?;*p+=1;ensure!(s<28||b<16,"Invalid LEB");n|=((b&127) as u32)<<s;if b<128{return Ok(n);}}bail!("Invalid LEB")}
fn section(out:&mut Vec<u8>,id:u8,b:&[u8]){out.push(id);out.extend(leb(b.len() as u32));out.extend_from_slice(b);}
pub(crate) fn transform(bytes:&[u8])->Result<(Vec<u8>,serde_json::Value)>{
 ensure!(bytes.len()<=256*1024*1024,"Source exceeds bound");wasmparser::Validator::new().validate_all(bytes)?;
 let(mut types,mut functions,mut bodies)=(Vec::new(),Vec::new(),Vec::new());
 let(mut add,mut setter,mut memories)=(None,None,0);
 for payload in Parser::new(0).parse_all(bytes){match payload?{
  Payload::TypeSection(s)=>for t in s.into_iter_err_on_gc_types(){types.push(t?);},
  Payload::ImportSection(s)=>for imp in s.into_imports(){let imp=imp?;match imp.ty{
   TypeRef::Func(ty)=>{let index=functions.len() as u32;functions.push(ty);if imp.module=="env"&&imp.name=="___atomic_fetch_add_8"{ensure!(add.is_none(),"Duplicate add import");add=Some((index,ty));}},
   TypeRef::Memory(m)=>{ensure!(!m.shared&&!m.memory64&&m.page_size_log2.is_none(),"Requires single-owner wasm32 memory");memories+=1;},_=>{}
  }},
  Payload::FunctionSection(s)=>for ty in s{functions.push(ty?);},
  Payload::MemorySection(s)=>for m in s{let m=m?;ensure!(!m.shared&&!m.memory64&&m.page_size_log2.is_none(),"Requires single-owner wasm32 memory");memories+=1;},
  Payload::ExportSection(s)=>for e in s{let e=e?;if e.name=="setTempRet0"{ensure!(e.kind==ExternalKind::Func,"Invalid setter export");setter=Some(e.index);}},
  Payload::CodeSectionEntry(b)=>bodies.push(b),_=>{}
 }}
 let (add,ty)=add.ok_or_else(||anyhow::anyhow!("Missing add import"))?;
 let setter=setter.ok_or_else(||anyhow::anyhow!("Missing setTempRet0 export"))?;
 // The reviewed module also has a private leaf-proof memory. Operate only on
 // memory zero; retain every memory and its original index unchanged.
 ensure!(memories>=1,"Requires guest memory zero");
 ensure!(types[ty as usize].params()==[ValType::I32;4]&&types[ty as usize].results()==[ValType::I32],"Unexpected legacy add ABI");
 let setter_ty=&types[functions[setter as usize] as usize];ensure!(setter_ty.params()==[ValType::I32]&&setter_ty.results().is_empty(),"Unexpected setter ABI");
 // Refuse alternate aliases rather than silently leave a mixed import path.
 for payload in Parser::new(0).parse_all(bytes){match payload?{
  Payload::ExportSection(s)=>for e in s{let e=e?;ensure!(e.kind!=ExternalKind::Func||e.index!=add,"Add import exported");},
  Payload::ElementSection(s)=>for e in s{match e?.items{
   ElementItems::Functions(s)=>for f in s{ensure!(f?!=add,"Add import table alias");},
   ElementItems::Expressions(_,s)=>for e in s{let mut r=e?.get_operators_reader();while !r.eof(){if let Operator::RefFunc{function_index}=r.read()?{ensure!(function_index!=add,"Add import ref alias");}}}
  }},_=>{}
 }}
 let helper=functions.len() as u32;let mut code=leb(bodies.len() as u32+1);let mut rewritten=0u64;
 for body in bodies{
  let range=body.range();let mut out=Vec::new();let mut cursor=range.start as usize;let mut ops=body.get_operators_reader()?;
  while !ops.eof(){let start=ops.original_position() as usize;match ops.read()?{
   Operator::Call{function_index} if function_index==add=>{out.extend_from_slice(&bytes[cursor..start]);I::Call(helper).encode(&mut out);cursor=ops.original_position() as usize;rewritten+=1;},
   Operator::RefFunc{function_index}|Operator::ReturnCall{function_index}=>ensure!(function_index!=add,"Unsupported add reference/tail call"),_=>{}
  }}
  out.extend_from_slice(&bytes[cursor..range.end as usize]);code.extend(leb(out.len() as u32));code.extend(out);
 }
 ensure!(rewritten>0,"No add call sites");
 let mem=MemArg{offset:0,align:0,memory_index:0};let mut f=Function::new([(1,V::I64)]);
 // Load all eight bytes before any mutation: unaligned accesses, carry,
 // wrapping, low return and old high-half setter match the existing host ABI.
 for op in [I::LocalGet(0),I::I64Load(mem),I::LocalSet(4),I::LocalGet(0),I::LocalGet(4),
  I::LocalGet(1),I::I64ExtendI32U,I::LocalGet(2),I::I64ExtendI32U,I::I64Const(32),I::I64Shl,I::I64Or,I::I64Add,I::I64Store(mem),
  I::LocalGet(4),I::I64Const(32),I::I64ShrU,I::I32WrapI64,I::Call(setter),I::LocalGet(4),I::I32WrapI64,I::End]{f.instruction(&op);}f.encode(&mut code);
 let mut output=bytes[..8].to_vec();let mut p=8;
 while p<bytes.len(){let id=bytes[p];p+=1;let len=uint(bytes,&mut p)? as usize;let payload=&bytes[p..p+len];p+=len;match id{
  3=>{let mut q=0;let n=uint(payload,&mut q)?;let mut s=leb(n+1);s.extend_from_slice(&payload[q..]);s.extend(leb(ty));section(&mut output,id,&s);},
  10=>section(&mut output,id,&code),_=>section(&mut output,id,payload)
 }}
 wasmparser::Validator::new().validate_all(&output)?;
 let proof=serde_json::json!({"mode":"single-owner-legacy-add-i64-v1","source_sha256":hash(bytes),"transformed_sha256":hash(&output),"rewritten_calls":rewritten,"helper":helper,"set_temp_ret":setter,"preserved":"indices, exports, data, C++ tag and original import; nonshared memory only; wrapping add; old low return/high setter; full read bounds before mutation"});
 Ok((output,proof))
}
fn main()->Result<()>{let a=std::env::args().skip(1).collect::<Vec<_>>();ensure!(a.len()==3,"Usage: prepare-guest-atomics reviewed-EH.wasm output.wasm proof.json");let b=std::fs::read(&a[0])?;ensure!(hash(&b)==SOURCE_SHA,"Requires exact reviewed EH source");ensure!(!std::path::Path::new(&a[1]).exists(),"Output already exists");let(out,proof)=transform(&b)?;std::fs::write(&a[1],out)?;let text=serde_json::to_string_pretty(&proof)?;std::fs::write(&a[2],format!("{text}\n"))?;println!("{text}");Ok(())}

#[cfg(test)]mod tests{
 use super::*;
 use wasmtime::{Engine,Module,Store,Linker,Caller};
 fn fixture()->Vec<u8>{wat::parse_str(r#"(module
  (import "env" "memory" (memory 1))
  (import "env" "___atomic_fetch_add_8" (func $add (param i32 i32 i32 i32) (result i32)))
  (memory (export "proof") 1) (data (memory 1) (i32.const 0) "private-proof")
  (global $high (mut i32) (i32.const 0))
  (func (export "setTempRet0") (param i32) local.get 0 global.set $high)
  (func (export "high") (result i32) global.get $high)
  (func (export "run") (param i32 i32 i32) (result i32) local.get 0 local.get 1 local.get 2 i32.const 5 call $add))"#).unwrap()}
 #[test]fn add_preserves_unaligned_carry_wrap_high_and_bounds(){
  let(out,proof)=transform(&fixture()).unwrap();assert_eq!(proof["rewritten_calls"],1);
  let engine=Engine::default();let module=Module::new(&engine,&out).unwrap();let mut store=Store::new(&engine,0u64);let mut linker=Linker::new(&engine);
  let mem=wasmtime::Memory::new(&mut store,wasmtime::MemoryType::new(1,None)).unwrap();linker.define(&mut store,"env","memory",mem).unwrap();
  linker.func_wrap("env","___atomic_fetch_add_8",|mut c:Caller<'_,u64>,_:i32,_:i32,_:i32,_:i32|->i32{*c.data_mut()+=1;-99}).unwrap();
  let instance=linker.instantiate(&mut store,&module).unwrap();let run=instance.get_typed_func::<(i32,i32,i32),i32>(&mut store,"run").unwrap();let high=instance.get_typed_func::<(),i32>(&mut store,"high").unwrap();
  for address in [0,1,32,65528]{for value in [0u64,0x1_ffffffff,u64::MAX]{for add in [0u64,1,0x100000002,u64::MAX]{
   mem.write(&mut store,address,&value.to_le_bytes()).unwrap();assert_eq!(run.call(&mut store,(address as i32,add as i32,(add>>32) as i32)).unwrap(),value as i32);
   assert_eq!(high.call(&mut store,()).unwrap(),(value>>32) as i32);assert_eq!(u64::from_le_bytes(mem.data(&store)[address..address+8].try_into().unwrap()),value.wrapping_add(add));
  }}}
  mem.write(&mut store,65528,&u64::MAX.to_le_bytes()).unwrap();assert!(run.call(&mut store,(65529,1,0)).is_err());assert_eq!(&mem.data(&store)[65528..],&u64::MAX.to_le_bytes());assert_eq!(*store.data(),0);
  let proof=instance.get_memory(&mut store,"proof").unwrap();assert_eq!(&proof.data(&store)[..13],b"private-proof");
 }
 #[test]fn refuses_shared_memory_and_import_aliases(){
  let bytes=fixture();let text=r#"(module (import "env" "memory" (memory 1 1 shared)))"#;assert!(transform(&wat::parse_str(text).unwrap()).unwrap_err().to_string().contains("single-owner"));
  // Valid source with an exported alias must fail before rewriting.
  let mut out=bytes[..8].to_vec();let mut p=8;while p<bytes.len(){let id=bytes[p];p+=1;let n=uint(&bytes,&mut p).unwrap() as usize;let b=&bytes[p..p+n];p+=n;if id==7{let mut q=0;let n=uint(b,&mut q).unwrap();let mut s=leb(n+1);s.extend_from_slice(&b[q..]);s.extend_from_slice(&[5,b'a',b'l',b'i',b'a',b's',0,0]);section(&mut out,id,&s);}else{section(&mut out,id,b);}}assert!(transform(&out).unwrap_err().to_string().contains("exported"));
 }
}
