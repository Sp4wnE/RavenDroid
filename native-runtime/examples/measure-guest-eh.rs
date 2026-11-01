//! Isolated feasibility experiment: guest catch boundaries instead of host invoke.
//! Does not transform the game or change the shipping runtime/engine profile.
use anyhow::Result;
use wasmtime::{AsContextMut,Caller,Config,Engine,ExnRef,ExnRefPre,ExnType,Linker,Module,RootScope,Store,ValType};

fn main()->Result<()>{
    let mut config=Config::new();config.consume_fuel(true).wasm_gc(false).wasm_function_references(false).gc_support(true).wasm_exceptions(true);
    let(mut inline,mut eager)=(false,false);
    for option in std::env::args().skip(1){match option.as_str(){
        "--inline"=>{config.compiler_inlining(wasmtime::Inlining::Yes);inline=true;},
        "--eager-tables"=>{config.table_lazy_init(false);eager=true;},
        _=>anyhow::bail!("Unknown fixture option: {option}"),
    }}
    println!("CODEGEN_FIXTURE inline={inline} eager_tables={eager}; isolated experiment, not game FPS");
    let engine=Engine::new(&config)?;
    let wasm=wat::parse_str(r#"(module
      (import "host" "choose" (func $choose (param i32) (result i32)))
      (tag $cxx (export "cxx")) (tag $other (export "other"))
      (func (export "inner") (result i32) i32.const 1 call $choose)
      (func (export "trap") (result i32) unreachable)
      (func (export "run") (param i32) (result i32)
        (block $done (result i32)
          (block $caught
            (try_table (catch $cxx $caught)
              local.get 0 call $choose br $done))
          i32.const 57)))"#)?;
    let module=Module::new(&engine,wasm)?;
    let mut linker=Linker::<u64>::new(&engine);
    linker.func_wrap("host","choose",|mut caller:Caller<'_,u64>,mode:i32|->wasmtime::Result<i32>{
        *caller.data_mut()+=1;
        match mode{
            0=>Ok(11),
            1|4=>{
                let name=if mode==1{"cxx"}else{"other"};
                let tag=caller.get_export(name).and_then(|v|v.into_tag()).unwrap();
                let ty=ExnType::new(caller.engine(),std::iter::empty::<ValType>())?;
                let mut scope=RootScope::new(&mut caller);
                let allocator=ExnRefPre::new(&mut scope,ty);
                let exn=ExnRef::new(&mut scope,&allocator,&tag,&[])?;
                scope.as_context_mut().throw(exn)
            },
            2|5=>{
                let name=if mode==2{"inner"}else{"trap"};
                let function=caller.get_export(name).and_then(|v|v.into_func()).unwrap().typed::<(),i32>(&caller)?;
                function.call(&mut caller,())
            },
            _=>Err(wasmtime::Error::msg("foreign host failure")),
        }
    })?;
    let mut store=Store::new(&engine,0u64);store.set_fuel(10_000_000)?;
    let instance=linker.instantiate(&mut store,&module)?;
    let run=instance.get_typed_func::<i32,i32>(&mut store,"run")?;
    assert_eq!(run.call(&mut store,0)?,11);
    assert_eq!(run.call(&mut store,1)?,57);
    assert_eq!(run.call(&mut store,2)?,57); // exception crosses nested host callback
    assert!(format!("{:#}",run.call(&mut store,3).unwrap_err()).contains("foreign host failure"));
    assert!(run.call(&mut store,5).unwrap_err().is::<wasmtime::Trap>());
    assert!(run.call(&mut store,4).unwrap_err().is::<wasmtime::ThrownException>());
    assert!(store.take_pending_exception().is_some());
    println!("PASS normal return, guest catch, nested host callback, unrelated tag, fatal host error and trap; host_calls={}",store.data());
    fixed_work(&engine)?;
    Ok(())
}

fn fixed_work(engine:&Engine)->Result<()>{
    use wasmtime::TypedFunc;
    use std::time::Instant;
    struct State{leaf:Option<TypedFunc<i32,i32>>,calls:u64}
    let mut source=String::from(r#"(module
      (type $t (func (param i32) (result i32)))
      (import "host" "invoke" (func $host (type $t)))
      (tag $cxx) (table 1 funcref) (elem (i32.const 0) $leaf)
      (func $leaf (export "leaf") (type $t) local.get 0 i32.const 1664525 i32.mul i32.const 1013904223 i32.add)
      (func $guest (type $t)
        (block $done (result i32) (block $caught
          (try_table (catch $cxx $caught)
            local.get 0 i32.const 0 call_indirect (type $t) br $done))
          i32.const 0))"#);
    for name in ["host","guest"]{source.push_str(&format!(r#"
      (func (export "{name}") (param $count i32) (result i32) (local $i i32) (local $sum i32)
        (loop $loop
          local.get $sum local.get $i call ${name} i32.add local.set $sum
          local.get $i i32.const 1 i32.add local.tee $i local.get $count i32.lt_u br_if $loop)
        local.get $sum)"#));}
    source.push(')');let module=Module::new(engine,wat::parse_str(&source)?)?;
    let mut linker=Linker::<State>::new(engine);
    linker.func_wrap("host","invoke",|mut caller:Caller<'_,State>,value:i32|->wasmtime::Result<i32>{
        caller.data_mut().calls+=1;let function=caller.data().leaf.as_ref().unwrap().clone();function.call(&mut caller,value)
    })?;
    let mut store=Store::new(engine,State{leaf:None,calls:0});store.set_fuel(u64::MAX)?;
    let instance=linker.instantiate(&mut store,&module)?;
    store.data_mut().leaf=Some(instance.get_typed_func::<i32,i32>(&mut store,"leaf")?);
    let functions=[instance.get_typed_func::<i32,i32>(&mut store,"host")?,instance.get_typed_func::<i32,i32>(&mut store,"guest")?];
    let mut samples=[Vec::new(),Vec::new()];let mut checksum=None;let count=250_000;
    for round in 0..5{for index in [round%2,1-round%2]{
        store.set_fuel(u64::MAX)?;let clock=Instant::now();let value=functions[index].call(&mut store,count)?;let elapsed=clock.elapsed().as_micros();
        if let Some(expected)=checksum{assert_eq!(value,expected);}else{checksum=Some(value);}
        samples[index].push(elapsed);
    }}
    assert_eq!(store.data().calls,count as u64*5);
    for sample in &mut samples{sample.sort_unstable();}
    println!("FIXED_WORK calls_per_sample={count} samples=5 checksum={} host_median_us={} guest_median_us={} (fixture, not game FPS)",checksum.unwrap(),samples[0][2],samples[1][2]);
    Ok(())
}
