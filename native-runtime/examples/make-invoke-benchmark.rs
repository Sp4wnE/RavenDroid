//! Developer fixture generator. No game assets, timing claim or phone install.
//! cargo run --release --features aot-builder --example make-invoke-benchmark -- OUTPUT_DIR
use anyhow::{Context,Result};
fn main()->Result<()>{
    let output=std::path::PathBuf::from(std::env::args().nth(1).context("Output directory required")?);
    std::fs::create_dir_all(&output)?;
    let count=5_000_000u64;
    let expected=(count*(count-1)/2) as u32 as i32;
    let expose=std::env::args().any(|a|a=="--stack-export");
    let stack_export=if expose{"(export \"__raven_stack_pointer_v1\")"}else{""};
    let mut source=format!(r#"(module
        (import "env" "memory" (memory 512))
        (import "env" "invoke_iii" (func $invoke (param i32 i32 i32) (result i32)))
        (global $heap (mut i32) (i32.const 4096))
        (func (export "bench_ctor"))
        (global $stack {stack_export} (mut i32) (i32.const 2048))
        (func (export "stackSave") (result i32) global.get $stack)
        (func (export "_malloc") (param $size i32) (result i32) (local $old i32)
            global.get $heap local.tee $old local.get $size i32.add global.set $heap local.get $old)
        (func (export "_free") (param i32))
        (func (export "dynCall_iii") (param i32 i32 i32) (result i32)
            local.get 1 local.get 2 i32.add)
        (func (export "_main") (param i32 i32) (result i32) (local $index i32) (local $sum i32)
            (block $done (loop $loop
                local.get $index i32.const {count} i32.ge_u br_if $done
                i32.const 0 local.get $sum local.get $index call $invoke local.set $sum
                local.get $index i32.const 1 i32.add local.set $index br $loop))
            local.get $sum i32.const {expected} i32.ne if unreachable end i32.const 0))"#);
    if std::env::args().any(|a|a=="--mixed") {
        source=source.replace("invoke_iii", "invoke_ifi").replace("dynCall_iii", "dynCall_ifi")
            .replace("(func $invoke (param i32 i32 i32)","(func $invoke (param i32 f64 i32)")
            .replace("(param i32 i32 i32) (result i32)","(param i32 f64 i32) (result i32)")
            .replace("local.get 1 local.get 2 i32.add", "local.get 1 i32.trunc_f64_s local.get 2 i32.add")
            .replace("i32.const 0 local.get $sum local.get $index", "i32.const 0 local.get $sum f64.convert_i32_s local.get $index");
    }
    let wasm=output.join("benchmark.wasm");std::fs::write(&wasm,wat::parse_str(&source)?)?;
    std::fs::write(output.join("benchmark.wat"),source)?;
    std::fs::write(output.join("framework.js"),"__ATINIT__.push({func:(function(){bench_ctor()})});")?;
    let target=std::env::args().nth(2).unwrap_or_else(||"x86_64-pc-windows-gnu".into());
    ravenfield_native_host::precompile_for_target_optimized(wasm.to_str().unwrap(),&target,output.join("benchmark.cwasm").to_str().unwrap(),true)?;
    let signature=if std::env::args().any(|a|a=="--mixed"){"invoke_ifi"}else{"invoke_iii"};
    println!("Generated {count} real {signature} calls with checked wrapping sum {expected}");
    Ok(())
}
