(function () {
  'use strict';
  const $=id=>document.getElementById(id);
  const canvas=$('unity-canvas'), input=new RavenInput(canvas);
  let instance, ready=false, started=false, hostPaused=false, panelOpen=false;
  let virtualLock=false, controlMode='auto', renderHeight=720, startAt=0, loadedAt=0;
  let frameSamples=[], lastFrame=0, lastStats=0, frameCount=0;
  const errors=[];
  const touchMode=!new URLSearchParams(location.search).has('desktop');

  function setLock(locked) {
    if (virtualLock===locked) return;
    virtualLock=locked;
    document.dispatchEvent(new Event('pointerlockchange'));
    updateControls();
  }
  // Android touch has no physical mouse to capture. Preserve Unity's lock state
  // while supplying relative movement through the existing Emscripten callbacks.
  if (touchMode) {
    for (const name of ['pointerLockElement','mozPointerLockElement','webkitPointerLockElement']) {
      Object.defineProperty(document,name,{configurable:true,get:()=>virtualLock?canvas:null});
    }
    canvas.requestPointerLock=()=>setLock(true);
    document.exitPointerLock=()=>setLock(false);
    if (!document.body.requestPointerLock) document.body.requestPointerLock=()=>setLock(true);
  }
  function releaseInput() {
    input.release();
    document.querySelectorAll('.held').forEach(el=>el.classList.remove('held'));
    $('stick').style.transform='';
    joystickPointer=null; lookPointer=null;
  }
  function updateControls() {
    const visible=ready && !hostPaused && !panelOpen && (controlMode==='on'||controlMode==='auto'&&virtualLock);
    $('controls').hidden=!visible;
    $('controls-button').textContent='Touch: '+({auto:'otomatik',on:'açık',off:'kapalı'}[controlMode]);
    if (!visible) releaseInput();
  }
  function applyPause() {
    releaseInput();
    if (instance?.Module.ravenPause) instance.Module.ravenPause(hostPaused||panelOpen);
    $('background').hidden=!hostPaused||panelOpen||!ready;
    updateControls();
  }
  function togglePanel() {
    if (!ready) return;
    panelOpen=!panelOpen; $('panel').hidden=!panelOpen; applyPause();
  }
  function fail(error) {
    const message=String(error?.message||error);
    errors.push(message); console.error('[RavenPort]',message);
    $('launch').hidden=false; $('error').hidden=false; $('error').textContent=message;
    $('status').textContent='Yükleme tamamlanamadı';
  }
  function resize() {
    const aspect=innerWidth/innerHeight;
    canvas.height=renderHeight; canvas.width=Math.round(renderHeight*aspect);
  }
  function diagnostics() {
    const engine=instance?.Module.ravenStats?.()||{};
    const sorted=frameSamples.slice().sort((a,b)=>a-b);
    return {ready,loadingMs:loadedAt?Math.round(loadedAt-startAt):null,
      canvas:[canvas.width,canvas.height],virtualLock,controlMode,hostPaused,panelOpen,
      ...engine,frameSamples:sorted.length,rafP95Ms:sorted.length?Math.round(sorted[Math.floor(sorted.length*.95)]):null,
      inputEvents:input.eventCount,errors:errors.slice(-8)};
  }
  function tick(now) {
    if (ready&&!hostPaused&&!panelOpen) {
      input.flushLook(); frameCount++;
      if (lastFrame) {frameSamples.push(now-lastFrame);if(frameSamples.length>180)frameSamples.shift();}
    }
    lastFrame=now;
    if (ready&&now-lastStats>1000) {
      const d=diagnostics();
      $('metrics').textContent=`Heap ${Math.round((d.heapBytes||0)/1048576)} MiB · frame p95 ${d.rafP95Ms||'—'} ms`;
      if (panelOpen) $('diagnostics').textContent=JSON.stringify(d,null,2);
      lastStats=now;
    }
    requestAnimationFrame(tick);
  }

  async function start() {
    if (started) return; started=true; startAt=performance.now();
    console.log('[RavenPort] unity-start-requested');
    $('start').disabled=true; $('resolution').disabled=true; $('progress').hidden=false;
    renderHeight=Number($('resolution').value); resize(); canvas.focus();
    try {
      await new Promise((resolve,reject)=>{const script=document.createElement('script');script.src='/game/loader.js';script.onload=resolve;script.onerror=()=>reject(new Error('Unity loader bulunamadı'));document.head.appendChild(script);});
      instance=await createUnityInstance(canvas,{
        dataUrl:'/game/game.data',frameworkUrl:'/game/framework.js',codeUrl:'/game/game.wasm',
        streamingAssetsUrl:'/game/StreamingAssets',companyName:'SteelRaven7',productName:'RavenfieldSteam',productVersion:'1.0',
        cacheControl:()=> 'no-store',devicePixelRatio:1,matchWebGLToCanvasSize:false,ravenTouch:touchMode,
        showBanner:(message,type)=>{if(type==='error')fail(message);else console.warn(message);},
        print:message=>console.log('[Unity]',message),printErr:message=>console.error('[Unity]',message),
        errorHandler:message=>{fail(message);return true;}
      },progress=>{$('bar').value=progress;$('status').textContent=progress<.9?`Yerel dosyalar yükleniyor · %${Math.round(progress*100)}`:'Unity başlatılıyor…';});
      window.unityInstance=instance; ready=true; loadedAt=performance.now();
      $('launch').hidden=true;$('toolbar').hidden=false;resize();applyPause();canvas.focus();
      console.log('[RavenPort] engine-ready',JSON.stringify(diagnostics()));
    } catch(error) {fail(error);}
  }

  let joystickPointer=null, lookPointer=null, lookX=0, lookY=0;
  function joystickMove(event) {
    const rect=$('joystick').getBoundingClientRect();
    let x=(event.clientX-rect.left-rect.width/2)/(rect.width*.38),y=(event.clientY-rect.top-rect.height/2)/(rect.height*.38);
    const length=Math.hypot(x,y);if(length>1){x/=length;y/=length;}
    const keys=[];if(x<-.28)keys.push('KeyA');if(x>.28)keys.push('KeyD');if(y<-.28)keys.push('KeyW');if(y>.28)keys.push('KeyS');
    input.setSource('joystick',keys);$('stick').style.transform=`translate(${x*rect.width*.28}px,${y*rect.height*.28}px)`;
  }
  $('joystick').addEventListener('pointerdown',event=>{
    if(joystickPointer!==null)return;event.preventDefault();joystickPointer=event.pointerId;
    event.currentTarget.setPointerCapture(event.pointerId);joystickMove(event);
  });
  $('joystick').addEventListener('pointermove',event=>{if(event.pointerId===joystickPointer)joystickMove(event);});
  for(const type of ['pointerup','pointercancel','lostpointercapture'])$('joystick').addEventListener(type,event=>{
    if(event.pointerId!==joystickPointer)return;joystickPointer=null;input.removeSource('joystick');$('stick').style.transform='';
  });
  $('look').addEventListener('pointerdown',event=>{
    if(lookPointer!==null)return;event.preventDefault();lookPointer=event.pointerId;lookX=event.clientX;lookY=event.clientY;
    event.currentTarget.setPointerCapture(event.pointerId);
  });
  $('look').addEventListener('pointermove',event=>{
    if(event.pointerId!==lookPointer)return;
    input.look(event.clientX-lookX,event.clientY-lookY);lookX=event.clientX;lookY=event.clientY;
  });
  for(const type of ['pointerup','pointercancel','lostpointercapture'])$('look').addEventListener(type,event=>{if(event.pointerId===lookPointer)lookPointer=null;});
  document.querySelectorAll('[data-key],[data-mouse]').forEach(button=>{
    button.addEventListener('pointerdown',event=>{
      event.preventDefault();button.setPointerCapture(event.pointerId);button.classList.add('held');
      input.setSource(event.pointerId,button.dataset.key?[button.dataset.key]:[],button.dataset.mouse!==undefined?[Number(button.dataset.mouse)]:[]);
    });
    for(const type of ['pointerup','pointercancel','lostpointercapture'])button.addEventListener(type,event=>{button.classList.remove('held');input.removeSource(event.pointerId);});
  });
  $('start').onclick=start;
  $('panel-button').onclick=togglePanel;$('resume').onclick=togglePanel;
  $('keyboard-button').onclick=()=>{$('keyboard').hidden=!$('keyboard').hidden;};
  $('controls-button').onclick=()=>{releaseInput();controlMode=({auto:'on',on:'off',off:'auto'})[controlMode];updateControls();};
  $('game-menu').onclick=()=>{togglePanel();controlMode='auto';input.tap('Escape');setLock(false);};
  $('restart').onclick=()=>{if(confirm('Oyun yeniden yüklensin mi? Kaydedilmemiş ilerleme kaybolabilir.'))location.reload();};
  try{$('sensitivity').value=localStorage.getItem('raven.sensitivity')||'1';}catch(_){}
  function sensitivity(){input.sensitivity=Number($('sensitivity').value);$('sensitivity-value').value=input.sensitivity.toFixed(2);try{localStorage.setItem('raven.sensitivity',String(input.sensitivity));}catch(_){}}
  $('sensitivity').oninput=sensitivity;sensitivity();
  window.addEventListener('resize',resize);
  window.addEventListener('blur',releaseInput);
  document.addEventListener('visibilitychange',()=>{if(document.hidden){releaseInput();instance?.Module.ravenPause?.(true);}else applyPause();});
  window.addEventListener('error',event=>{errors.push(String(event.message));});
  window.addEventListener('unhandledrejection',event=>{errors.push(String(event.reason));});
  canvas.addEventListener('contextmenu',event=>event.preventDefault());
  window.RavenPort={start,releaseInput,togglePanel,diagnostics,input,
    setHostPaused(paused){hostPaused=!!paused;applyPause();},
    setRenderHeight(height){if(Number.isInteger(height)&&height>=360&&height<=1080){renderHeight=height;resize();}},
    setControls(mode){if(['auto','on','off'].includes(mode)){controlMode=mode;updateControls();}}
  };
  resize();requestAnimationFrame(tick);
  console.log('[RavenPort] launcher-ready');
})();
