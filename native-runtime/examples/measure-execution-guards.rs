//! Small original fixed-work experiment. Does not change game execution guards.
use anyhow::Result;
use std::time::Instant;
use wasmtime::{Config,Engine,Module,Store,Instance};
fn main()->Result<()>{
    let wasm=wat::parse_str(r#"(module (func (export "run") (param $count i32) (result i32)
        (local $i i32) (local $x i32)
        i32.const 17 local.set $x
        (loop $loop
            local.get $x i32.const 1664525 i32.mul i32.const 1013904223 i32.add local.set $x
            local.get $i i32.const 1 i32.add local.tee $i local.get $count i32.lt_u br_if $loop)
        local.get $x))"#)?;
    let mut variants=Vec::new();
    for mode in ["fuel","epoch"]{
        let mut c=Config::new();c.cranelift_opt_level(wasmtime::OptLevel::Speed).consume_fuel(mode=="fuel").epoch_interruption(mode=="epoch");
        let engine=Engine::new(&c)?;let module=Module::new(&engine,&wasm)?;let mut store=Store::new(&engine,());
        if mode=="fuel"{store.set_fuel(u64::MAX)?;}else{store.set_epoch_deadline(u64::MAX/2);}
        let instance=Instance::new(&mut store,&module,&[])?;let f=instance.get_typed_func::<i32,i32>(&mut store,"run")?;
        variants.push((mode,store,f,Vec::new()));
    }
    let mut expected=None;
    for _ in 0..7{for (mode,store,f,times) in &mut variants{
        let clock=Instant::now();let value=f.call(store,100_000_000)?;let micros=clock.elapsed().as_micros();
        if let Some(expected)=expected{assert_eq!(value,expected);}else{expected=Some(value);}
        times.push(micros);println!("{mode}: {micros} us checksum={value}");
    }}
    for(mode,_,_,mut times)in variants{times.sort_unstable();println!("{mode} median_us={}",times[times.len()/2]);}
    Ok(())
}
