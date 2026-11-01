//! Experimental, hash-pinned guest catch boundaries layered on the 0.17 proof.
//! Existing indices, private leaf bitset and original dynCall casts are retained.
use anyhow::{Result,ensure,bail};
use sha2::{Digest,Sha256};
use std::{borrow::Cow,collections::BTreeMap};
use wasm_encoder::{BlockType,Catch,Encode,Function,Instruction as I,ValType as V};
use wasmparser::{ExternalKind,Parser,Payload,TypeRef,Operator,ElementItems,ValType};

const TAG:&str="__raven_cxx_tag_v1";
const SOURCE_SHA:&str="97fb7db1805bb557c88bbd5d5e4d2f1e61982aad7b1c1e4b160bf4aa133ac71b";
fn hash(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
fn leb(mut n:u32)->Vec<u8>{let mut out=Vec::new();loop{let b=(n&127) as u8;n>>=7;out.push(b|if n!=0{128}else{0});if n==0{return out;}}}
fn uint(bytes:&[u8],cursor:&mut usize)->Result<u32>{
    let mut n=0;for shift in (0..35).step_by(7){let b=*bytes.get(*cursor).ok_or_else(||anyhow::anyhow!("Truncated LEB"))?;*cursor+=1;ensure!(shift<28||b<16,"Invalid LEB");n|=((b&127) as u32)<<shift;if b<128{return Ok(n);}}bail!("Invalid LEB")
}
fn section(output:&mut Vec<u8>,id:u8,bytes:&[u8]){output.push(id);output.extend(leb(bytes.len() as u32));output.extend_from_slice(bytes);}
fn scalar(ty:ValType)->Result<V>{Ok(match ty{ValType::I32=>V::I32,ValType::I64=>V::I64,ValType::F32=>V::F32,ValType::F64=>V::F64,_=>bail!("Unsupported invoke scalar")})}

pub(crate) fn transform(bytes:&[u8])->Result<(Vec<u8>,serde_json::Value)>{
    ensure!(bytes.len()<=256*1024*1024,"Source exceeds bound");
    wasmparser::Validator::new().validate_all(bytes)?;
    let mut types=Vec::new();let mut functions=Vec::new();let mut globals=Vec::new();
    let mut exports=BTreeMap::new();let mut invokes=BTreeMap::new();let mut bodies=Vec::new();
    for payload in Parser::new(0).parse_all(bytes){match payload?{
        Payload::TypeSection(s)=>for ty in s.into_iter_err_on_gc_types(){types.push(ty?);},
        Payload::ImportSection(s)=>for import in s.into_imports(){let import=import?;match import.ty{
            TypeRef::Func(ty)=>{let index=functions.len() as u32;functions.push(ty);if import.module=="env"&&import.name.starts_with("invoke_"){invokes.insert(index,(ty,import.name.to_string()));}},
            TypeRef::Global(ty)=>globals.push(ty),TypeRef::Tag(_)=>bail!("Source already imports exception tags"),_=>{}
        }},
        Payload::FunctionSection(s)=>for ty in s{functions.push(ty?);},
        Payload::GlobalSection(s)=>for g in s{globals.push(g?.ty);},
        Payload::ExportSection(s)=>for e in s{let e=e?;ensure!(e.name!=TAG,"Source already has guest catch marker");exports.insert(e.name.to_string(),(e.kind,e.index));},
        Payload::TagSection(_)=>bail!("Source already defines exception tags"),
        Payload::ElementSection(s)=>for element in s{match element?.items{
            ElementItems::Functions(s)=>for f in s{ensure!(!invokes.contains_key(&f?),"Invoke import has a table alias");},
            ElementItems::Expressions(_,s)=>for expr in s{let mut r=expr?.get_operators_reader();while !r.eof(){if let Operator::RefFunc{function_index}=r.read()?{ensure!(!invokes.contains_key(&function_index),"Invoke import has an element alias");}}}
        }},
        Payload::CodeSectionEntry(body)=>bodies.push(body),_=>{}
    }}
    ensure!(!invokes.is_empty(),"No invoke imports");
    ensure!(!exports.values().any(|&(kind,index)|kind==ExternalKind::Func&&invokes.contains_key(&index)),"Invoke import is externally exported");
    let function=|name:&str|->Result<u32>{let &(kind,index)=exports.get(name).ok_or_else(||anyhow::anyhow!("Missing export: {name}"))?;ensure!(kind==ExternalKind::Func,"Expected function export: {name}");Ok(index)};
    let &(kind,stack)=exports.get("__raven_stack_pointer_v1").ok_or_else(||anyhow::anyhow!("Reviewed stack export missing"))?;
    ensure!(kind==ExternalKind::Global&&globals[stack as usize].content_type==ValType::I32&&globals[stack as usize].mutable,"Invalid stack global");
    let restore=function("stackRestore")?;let set_threw=function("setThrew")?;
    for (index,params) in [(restore,vec![ValType::I32]),(set_threw,vec![ValType::I32,ValType::I32])]{let ty=&types[functions[index as usize] as usize];ensure!(ty.params()==params&&ty.results().is_empty(),"Cleanup helper ABI mismatch");}
    let void_type=types.iter().position(|ty|ty.params().is_empty()&&ty.results().is_empty()).unwrap_or(types.len()) as u32;
    let append_void=void_type as usize==types.len();
    let mut wrappers=Vec::new();let mut replacements=BTreeMap::new();
    for (&import,(ty,name)) in &invokes{
        let trampoline=function(&format!("dynCall_{}",name.strip_prefix("invoke_").unwrap()))?;
        let signature=&types[*ty as usize];
        ensure!(signature==&types[functions[trampoline as usize] as usize],"dynCall ABI mismatch");
        ensure!(signature.params().first()==Some(&ValType::I32)&&signature.results().len()<=1,"Unsupported invoke ABI");
        ensure!(signature.results().iter().all(|ty|matches!(ty,ValType::I32|ValType::F64)),"Unsupported legacy exception result ABI");
        for ty in signature.params().iter().chain(signature.results()){scalar(*ty)?;}
        let mut replacement=Vec::new();I::Call(functions.len() as u32+wrappers.len() as u32).encode(&mut replacement);
        replacements.insert(import,replacement);wrappers.push((*ty,trampoline));
    }
    let mut rewritten=BTreeMap::<String,u64>::new();let mut code=leb((bodies.len()+wrappers.len()) as u32);
    for body in bodies{
        let range=body.range();let mut cursor=range.start as usize;let mut output=Vec::new();let mut reader=body.get_operators_reader()?;
        while !reader.eof(){let start=reader.original_position() as usize;match reader.read()?{
            Operator::Call{function_index}=>if let Some(replacement)=replacements.get(&function_index){
                output.extend_from_slice(&bytes[cursor..start]);output.extend_from_slice(replacement);cursor=reader.original_position() as usize;
                *rewritten.entry(invokes[&function_index].1.clone()).or_default()+=1;
            },
            Operator::RefFunc{function_index}|Operator::ReturnCall{function_index}=>ensure!(!invokes.contains_key(&function_index),"Unsupported invoke reference/tail call"),_=>{}
        }}
        output.extend_from_slice(&bytes[cursor..range.end as usize]);code.extend(leb(output.len() as u32));code.extend(output);
    }
    for &(ty,trampoline) in &wrappers{
        let signature=&types[ty as usize];let saved=signature.params().len() as u32;let result=signature.results().first().copied();
        let mut f=Function::new([(1,V::I32)]);
        f.instruction(&I::GlobalGet(stack)).instruction(&I::LocalSet(saved));
        f.instruction(&I::Block(if let Some(ty)=result{BlockType::Result(scalar(ty)?)}else{BlockType::Empty}));
        f.instruction(&I::Block(BlockType::Empty));
        f.instruction(&I::TryTable(BlockType::Empty,Cow::Owned(vec![Catch::One{tag:0,label:0}])));
        for arg in 0..saved{f.instruction(&I::LocalGet(arg));}
        f.instruction(&I::Call(trampoline)).instruction(&I::Br(2)).instruction(&I::End).instruction(&I::End);
        f.instruction(&I::LocalGet(saved)).instruction(&I::Call(restore));
        f.instruction(&I::I32Const(1)).instruction(&I::I32Const(0)).instruction(&I::Call(set_threw));
        if let Some(ty)=result{f.instruction(&match ty{ValType::I32=>I::I32Const(0),ValType::F64=>I::F64Const(wasm_encoder::Ieee64::new(f64::NAN.to_bits())),_=>unreachable!()});}
        f.instruction(&I::End).instruction(&I::End);f.encode(&mut code);
    }
    let mut output=bytes[..8].to_vec();let mut cursor=8;let mut inserted=false;
    while cursor<bytes.len(){let id=bytes[cursor];cursor+=1;let length=uint(bytes,&mut cursor)? as usize;let payload=&bytes[cursor..cursor+length];cursor+=length;
        if !inserted&&matches!(id,6|7){let mut tags=vec![1,0];tags.extend(leb(void_type));section(&mut output,13,&tags);inserted=true;}
        match id{
            1 if append_void=>{let mut p=0;let count=uint(payload,&mut p)?;let mut s=leb(count+1);s.extend_from_slice(&payload[p..]);s.extend_from_slice(&[0x60,0,0]);section(&mut output,id,&s);},
            3=>{let mut p=0;let count=uint(payload,&mut p)?;let mut s=leb(count+wrappers.len() as u32);s.extend_from_slice(&payload[p..]);for &(ty,_) in &wrappers{s.extend(leb(ty));}section(&mut output,id,&s);},
            7=>{let mut p=0;let count=uint(payload,&mut p)?;let mut s=leb(count+1);s.extend_from_slice(&payload[p..]);s.extend(leb(TAG.len() as u32));s.extend_from_slice(TAG.as_bytes());s.extend_from_slice(&[4,0]);section(&mut output,id,&s);},
            10=>section(&mut output,id,&code),_=>section(&mut output,id,payload)
        }
    }
    ensure!(inserted,"No insertion point for exception tag");wasmparser::Validator::new().validate_all(&output)?;
    let proof=serde_json::json!({"version":1,"mode":"guest-cxx-tag-v1","tag_export":TAG,"source_sha256":hash(bytes),"transformed_sha256":hash(&output),"wrappers":wrappers.len(),"rewritten_calls":rewritten,"stack_global":stack,"stack_restore":restore,"set_threw":set_threw,"exception_returns":"i32=0; f64=NaN; void=none","preserved":"original indices/dynCall casts/leaf proof; only dedicated C++ tag caught; other errors/traps propagate"});
    Ok((output,proof))
}

fn main()->Result<()>{
    let args=std::env::args().skip(1).collect::<Vec<_>>();ensure!(args.len()==3,"Usage: prepare-guest-eh reviewed-017.wasm output.wasm proof.json");
    let bytes=std::fs::read(&args[0])?;ensure!(hash(&bytes)==SOURCE_SHA,"Requires exact reviewed 0.17 transformed source");
    ensure!(!std::path::Path::new(&args[1]).exists(),"Output already exists; preserve comparison artifacts");
    let (output,proof)=transform(&bytes)?;std::fs::write(&args[1],output)?;std::fs::write(&args[2],serde_json::to_string_pretty(&proof)?+"\n")?;
    println!("{}",serde_json::to_string_pretty(&proof)?);Ok(())
}

#[cfg(all(test,feature="guest-eh-prototype"))]mod tests{
    use super::*;
    use wasmtime::{AsContextMut,Caller,Config,Engine,ExnRef,ExnRefPre,ExnType,Linker,Module,RootScope,Store};
    fn fixture()->Vec<u8>{wat::parse_str(r#"(module
      (type $callee (func (param i32) (result i32))) (type $double (func (result f64)))
      (import "env" "invoke_ii" (func $invoke (param i32 i32) (result i32)))
      (import "env" "invoke_d" (func $invoke_d (param i32) (result f64)))
      (import "env" "throw" (func $throw (param i32)))
      (global $sp (export "__raven_stack_pointer_v1") (mut i32) (i32.const 1024))
      (global $threw (export "threw") (mut i32) (i32.const 0))
      (func (export "stackSave") (result i32) global.get $sp)
      (func (export "stackRestore") (param i32) local.get 0 global.set $sp)
      (func (export "setThrew") (param i32 i32) local.get 0 global.set $threw)
      (table 5 funcref) (elem (i32.const 0) $normal $throws $trap $nested $double_throw)
      (func $normal (type $callee) i32.const 4096 global.set $sp local.get 0 i32.const 1 i32.add)
      (func $throws (type $callee) i32.const 8192 global.set $sp local.get 0 call $throw unreachable)
      (func $trap (type $callee) i32.const 16384 global.set $sp unreachable)
      (func $nested (type $callee) i32.const 2048 global.set $sp i32.const 1 local.get 0 call $invoke)
      (func $double_throw (type $double) i32.const 12288 global.set $sp i32.const 7 call $throw unreachable)
      (func (export "dynCall_ii") (param i32 i32) (result i32) local.get 1 local.get 0 call_indirect (type $callee))
      (func (export "dynCall_d") (param i32) (result f64) local.get 0 call_indirect (type $double))
      (func (export "run") (param i32) (result i32) local.get 0 i32.const 71 call $invoke)
      (func (export "run-double") (result f64) i32.const 4 call $invoke_d))"#).unwrap()}

    #[test]fn guest_boundaries_preserve_normal_nested_throw_stack_and_traps(){
        let (bytes,proof)=transform(&fixture()).unwrap();assert_eq!(proof["wrappers"],2);
        let mut config=Config::new();config.consume_fuel(true).wasm_gc(false).wasm_function_references(false).gc_support(true).wasm_exceptions(true);
        let engine=Engine::new(&config).unwrap();let module=Module::new(&engine,&bytes).unwrap();
        let mut store=Store::new(&engine,(0u64,0u64));store.set_fuel(10_000_000).unwrap();let mut linker=Linker::new(&engine);
        linker.func_wrap("env","invoke_ii",|mut c:Caller<'_,(u64,u64)>,_:i32,_:i32|->i32{c.data_mut().0+=1;-1}).unwrap();
        linker.func_wrap("env","invoke_d",|mut c:Caller<'_,(u64,u64)>,_:i32|->f64{c.data_mut().0+=1;-1.}).unwrap();
        linker.func_wrap("env","throw",|mut c:Caller<'_,(u64,u64)>,_:i32|->wasmtime::Result<()>{
            c.data_mut().1+=1;let tag=c.get_export(TAG).and_then(|v|v.into_tag()).unwrap();
            let ty=ExnType::new(c.engine(),std::iter::empty::<wasmtime::ValType>())?;let mut scope=RootScope::new(&mut c);
            let allocator=ExnRefPre::new(&mut scope,ty);let exn=ExnRef::new(&mut scope,&allocator,&tag,&[])?;scope.as_context_mut().throw(exn)
        }).unwrap();
        let instance=linker.instantiate(&mut store,&module).unwrap();let run=instance.get_typed_func::<i32,i32>(&mut store,"run").unwrap();
        let stack=instance.get_global(&mut store,"__raven_stack_pointer_v1").unwrap();let threw=instance.get_global(&mut store,"threw").unwrap();
        assert_eq!(run.call(&mut store,0).unwrap(),72);assert_eq!(stack.get(&mut store).i32(),Some(4096));
        stack.set(&mut store,wasmtime::Val::I32(1024)).unwrap();assert_eq!(run.call(&mut store,1).unwrap(),0);
        assert_eq!(stack.get(&mut store).i32(),Some(1024));assert_eq!(threw.get(&mut store).i32(),Some(1));
        stack.set(&mut store,wasmtime::Val::I32(1024)).unwrap();assert_eq!(run.call(&mut store,3).unwrap(),0);
        assert_eq!(stack.get(&mut store).i32(),Some(2048));
        stack.set(&mut store,wasmtime::Val::I32(1024)).unwrap();let double=instance.get_typed_func::<(),f64>(&mut store,"run-double").unwrap();
        assert!(double.call(&mut store,()).unwrap().is_nan());assert_eq!(stack.get(&mut store).i32(),Some(1024));
        threw.set(&mut store,wasmtime::Val::I32(0)).unwrap();assert!(run.call(&mut store,2).unwrap_err().is::<wasmtime::Trap>());
        assert_eq!(threw.get(&mut store).i32(),Some(0));assert_eq!(stack.get(&mut store).i32(),Some(16384));assert_eq!(store.data(),&(0,3));
        assert!(transform(&bytes).is_err());
    }

    #[test]fn imported_invoke_table_alias_is_rejected(){
        let text=r#"(module (import "env" "invoke_v" (func $invoke (param i32))) (table 1 funcref) (elem (i32.const 0) $invoke))"#;
        assert!(transform(&wat::parse_str(text).unwrap()).unwrap_err().to_string().contains("table alias"));
    }
}
