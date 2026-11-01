use super::*;
use tests::fixture;
#[cfg(feature="aot-builder")]
#[path="../examples/prepare-leaf-invokes.rs"]mod leaf_transform;
#[cfg(all(feature="aot-builder",feature="guest-eh-prototype"))]
#[path="../examples/prepare-guest-eh.rs"]mod guest_eh_transform;
#[cfg(feature="aot-builder")]
#[path="../examples/prepare-guest-atomics.rs"]mod guest_atomic_transform;
#[cfg(feature="aot-builder")]
#[test]fn guest_atomic_add_matches_production_host_return_and_memory(){
    let original=wat::parse_str(r#"(module
      (import "env" "memory" (memory 512))
      (import "env" "___atomic_fetch_add_8" (func $add (param i32 i32 i32 i32) (result i32)))
      (global $high (mut i32) (i32.const 0))
      (func (export "setTempRet0") (param i32) local.get 0 global.set $high)
      (func (export "high") (result i32) global.get $high)
      (func (export "run") (param i32 i32 i32) (result i32) local.get 0 local.get 1 local.get 2 i32.const 5 call $add))"#).unwrap();
    let (lowered,_)=guest_atomic_transform::transform(&original).unwrap();
    for (source,expected_calls) in [(original,1),(lowered,0)]{
        let engine=Engine::new(&engine_config()).unwrap();let module=Module::new(&engine,source).unwrap();
        let(mut store,instance)=initialize(&engine,&module).unwrap();let memory=store.data().memory.unwrap();
        let initial=0x1_ffffffffu64;memory.write(&mut store,33,&initial.to_le_bytes()).unwrap();
        let run=instance.get_typed_func::<(i32,i32,i32),i32>(&mut store,"run").unwrap();assert_eq!(run.call(&mut store,(33,2,-1)).unwrap(),-1);
        assert_eq!(u64::from_le_bytes(memory.data(&store)[33..41].try_into().unwrap()),initial.wrapping_add(0xffffffff00000002));
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"high").unwrap().call(&mut store,()).unwrap(),1);
        assert_eq!(store.data().calls.iter().find(|(name,_)|name=="env.___atomic_fetch_add_8").unwrap().1,expected_calls);
        let end=memory.data_size(&store);let before=memory.data(&store)[end-8..].to_vec();assert!(run.call(&mut store,((end-7) as i32,1,0)).is_err());assert_eq!(&memory.data(&store)[end-8..],before.as_slice());
    }
}
#[cfg(feature="aot-builder")]
#[test]
fn host_boundary_transform_keeps_real_nested_cpp_throw_and_stack_restore(){
    let original=wat::parse_str(r#"(module
      (type $target (func (param i32) (result i32)))
      (import "env" "memory" (memory 512))
      (import "env" "table" (table 4 4 funcref))
      (import "env" "invoke_ii" (func $invoke (param i32 i32) (result i32)))
      (import "env" "___cxa_throw" (func $throw (param i32 i32 i32)))
      (global $stack (export "__raven_stack_pointer_v1") (mut i32) (i32.const 1024))
      (func (export "stackSave") (result i32) global.get $stack)
      (func (export "stackRestore") (param i32) local.get 0 global.set $stack)
      (func (export "setThrew") (param i32 i32) i32.const 100 local.get 0 i32.store)
      (func $leaf (type $target) local.get 0)
      (func $nested (type $target)
        i32.const 4096 global.set $stack
        i32.const 104 i32.const 2 local.get 0 call $invoke i32.store
        i32.const 108 global.get $stack i32.store
        i32.const 1024 global.set $stack i32.const 77)
      (func $thrower (type $target)
        i32.const 8192 global.set $stack
        i32.const 128 i32.const 7 i32.const 0 call $throw unreachable)
      (elem (i32.const 0) $leaf $nested $thrower $thrower)
      (func (export "dynCall_ii") (param i32 i32) (result i32)
        local.get 1 local.get 0 i32.const 3 i32.and i32.const 0 i32.add call_indirect (type $target))
      (func (export "run") (result i32) i32.const 1 i32.const 42 call $invoke)
      (data (i32.const 0) "original"))"#).unwrap();
    let (transformed,proof)=leaf_transform::transform_mode(&original,None,true).unwrap();
    assert_eq!(proof["safe_table_entries"],2);
    #[allow(unused_mut)]let mut cases=vec![(original,2,false),(transformed.clone(),1,false)];
    #[cfg(feature="guest-eh-prototype")]{let (guest,_)=guest_eh_transform::transform(&transformed).unwrap();cases.push((guest,0,true));}
    for (bytes,expected_calls,guest_eh) in cases{
      for (inline_eager,epoch_guard) in [(false,false),(true,false),(false,true)]{
        let engine=Engine::new(&engine_config_execution(guest_eh,inline_eager,epoch_guard)).unwrap();let module=Module::new(&engine,&bytes).unwrap();
        let _watchdog=epoch_guard.then(||execution_guard::Watchdog::start(&engine).unwrap());
        let(mut store,instance)=initialize_guarded(&engine,&module,epoch_guard).unwrap();
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"run").unwrap().call(&mut store,()).unwrap(),77);
        assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"stackSave").unwrap().call(&mut store,()).unwrap(),1024);
        let data=store.data().memory.unwrap().data(&store);
        assert_eq!(i32::from_le_bytes(data[100..104].try_into().unwrap()),1);
        assert_eq!(i32::from_le_bytes(data[104..108].try_into().unwrap()),0);
        assert_eq!(i32::from_le_bytes(data[108..112].try_into().unwrap()),4096);
        assert_eq!(store.data().calls.iter().find(|(name,_)|name=="env.invoke_ii").unwrap().1,expected_calls);
      }
    }
}
#[cfg(feature="aot-builder")]
#[test]
#[ignore="Fixed-work benchmark; run alone with --nocapture after other compilers stop"]
fn leaf_invoke_fixed_work_benchmark(){
    let source=wat::parse_str(r#"(module
      (type $target (func (param i32) (result i32)))
      (import "env" "memory" (memory 512)) (import "env" "table" (table 2 2 funcref))
      (import "env" "invoke_ii" (func $invoke (param i32 i32) (result i32)))
      (global (export "__raven_stack_pointer_v1") (mut i32) (i32.const 1024))
      (func $leaf (type $target) local.get 0 i32.const 1 i32.add)
      (elem (i32.const 0) $leaf $leaf)
      (func (export "dynCall_ii") (param i32 i32) (result i32) local.get 1 local.get 0 i32.const 1 i32.and i32.const 0 i32.add call_indirect (type $target))
      (func (export "run") (result i32) (local $n i32)
        loop $again i32.const 0 local.get $n call $invoke local.tee $n i32.const 5000000 i32.lt_u br_if $again end local.get $n)
      (data (i32.const 0) ""))"#).unwrap();
    let (transformed,_)=leaf_transform::transform(&source).unwrap();let mut config=Config::new();config.consume_fuel(true).cranelift_opt_level(wasmtime::OptLevel::Speed);let engine=Engine::new(&config).unwrap();
    let modules=[Module::new(&engine,&source).unwrap(),Module::new(&engine,&transformed).unwrap()];
    let mut measurements=[Vec::new(),Vec::new()];
    for round in 0..6{for index in if round%2==0{[0,1]}else{[1,0]}{
        let(mut store,instance)=initialize(&engine,&modules[index]).unwrap();store.set_fuel(10_000_000_000).unwrap();let run=instance.get_typed_func::<(),i32>(&mut store,"run").unwrap();let clock=Instant::now();assert_eq!(run.call(&mut store,()).unwrap(),5_000_000);let elapsed=clock.elapsed().as_micros();
        if round>0{measurements[index].push(elapsed);}let count=store.data().calls.iter().find(|(n,_)|n=="env.invoke_ii").unwrap().1;assert_eq!(count,if index==0{5_000_000}else{0});
    }}
    println!("leaf-invoke-fixed-work-5M-us original={:?} transformed={:?}",measurements[0],measurements[1]);
}
#[cfg(feature="aot-builder")]
#[test]fn transformed_leaf_invokes_preserve_real_cpp_restore_and_unrelated_traps(){
    let source=wat::parse_str(r#"(module
      (type $target (func (param i32)))
      (import "env" "memory" (memory 512)) (import "env" "table" (table 4 4 funcref))
      (import "env" "invoke_vi" (func $invoke (param i32 i32)))
      (import "env" "___cxa_throw" (func $throw (param i32 i32 i32)))
      (global $stack (export "__raven_stack_pointer_v1") (mut i32) (i32.const 1024))
      (func (export "stackSave") (result i32) global.get $stack)
      (func (export "stackRestore") (param i32) local.get 0 global.set $stack)
      (func (export "setThrew") (param i32 i32) i32.const 100 local.get 0 i32.store)
      (func $leaf (type $target) i32.const 64 local.get 0 i32.store)
      (func $throws (type $target) i32.const 4096 global.set $stack i32.const 128 i32.const 7 i32.const 0 call $throw)
      (func $traps (type $target) i32.const 8192 global.set $stack unreachable)
      (func $transitive (type $target) local.get 0 call $throws)
      (elem (i32.const 0) $leaf $throws $traps $transitive)
      (func (export "dynCall_vi") (param i32 i32) local.get 1 local.get 0 i32.const 3 i32.and i32.const 0 i32.add call_indirect (type $target))
      (func (export "run") (param i32) local.get 0 i32.const 42 call $invoke)
      (data (i32.const 0) ""))"#).unwrap();
    let (transformed,proof)=leaf_transform::transform(&source).unwrap();assert_eq!(proof["safe_table_entries"],2);
    for bytes in [&source,&transformed]{for throwing_index in [1,3]{
        let mut config=Config::new();config.consume_fuel(true);let engine=Engine::new(&config).unwrap();let module=Module::new(&engine,bytes).unwrap();
        let(mut store,instance)=initialize(&engine,&module).unwrap();store.set_fuel(1_000_000).unwrap();
        let run=instance.get_typed_func::<i32,()>(&mut store,"run").unwrap();run.call(&mut store,0).unwrap();
        let memory=store.data().memory.unwrap();assert_eq!(&memory.data(&store)[64..68],&42i32.to_le_bytes());
        let calls=store.data().calls.iter().find(|(n,_)|n=="env.invoke_vi").unwrap().1;assert_eq!(calls,if bytes==&source{1}else{0});
        run.call(&mut store,throwing_index).unwrap();assert_eq!(&memory.data(&store)[100..104],&1i32.to_le_bytes());
        let stack=instance.get_typed_func::<(),i32>(&mut store,"stackSave").unwrap();assert_eq!(stack.call(&mut store,()).unwrap(),1024);
        assert!(run.call(&mut store,2).is_err());assert_eq!(stack.call(&mut store,()).unwrap(),8192);
    }}
}
#[test]fn cached_integer_invokes_preserve_every_argument_and_generic_float_fallback(){
    for signature in ["v","vi","vii","viii","viiii","viiiii","viiiiii","i","ii","iii","iiii","iiiii","iiiiii","iiiiiii"]{
        let returns=signature.starts_with('i');let count=signature.len();
        let params=" i32".repeat(count);let result=if returns{" (result i32)"}else{""};
        let mut sum=String::from("i32.const 0");let mut expected=0;
        for index in 0..count{let weight=3i32.pow(index as u32);sum.push_str(&format!(" local.get {index} i32.const {weight} i32.mul i32.add"));expected+=(index as i32-2)*weight;}
        let body=if returns{sum}else{format!("i32.const 64 {sum} i32.store")};
        let args=(0..count).map(|index|format!("i32.const {}",index as i32-2)).collect::<Vec<_>>().join(" ");
        let after=if returns{""}else{"i32.const 64 i32.load"};
        let source=format!(r#"(module (import "env" "memory" (memory 512))
            (import "env" "invoke_{signature}" (func $invoke (param{params}){result}))
            (func (export "dynCall_{signature}") (param{params}){result} {body})
            (func (export "run") (result i32) {args} call $invoke {after}))"#);
        let(mut store,instance)=fixture(&source);let run=instance.get_typed_func::<(),i32>(&mut store,"run").unwrap();
        assert_eq!(run.call(&mut store,()).unwrap(),expected,"{signature}");
        assert_eq!(run.call(&mut store,()).unwrap(),expected,"cached {signature}");
    }
    let(mut store,instance)=fixture(r#"(module (import "env" "invoke_d" (func $invoke (param i32) (result f64)))
        (func (export "dynCall_d") (param i32) (result f64) local.get 0 f64.convert_i32_s f64.const 0.5 f64.add)
        (func (export "run") (result f64) i32.const 42 call $invoke))"#);
    let run=instance.get_typed_func::<(),f64>(&mut store,"run").unwrap();
    assert_eq!(run.call(&mut store,()).unwrap(),42.5);assert_eq!(run.call(&mut store,()).unwrap(),42.5);
}
#[test]fn environment_constructor_populates_a_real_guest_environ_array(){
    let (mut store,instance)=fixture(r#"(module (import "env" "memory" (memory 512))
      (import "env" "___buildEnvironment" (func $build (param i32)))
      (import "env" "_getenv" (func $get (param i32) (result i32)))
      (func (export "_malloc") (param i32) (result i32) i32.const 4096)
      (func (export "___errno_location") (result i32) i32.const 100)
      (func (export "init") i32.const 128 call $build)
      (func (export "get") (result i32) i32.const 256 call $get)
      (data (i32.const 256) "USER\00"))"#);
    instance.get_typed_func::<(),()>(&mut store,"init").unwrap().call(&mut store,()).unwrap();
    let pointer=instance.get_typed_func::<(),i32>(&mut store,"get").unwrap().call(&mut store,()).unwrap() as usize;
    let memory=store.data().memory.unwrap();assert_eq!(&memory.data(&store)[pointer..pointer+9],b"web_user\0");
    let array=u32::from_le_bytes(memory.data(&store)[128..132].try_into().unwrap()) as usize;
    assert_ne!(array,0);assert_eq!(&memory.data(&store)[array+7*4..array+8*4],&0u32.to_le_bytes());
}
#[test]fn clocks_write_wasm32_timespec_units_and_reject_unknown_clock_ids(){
    let(mut store,instance)=fixture(r#"(module (import "env" "memory" (memory 512))
      (import "env" "_clock_getres" (func $res (param i32 i32) (result i32)))
      (import "env" "_clock_gettime" (func $time (param i32 i32) (result i32)))
      (func (export "___errno_location") (result i32) i32.const 100)
      (func (export "res") (param i32) (result i32) local.get 0 i32.const 256 call $res)
      (func (export "time") (param i32) (result i32) local.get 0 i32.const 256 call $time))"#);
    let memory=store.data().memory.unwrap();let res=instance.get_typed_func::<i32,i32>(&mut store,"res").unwrap();assert_eq!(res.call(&mut store,1).unwrap(),0);assert_eq!(&memory.data(&store)[256..264],&[0,0,0,0,232,3,0,0]);
    assert_eq!(res.call(&mut store,55).unwrap(),-1);assert_eq!(&memory.data(&store)[100..104],&22i32.to_le_bytes());
    assert_eq!(instance.get_typed_func::<i32,i32>(&mut store,"time").unwrap().call(&mut store,1).unwrap(),0);
    let nanos=i32::from_le_bytes(memory.data(&store)[260..264].try_into().unwrap());assert!((0..1_000_000_000).contains(&nanos));assert_eq!(nanos%1000,0);
}
#[test]fn guest_calendar_writes_real_tm_and_respects_strftime_buffer_capacity(){
    let(mut store,instance)=fixture(r#"(module (import "env" "memory" (memory 512))
      (import "env" "_gmtime" (func $gmtime (param i32) (result i32)))
      (import "env" "_mktime" (func $mktime (param i32) (result i32)))
      (import "env" "_strftime" (func $format (param i32 i32 i32 i32) (result i32)))
      (global $heap (mut i32) (i32.const 4096))
      (func (export "_malloc") (param i32) (result i32) (local i32) global.get $heap local.tee 1 local.get 0 i32.add global.set $heap local.get 1)
      (func (export "gmtime") (result i32) i32.const 64 call $gmtime)
      (func (export "mktime") (param i32) (result i32) local.get 0 call $mktime)
      (func (export "format") (param i32 i32) (result i32) i32.const 256 local.get 0 i32.const 128 local.get 1 call $format))"#);
    let memory=store.data().memory.unwrap();memory.write(&mut store,64,&0i32.to_le_bytes()).unwrap();memory.write(&mut store,128,b"%F %Z\0").unwrap();
    let tm=instance.get_typed_func::<(),i32>(&mut store,"gmtime").unwrap().call(&mut store,()).unwrap();
    assert_eq!(instance.get_typed_func::<i32,i32>(&mut store,"mktime").unwrap().call(&mut store,tm).unwrap(),0);
    let format=instance.get_typed_func::<(i32,i32),i32>(&mut store,"format").unwrap();memory.write(&mut store,256,&[0xA5;16]).unwrap();
    assert_eq!(format.call(&mut store,(14,tm)).unwrap(),0);assert_eq!(&memory.data(&store)[256..272],&[0xA5;16]);
    assert_eq!(format.call(&mut store,(15,tm)).unwrap(),14);assert_eq!(&memory.data(&store)[256..271],b"1970-01-01 UTC\0");
}
#[test]fn virtual_uname_preserves_the_pinned_layout_and_rejects_short_buffers(){
    let(mut store,instance)=fixture(r#"(module (import "env" "memory" (memory 512))
      (import "env" "___syscall122" (func $uname (param i32 i32) (result i32)))
      (func (export "uname") (result i32) i32.const 122 i32.const 64 call $uname))"#);
    let memory=store.data().memory.unwrap();memory.write(&mut store,64,&256i32.to_le_bytes()).unwrap();
    let function=instance.get_typed_func::<(),i32>(&mut store,"uname").unwrap();assert_eq!(function.call(&mut store,()).unwrap(),0);
    assert_eq!(&memory.data(&store)[256..267],b"Emscripten\0");assert_eq!(&memory.data(&store)[516..523],b"x86-JS\0");
    let end=memory.data_size(&store) as i32;memory.write(&mut store,64,&(end-100).to_le_bytes()).unwrap();assert_eq!(function.call(&mut store,()).unwrap(),-14);
}
#[test]fn lowered_math_imports_preserve_ieee_special_values(){
    let(mut store,instance)=fixture(r#"(module (import "env" "memory" (memory 512))
      (import "env" "_llvm_log2_f32" (func $log (param f64) (result f64)))
      (import "asm2wasm" "f64-rem" (func $rem (param f64 f64) (result f64)))
      (func (export "log") (param f64) (result f64) local.get 0 call $log)
      (func (export "rem") (param f64 f64) (result f64) local.get 0 local.get 1 call $rem))"#);
    let log=instance.get_typed_func::<f64,f64>(&mut store,"log").unwrap();assert_eq!(log.call(&mut store,4.).unwrap(),2.);assert_eq!(log.call(&mut store,0.).unwrap(),f64::NEG_INFINITY);assert!(log.call(&mut store,-1.).unwrap().is_nan());
    let rem=instance.get_typed_func::<(f64,f64),f64>(&mut store,"rem").unwrap();assert_eq!(rem.call(&mut store,(-0.,1.)).unwrap().to_bits(),(-0f64).to_bits());assert!(rem.call(&mut store,(1.,0.)).unwrap().is_nan());
}
#[test]fn guest_cpp_exception_restores_stack_matches_rtti_and_runs_destructor(){
    let source=r#"(module
      (import "env" "memory" (memory 512))
      (import "env" "invoke_vi" (func $invoke (param i32 i32)))
      (import "env" "___cxa_throw" (func $throw (param i32 i32 i32)))
      (import "env" "___cxa_find_matching_catch_3" (func $find (param i32) (result i32)))
      (import "env" "___cxa_begin_catch" (func $begin (param i32) (result i32)))
      (import "env" "___cxa_end_catch" (func $end))
      (global $stack (mut i32) (i32.const 1024))
      (func (export "stackSave") (result i32) global.get $stack)
      (func (export "stackRestore") (param i32) local.get 0 global.set $stack)
      (func (export "setThrew") (param i32 i32) i32.const 100 local.get 0 i32.store)
      (func (export "setTempRet0") (param i32) i32.const 104 local.get 0 i32.store)
      (func (export "_malloc") (param i32) (result i32) i32.const 256)
      (func (export "_free") (param i32) i32.const 80 local.get 0 i32.store)
      (func (export "___cxa_is_pointer_type") (param i32) (result i32) i32.const 0)
      (func (export "___cxa_can_catch") (param i32 i32 i32) (result i32) local.get 0 local.get 1 i32.eq)
      (func (export "dynCall_vi") (param i32 i32)
        local.get 0 i32.const 9 i32.eq
        if i32.const 88 local.get 1 i32.store
        else i32.const 2048 global.set $stack i32.const 128 i32.const 7 i32.const 9 call $throw end)
      (func (export "raise") i32.const 1 i32.const 42 call $invoke)
      (func (export "catch") (result i32) i32.const 7 call $find call $begin)
      (func (export "end") call $end))"#;
    #[allow(unused_mut)]let mut cases=vec![(fixture(source),false)];
    #[cfg(all(feature="aot-builder",feature="guest-eh-prototype"))]{
        let source=source.replace("(global $stack (mut i32)","(global $stack (export \"__raven_stack_pointer_v1\") (mut i32)");
        let (bytes,_)=guest_eh_transform::transform(&wat::parse_str(source).unwrap()).unwrap();
        let engine=Engine::new(&engine_config_profile(true)).unwrap();let module=Module::new(&engine,bytes).unwrap();
        cases.push((initialize(&engine,&module).unwrap(),true));
    }
    for ((mut store,instance),guest_eh) in cases{
    instance.get_typed_func::<(),()>(&mut store,"raise").unwrap().call(&mut store,()).unwrap();
    assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"stackSave").unwrap().call(&mut store,()).unwrap(),1024);
    let memory=store.data().memory.unwrap();assert_eq!(&memory.data(&store)[100..104],&1i32.to_le_bytes());
    assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"catch").unwrap().call(&mut store,()).unwrap(),128);
    assert_eq!(&memory.data(&store)[104..108],&7i32.to_le_bytes());
    instance.get_typed_func::<(),()>(&mut store,"end").unwrap().call(&mut store,()).unwrap();
    assert_eq!(&memory.data(&store)[80..84],&128i32.to_le_bytes());assert_eq!(&memory.data(&store)[88..92],&128i32.to_le_bytes());
    assert_eq!(&memory.data(&store)[100..104],&0i32.to_le_bytes());
    instance.get_typed_func::<i32,()>(&mut store,"stackRestore").unwrap().call(&mut store,4096).unwrap();
    instance.get_typed_func::<(),()>(&mut store,"raise").unwrap().call(&mut store,()).unwrap();
    assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"stackSave").unwrap().call(&mut store,()).unwrap(),4096);
    if guest_eh{assert_eq!(store.data().calls.iter().find(|(name,_)|name=="env.invoke_vi").unwrap().1,0);}
    }
}
#[test]fn exported_stack_global_preserves_changed_stack_and_cpp_unwinding(){
    let (mut store,instance)=fixture(r#"(module
      (import "env" "memory" (memory 512))
      (import "env" "invoke_vi" (func $invoke (param i32 i32)))
      (import "env" "___cxa_throw" (func $throw (param i32 i32 i32)))
      (import "env" "___cxa_find_matching_catch_3" (func $find (param i32) (result i32)))
      (import "env" "___cxa_begin_catch" (func $begin (param i32) (result i32)))
      (import "env" "___cxa_end_catch" (func $end))
      (global $stack (export "__raven_stack_pointer_v1") (mut i32) (i32.const 1024))
      (func (export "stackSave") (result i32) global.get $stack)
      (func (export "stackRestore") (param i32) local.get 0 global.set $stack)
      (func (export "setThrew") (param i32 i32) i32.const 100 local.get 0 i32.store)
      (func (export "setTempRet0") (param i32) i32.const 104 local.get 0 i32.store)
      (func (export "_malloc") (param i32) (result i32) i32.const 256)
      (func (export "_free") (param i32) i32.const 80 local.get 0 i32.store)
      (func (export "___cxa_is_pointer_type") (param i32) (result i32) i32.const 0)
      (func (export "___cxa_can_catch") (param i32 i32 i32) (result i32) local.get 0 local.get 1 i32.eq)
      (func (export "dynCall_vi") (param i32 i32)
        local.get 0 i32.const 9 i32.eq
        if i32.const 88 local.get 1 i32.store
        else i32.const 2048 global.set $stack i32.const 128 i32.const 7 i32.const 9 call $throw end)
      (func (export "raise") i32.const 1 i32.const 42 call $invoke)
      (func (export "catch") (result i32) i32.const 7 call $find call $begin)
      (func (export "end") call $end))"#);
    instance.get_typed_func::<(),()>(&mut store,"raise").unwrap().call(&mut store,()).unwrap();
    assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"stackSave").unwrap().call(&mut store,()).unwrap(),1024);
    let memory=store.data().memory.unwrap();assert_eq!(&memory.data(&store)[100..104],&1i32.to_le_bytes());
    assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"catch").unwrap().call(&mut store,()).unwrap(),128);
    assert_eq!(&memory.data(&store)[104..108],&7i32.to_le_bytes());
    instance.get_typed_func::<(),()>(&mut store,"end").unwrap().call(&mut store,()).unwrap();
    assert_eq!(&memory.data(&store)[80..84],&128i32.to_le_bytes());assert_eq!(&memory.data(&store)[88..92],&128i32.to_le_bytes());
    assert_eq!(&memory.data(&store)[100..104],&0i32.to_le_bytes());
    instance.get_typed_func::<i32,()>(&mut store,"stackRestore").unwrap().call(&mut store,4096).unwrap();
    instance.get_typed_func::<(),()>(&mut store,"raise").unwrap().call(&mut store,()).unwrap();
    assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"stackSave").unwrap().call(&mut store,()).unwrap(),4096);
}
#[test]fn main_loop_unwind_and_cancellation_execute_real_guest_callbacks(){
    let (mut store,instance)=fixture(r#"(module
      (import "env" "memory" (memory 512))
      (import "env" "_emscripten_set_main_loop" (func $set (param i32 i32 i32)))
      (import "env" "_emscripten_cancel_main_loop" (func $cancel))
      (func (export "start") i32.const 7 i32.const 1000 i32.const 1 call $set unreachable)
      (func (export "dynCall_v") (param i32)
        local.get 0 i32.const 7 i32.ne if unreachable end
        i32.const 64 i32.const 64 i32.load i32.const 1 i32.add i32.store
        i32.const 64 i32.load i32.const 3 i32.eq if call $cancel end))"#);
    let error=instance.get_typed_func::<(),()>(&mut store,"start").unwrap().call(&mut store,()).unwrap_err();
    assert!(error.downcast_ref::<scheduler::MainLoopYield>().is_some());
    scheduler::run(&mut store,&instance,10,100000,&diagnostics::Progress::disabled(),false,None,&[]).unwrap();
    assert_eq!(store.data().scheduler.frames,3);assert!(!scheduler::active(store.data()));
    assert_eq!(&store.data().memory.unwrap().data(&store)[64..68],&3i32.to_le_bytes());
}
#[test]fn disabled_audio_returns_failure_and_never_claims_a_loaded_clip(){
    let (mut store,instance)=fixture(r#"(module
      (import "env" "memory" (memory 512))
      (import "env" "_JS_Sound_Init" (func $init))
      (import "env" "_JS_Sound_Load" (func $load (param i32 i32) (result i32)))
      (import "env" "_JS_Sound_GetLoadState" (func $state (param i32) (result i32)))
      (import "env" "_JS_Sound_Set3D" (func $unguarded (param i32 i32)))
      (func (export "init") call $init)
      (func (export "load") (result i32) i32.const 0 i32.const 0 call $load)
      (func (export "state") (result i32) i32.const 0 call $state)
      (func (export "unguarded") i32.const 0 i32.const 1 call $unguarded))"#);
    let init=instance.get_typed_func::<(),()>(&mut store,"init").unwrap();assert!(init.call(&mut store,()).is_err());
    store.data_mut().headless_audio=true;init.call(&mut store,()).unwrap();
    assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"load").unwrap().call(&mut store,()).unwrap(),0);
    assert_eq!(instance.get_typed_func::<(),i32>(&mut store,"state").unwrap().call(&mut store,()).unwrap(),2);
    assert!(instance.get_typed_func::<(),()>(&mut store,"unguarded").unwrap().call(&mut store,()).is_err());
    assert!(store.data().unavailable_services.contains("env._JS_Sound_Init"));
}

#[test]fn typed_float_invokes_preserve_mixed_argument_order_and_exception_nan(){
    for signature in ["f","fi","fii","fiii","fffi"]{
        let types=std::iter::once("i32").chain(signature[1..].chars().map(|c|if c=='f'{"f64"}else{"i32"})).collect::<Vec<_>>();
        let params=types.join(" ");let mut sum=String::from("f64.const 0");let mut args=String::new();let mut expected=0.;
        for (index,ty) in types.iter().enumerate(){
            let value=index as f64+0.5;let actual=if *ty=="i32"{value as i32 as f64}else{value};
            args+=&format!(" {ty}.const {actual}");sum+=&format!(" local.get {index} {} f64.const {} f64.mul f64.add",if *ty=="i32"{"f64.convert_i32_s"}else{""},index+1);expected+=actual*(index+1) as f64;
        }
        let source=format!(r#"(module (import "env" "invoke_{signature}" (func $invoke (param {params}) (result f64)))
            (func (export "dynCall_{signature}") (param {params}) (result f64) {sum})
            (func (export "run") (result f64) {args} call $invoke))"#);
        let(mut store,instance)=fixture(&source);let run=instance.get_typed_func::<(),f64>(&mut store,"run").unwrap();
        for _ in 0..2{assert_eq!(run.call(&mut store,()).unwrap(),expected);}
    }
    let(mut store,instance)=fixture(r#"(module
        (import "env" "invoke_f" (func $invoke (param i32) (result f64)))
        (import "env" "___cxa_throw" (func $throw (param i32 i32 i32)))
        (global $stack (export "__raven_stack_pointer_v1") (mut i32) (i32.const 4096))
        (func (export "stackRestore") (param i32) local.get 0 global.set $stack)
        (func (export "setThrew") (param i32 i32))
        (func (export "dynCall_f") (param i32) (result f64)
            i32.const 1024 global.set $stack i32.const 128 i32.const 0 i32.const 0 call $throw f64.const 5)
        (func (export "run") (result f64) i32.const 0 call $invoke))"#);
    assert!(instance.get_typed_func::<(),f64>(&mut store,"run").unwrap().call(&mut store,()).unwrap().is_nan());
    assert_eq!(instance.get_global(&mut store,"__raven_stack_pointer_v1").unwrap().get(&mut store).i32(),Some(4096));
}

#[test]fn extra_typed_invokes_preserve_pinned_mixed_abi_and_i64_return_side_effects(){
    let inventory:serde_json::Value=serde_json::from_str(include_str!("../abi-inventory.json")).unwrap();
    let selected:serde_json::Value=serde_json::from_str(include_str!("../invoke-specializations.json")).unwrap();
    for signature in selected["signatures"].as_array().unwrap(){
        let signature=signature.as_str().unwrap();let name=format!("invoke_{signature}");
        let item=inventory["imports"].as_array().unwrap().iter().find(|v|v["name"]==name).unwrap();
        let types=item["params"].as_array().unwrap().iter().map(|v|v.as_str().unwrap()).collect::<Vec<_>>();
        let ret=item["results"].as_array().unwrap().first().map(|v|v.as_str().unwrap());
        let params=types.join(" ");let mut args=String::new();let mut sum="f64.const 0".to_string();let mut expected=0.;
        for(index,ty)in types.iter().enumerate(){
            let value=if *ty=="i32"{index as f64-3.}else{index as f64+0.25};args+=&format!(" {ty}.const {value}");
            sum+=&format!(" local.get {index} {} f64.const {} f64.mul f64.add",if *ty=="i32"{"f64.convert_i32_s"}else{""},index+1);expected+=value*(index+1) as f64;
        }
        let returns=ret.map(|ty|format!("(result {ty})")).unwrap_or_default();
        let body=match ret{Some("i32")=>format!("i32.const 88 i32.const -1234567 i32.store {sum} i32.trunc_f64_s"),Some("f64")=>sum,_=>format!("i32.const 64 {sum} f64.store")};
        let after=match ret{Some("i32")=>"f64.convert_i32_s",Some("f64")=>"",_=>"i32.const 64 f64.load"};
        let source=format!(r#"(module (import "env" "memory" (memory 512))
            (import "env" "{name}" (func $invoke (param {params}) {returns}))
            (func (export "dynCall_{signature}") (param {params}) {returns} {body})
            (func (export "run") (result f64) {args} call $invoke {after}))"#);
        let(mut store,instance)=fixture(&source);let run=instance.get_typed_func::<(),f64>(&mut store,"run").unwrap();
        for _ in 0..2{assert_eq!(run.call(&mut store,()).unwrap(),if ret==Some("i32"){expected as i32 as f64}else{expected},"{signature}");}
        if signature.starts_with('j') {let memory=store.data().memory.unwrap();assert_eq!(&memory.data(&store)[88..92],&(-1234567i32).to_le_bytes());}
    }
}

#[test]fn shared_invoke_handles_survive_recursive_and_cold_other_signature_reentry(){
    let(mut store,instance)=fixture(r#"(module (import "env" "memory" (memory 512))
      (import "env" "invoke_ifi" (func $recurse (param i32 f64 i32) (result i32)))
      (import "env" "invoke_vif" (func $other (param i32 i32 f64)))
      (global (export "__raven_stack_pointer_v1") (mut i32) (i32.const 4096))
      (func (export "dynCall_vif") (param i32 i32 f64) i32.const 64 local.get 1 i32.store)
      (func (export "dynCall_ifi") (param i32 f64 i32) (result i32)
        local.get 2 i32.eqz
        if (result i32)
          i32.const 0 i32.const 123 f64.const 7.5 call $other
          local.get 1 i32.trunc_f64_s
        else
          i32.const 0 local.get 1 f64.const 0.25 f64.add local.get 2 i32.const 1 i32.sub call $recurse
          i32.const 1 i32.add
        end)
      (func (export "run") (result i32) i32.const 0 f64.const 1.25 i32.const 12 call $recurse))"#);
    let run=instance.get_typed_func::<(),i32>(&mut store,"run").unwrap();
    for _ in 0..3{assert_eq!(run.call(&mut store,()).unwrap(),16);}
    let memory=store.data().memory.unwrap();assert_eq!(&memory.data(&store)[64..68],&123i32.to_le_bytes());
    for handle in store.data().exceptions.invokes.iter().flatten(){assert_eq!(std::rc::Rc::strong_count(handle),1);}
}
