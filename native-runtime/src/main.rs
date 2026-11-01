fn main() {
    let mut args=std::env::args().skip(1);
    let Some(first) = args.next() else {
        eprintln!("Usage: raven-native-probe [--trusted-artifact|--trusted-smoke] local-file [framework.js] [--data UnityWebData] [--headless-events] [--headless-audio] [--save-state snapshot] [--desktop-gles] [--frames N] [--startup-fuel N] [--frame-fuel N] [--capture frame.ppm]");
        std::process::exit(2);
    };
    if first == "--trusted-smoke" {
        let path = args.next().expect("Locally produced smoke artifact required");
        let result = unsafe { ravenfield_native_host::smoke_trusted_artifact(&path) };
        match result {
            Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
            Err(error) => { eprintln!("Native smoke failed: {error:#}"); std::process::exit(1); }
        }
        return;
    }
    let compiled=first=="--trusted-artifact";
    let path=if compiled {args.next().expect("Trusted locally compiled artifact path required")} else {first};
    let mut framework=None;let mut data=None;let mut options=ravenfield_native_host::ProbeOptions::default();
    while let Some(argument)=args.next(){
        if argument=="--data" {data=Some(args.next().expect("UnityWebData path required"));}
        else if argument=="--headless-events" {options.headless_events=true;}
        else if argument=="--audio-wav" {options.audio_wav=Some(args.next().expect("Audio output WAV required").into());}
        else if argument=="--headless-audio" {options.headless_audio=true;}
        else if argument=="--save-state" {options.save_state=Some(args.next().expect("Save snapshot path required").into());}
        else if argument=="--desktop-gles" {options.desktop_gles=true;}
        else if argument=="--android-gles" {options.android_gles=true;}
        else if argument=="--guest-eh" {options.guest_eh=true;}
        else if argument=="--epoch-guard" {options.epoch_guard=true;}
        else if argument=="--inline-eager-codegen" {options.inline_eager_codegen=true;}
        else if argument=="--native-audio" {assert!(cfg!(target_os="android"),"Native audio diagnostic requires Android");options.native_audio=true;}
        else if argument=="--desktop-keyboard" {options.desktop_keyboard=true;}
        else if argument=="--space-at-frame" {options.space_at_frame=Some(args.next().expect("Space frame required").parse().expect("Integer frame required"));}
        else if argument=="--tap-at-frame" {options.tap_at_frame=Some(args.next().expect("Tap frame required").parse().expect("Integer frame required"));}
        else if argument=="--tap-x" {options.tap_x=args.next().expect("Tap x required").parse().expect("Integer x required");}
        else if argument=="--tap-y" {options.tap_y=args.next().expect("Tap y required").parse().expect("Integer y required");}
        else if argument=="--tap" {let tap=args.next().expect("Tap frame,x,y required").split(',').map(|value|value.parse::<i64>().expect("Integer tap components required")).collect::<Vec<_>>();assert_eq!(tap.len(),3,"Tap requires frame,x,y");options.tap_sequence.push((tap[0].try_into().expect("Invalid frame"),tap[1].try_into().expect("Invalid x"),tap[2].try_into().expect("Invalid y")));}
        else if argument=="--game-input"{let values=args.next().expect("Game event frame,kind,a,b required").split(',').map(|v|v.parse::<i64>().expect("Integer game input required")).collect::<Vec<_>>();assert_eq!(values.len(),4);options.game_sequence.push((values[0].try_into().expect("Invalid frame"),values[1].try_into().expect("Invalid kind"),values[2].try_into().expect("Invalid input"),values[3].try_into().expect("Invalid input")));}
        else if argument=="--trace-guest-lists" {options.trace_guest_lists=true;}
        else if argument=="--profile-imports" {options.profile_imports=true;}
        else if argument=="--trace-gl-calls" {options.trace_gl_calls=true;}
        else if argument=="--frames" {options.frame_limit=args.next().expect("Frame limit required").parse().expect("Integer frame limit required");}
        else if argument=="--startup-fuel" {options.startup_fuel=Some(args.next().expect("Fuel budget required").parse().expect("Integer fuel budget required"));}
        else if argument=="--frame-fuel" {options.frame_fuel=Some(args.next().expect("Frame fuel budget required").parse().expect("Integer frame fuel budget required"));}
        else if argument=="--capture" {options.capture=Some(args.next().expect("Capture path required").into());}
        else if argument=="--capture-at"{let value=args.next().expect("Frame,path required");let(frame,path)=value.split_once(',').expect("Frame,path required");options.capture_frames.push((frame.parse().expect("Integer frame required"),path.into()));}
        else if framework.is_none(){framework=Some(std::fs::read_to_string(&argument).unwrap_or_else(|error|panic!("Read framework argument {argument:?}: {error}")));}
        else {panic!("Unexpected argument");}
    }
    let result=if compiled {
        // Explicit developer CLI option; do not use with downloaded artifacts.
        unsafe {ravenfield_native_host::probe_trusted_with_options(&path,framework.as_deref(),data.as_deref(),options)}
    } else {ravenfield_native_host::probe_source_with_options(&path,framework.as_deref(),data.as_deref(),options)};
    match result {
        Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
        Err(error) => { eprintln!("Native probe failed: {error:#}"); std::process::exit(1); }
    }
}
