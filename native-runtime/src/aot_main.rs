use anyhow::{Context, Result};

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(args.len()>=3,"Usage: raven-aot <target> <source.wasm> <output.cwasm> [--opt-level none|speed] [--guest-eh] [--parallel] [--inline-eager-codegen] [--epoch-guard]");
    let(mut optimized,mut guest_eh,mut parallel,mut inline_eager,mut epoch_guard)=(false,false,false,false,false);let mut extra=args[3..].iter();
    while let Some(option)=extra.next(){match option.as_str(){
        "--opt-level"=>optimized=match extra.next().map(String::as_str){Some("none")=>false,Some("speed")=>true,_=>anyhow::bail!("Expected --opt-level none|speed")},
        "--guest-eh"=>guest_eh=true,"--parallel"=>parallel=true,"--inline-eager-codegen"=>inline_eager=true,"--epoch-guard"=>epoch_guard=true,other=>anyhow::bail!("Unknown option: {other}"),
    }}
    if parallel{let threads=std::env::var("RAYON_NUM_THREADS").ok().and_then(|n|n.parse::<u32>().ok());anyhow::ensure!(threads==Some(2),"Parallel AOT requires RAYON_NUM_THREADS=2 to bound memory");}
    ravenfield_native_host::precompile_for_target_guarded(&args[1],&args[0],&args[2],optimized,guest_eh,parallel,inline_eager,epoch_guard)
        .context("AOT compilation failed")?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
