//! Lower reviewed host ABIs to equivalent guest instructions, preserving indices.
use anyhow::{Result,ensure};
use std::collections::BTreeMap;
use wasm_encoder::{Encode,Function,Instruction as I};
use wasmparser::{Parser,Payload,TypeRef,Operator,ValType};

pub(crate) fn lower(bytes:&[u8])->Result<(Vec<u8>,serde_json::Value)>{
    let mut types=Vec::new();let mut functions=Vec::new();let mut bodies=Vec::new();
    let mut replacements=BTreeMap::new();let mut names=BTreeMap::new();let mut memcpy=None;let mut memories=0;
    for payload in Parser::new(0).parse_all(bytes){match payload?{
        Payload::TypeSection(s)=>for t in s.into_iter_err_on_gc_types(){types.push(t?);},
        Payload::ImportSection(s)=>for import in s.into_imports(){let import=import?;match import.ty{
            TypeRef::Memory(m)=>{ensure!(!m.memory64&&!m.shared,"Expected nonshared wasm32 memory");memories+=1;},
            TypeRef::Func(ty)=>{
                let index=functions.len() as u32;functions.push(ty);
                if import.module!="env"{continue;}
                let signature=&types[ty as usize];let mut code=Vec::new();
                let (params,results):(Vec<ValType>,Vec<ValType>)=match import.name{
                    "_llvm_trunc_f32"=>{I::F64Trunc.encode(&mut code);(vec![ValType::F64],vec![ValType::F64])},
                    "_llvm_eh_typeid_for"=>(vec![ValType::I32],vec![ValType::I32]),
                    "_getpagesize"=>{I::I32Const(16384).encode(&mut code);(vec![],vec![ValType::I32])},
                    "getTotalMemory"=>{for op in [I::MemorySize(0),I::I32Const(16),I::I32Shl]{op.encode(&mut code);}(vec![],vec![ValType::I32])},
                    "_emscripten_memcpy_big"=>{memcpy=Some((index,ty));(vec![ValType::I32;3],vec![ValType::I32])},
                    _=>continue,
                };
                ensure!(signature.params()==params&&signature.results()==results,"Unexpected intrinsic ABI: {}",import.name);
                replacements.insert(index,code);names.insert(index,import.name.to_string());
            },_=>{}
        }},
        Payload::FunctionSection(s)=>for ty in s{functions.push(ty?);},
        Payload::MemorySection(_)=>anyhow::bail!("Unexpected defined memory"),
        Payload::CodeSectionEntry(body)=>bodies.push(body),_=>{}
    }}
    ensure!(memories==1&&!replacements.is_empty(),"Expected reviewed intrinsic imports and one memory");
    if let Some((index,_))=memcpy{let mut call=Vec::new();I::Call(functions.len() as u32).encode(&mut call);replacements.insert(index,call);}
    let mut code=super::leb((bodies.len()+usize::from(memcpy.is_some())) as u32);let mut counts=BTreeMap::<String,u64>::new();
    for body in bodies{
        let range=body.range();let mut output=Vec::new();let mut cursor=range.start as usize;let mut ops=body.get_operators_reader()?;
        while !ops.eof(){let start=ops.original_position() as usize;if let Operator::Call{function_index}=ops.read()?{
            if let Some(replacement)=replacements.get(&function_index){
                output.extend_from_slice(&bytes[cursor..start]);output.extend_from_slice(replacement);cursor=ops.original_position() as usize;
                *counts.entry(names[&function_index].clone()).or_default()+=1;
            }
        }}
        output.extend_from_slice(&bytes[cursor..range.end as usize]);code.extend(super::leb(output.len() as u32));code.extend(output);
    }
    if memcpy.is_some(){
        let mut f=Function::new([]);for op in [I::LocalGet(0),I::LocalGet(1),I::LocalGet(2),I::MemoryCopy{src_mem:0,dst_mem:0},I::LocalGet(0),I::End]{f.instruction(&op);}f.encode(&mut code);
    }
    let mut output=bytes[..8].to_vec();let mut cursor=8;
    while cursor<bytes.len(){let id=bytes[cursor];cursor+=1;let length=super::uint(bytes,&mut cursor)? as usize;let payload=&bytes[cursor..cursor+length];cursor+=length;
        match id{
            3 if memcpy.is_some()=>{let mut p=0;let count=super::uint(payload,&mut p)?;let mut section=super::leb(count+1);section.extend_from_slice(&payload[p..]);section.extend(super::leb(memcpy.unwrap().1));super::section(&mut output,id,&section);},
            10=>super::section(&mut output,id,&code),_=>super::section(&mut output,id,payload)
        }
    }
    wasmparser::Validator::new().validate_all(&output)?;
    Ok((output,serde_json::json!({"rewritten_call_sites":counts,"memory_index":0,"page_size_result":16384,"original_imports_retained":true,"memcpy":"overlap-safe memory.copy; bounds trap before mutation; returns original destination"})))
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn lowered_numeric_memory_and_copy_abis_preserve_edge_cases(){
        let source=wat::parse_str(r#"(module
          (import "env" "memory" (memory 1 2))
          (import "env" "_llvm_trunc_f32" (func $trunc (param f64) (result f64)))
          (import "env" "_llvm_eh_typeid_for" (func $id (param i32) (result i32)))
          (import "env" "_getpagesize" (func $page (result i32)))
          (import "env" "getTotalMemory" (func $size (result i32)))
          (import "env" "_emscripten_memcpy_big" (func $copy (param i32 i32 i32) (result i32)))
          (func (export "trunc") (param f64) (result f64) local.get 0 call $trunc)
          (func (export "id") (param i32) (result i32) local.get 0 call $id)
          (func (export "page") (result i32) call $page)
          (func (export "size") (result i32) call $size)
          (func (export "copy") (param i32 i32 i32) (result i32) local.get 0 local.get 1 local.get 2 call $copy))"#).unwrap();
        let (bytes,report)=lower(&source).unwrap();assert_eq!(report["rewritten_call_sites"]["_llvm_trunc_f32"],1);
        let engine=wasmtime::Engine::default();let module=wasmtime::Module::new(&engine,bytes).unwrap();let mut store=wasmtime::Store::new(&engine,());let mut linker=wasmtime::Linker::new(&engine);
        let memory=wasmtime::Memory::new(&mut store,wasmtime::MemoryType::new(1,Some(2))).unwrap();linker.define(&mut store,"env","memory",memory).unwrap();
        // Any residual host call fails. Imports stay available for table aliases.
        for import in module.imports(){if let wasmtime::ExternType::Func(ty)=import.ty(){linker.func_new(import.module(),import.name(),ty,|_,_,_|Err(wasmtime::Error::msg("Unexpected host transition"))).unwrap();}}
        let instance=linker.instantiate(&mut store,&module).unwrap();
        let trunc=instance.get_typed_func::<f64,f64>(&mut store,"trunc").unwrap();
        for value in [-0.,-0.75,-1.9,1.9,f64::INFINITY,f64::NEG_INFINITY,f64::NAN]{let actual=trunc.call(&mut store,value).unwrap();if value.is_nan(){assert!(actual.is_nan());}else{assert_eq!(actual.to_bits(),value.trunc().to_bits());}}
        assert_eq!(instance.get_typed_func::<i32,i32>(&mut store,"id").unwrap().call(&mut store,i32::MIN).unwrap(),i32::MIN);
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"page").unwrap().call(&mut store,()).unwrap(),16384);
        let size=instance.get_typed_func::<(),i32>(&mut store,"size").unwrap();assert_eq!(size.call(&mut store,()).unwrap(),65536);memory.grow(&mut store,1).unwrap();assert_eq!(size.call(&mut store,()).unwrap(),131072);
        let copy=instance.get_typed_func::<(i32,i32,i32),i32>(&mut store,"copy").unwrap();memory.write(&mut store,0,b"abcdefgh").unwrap();assert_eq!(copy.call(&mut store,(2,0,6)).unwrap(),2);assert_eq!(&memory.data(&store)[..8],b"ababcdef");
        let before=memory.data(&store)[..8].to_vec();assert!(copy.call(&mut store,(0,131071,2)).is_err());assert_eq!(&memory.data(&store)[..8],before.as_slice());assert!(copy.call(&mut store,(-1,0,0)).is_err());assert_eq!(copy.call(&mut store,(131072,131072,0)).unwrap(),131072);
    }
}
