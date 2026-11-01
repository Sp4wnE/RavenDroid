//! Offline diagnostic of the pinned module. Does not execute or alter it.
use anyhow::Result;
use wasmparser::{Parser,Payload,TypeRef,Operator};
use std::collections::{BTreeMap,BTreeSet};
fn main()->Result<()>{
 let bytes=std::fs::read(std::env::args().nth(1).unwrap())?;
 let mut imports=0;let mut names=BTreeMap::new();let mut bodies=Vec::new();let mut deps=Vec::new();let mut unsafe_funcs=BTreeSet::new();
 for payload in Parser::new(0).parse_all(&bytes){match payload?{
  Payload::ImportSection(s)=>for i in s.into_imports(){let i=i?;if matches!(i.ty,TypeRef::Func(_)){unsafe_funcs.insert(imports);imports+=1;}},
  Payload::ExportSection(s)=>for e in s{let e=e?;if e.name.starts_with("dynCall_"){names.insert(e.index,e.name.to_string());}},
  Payload::CodeSectionEntry(body)=>{let index=imports+bodies.len() as u32;let mut calls=Vec::new();let mut ops=body.get_operators_reader()?;let mut text=Vec::new();while !ops.eof(){let op=ops.read()?;if names.contains_key(&index){text.push(format!("{op:?}"));}match op{Operator::Call{function_index}=>calls.push(function_index),Operator::CallIndirect{..}|Operator::ReturnCallIndirect{..}|Operator::CallRef{..}|Operator::TableSet{..}|Operator::TableInit{..}|Operator::TableCopy{..}|Operator::TableGrow{..}|Operator::TableFill{..}=>{unsafe_funcs.insert(index);},_=>{}}}deps.push(calls);bodies.push(text);},_=>{}
 }}
 loop{let before=unsafe_funcs.len();for (n,calls) in deps.iter().enumerate(){if calls.iter().any(|i|unsafe_funcs.contains(i)){unsafe_funcs.insert(imports+n as u32);}}if unsafe_funcs.len()==before{break;}}
 println!("Functions={} proven-no-import-or-indirect={}",imports as usize+bodies.len(),imports as usize+bodies.len()-unsafe_funcs.len());
 for (i,n) in names.iter().filter(|(_,n)|["dynCall_v","dynCall_vi","dynCall_iii"].contains(&n.as_str())){println!("{n} index={i} ops={:?}",bodies[(*i-imports) as usize]);}
 Ok(())
}
