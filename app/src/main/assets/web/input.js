(function () {
  'use strict';
  const keyCodes = {KeyW:87,KeyA:65,KeyS:83,KeyD:68,KeyR:82,KeyF:70,KeyC:67,KeyV:86,KeyM:77,Space:32,ShiftLeft:16,Escape:27,Backspace:8,Enter:13,Digit0:48,Digit1:49,Digit2:50,Digit3:51,Digit4:52,Digit5:53,Digit6:54,Digit7:55,Digit8:56,Digit9:57};
  const keyNames = {Space:' ',ShiftLeft:'Shift',Escape:'Escape',Backspace:'Backspace',Enter:'Enter'};

  class RavenInput {
    constructor(canvas) {
      this.canvas = canvas;
      this.sources = new Map();
      this.sentKeys = new Set();
      this.sentButtons = new Set();
      this.dx = 0; this.dy = 0; this.fractionX = 0; this.fractionY = 0;
      this.sensitivity = 1;
      this.mouseMask = 0;
      this.eventCount = 0;
    }
    setSource(id, keys = [], buttons = []) {
      this.sources.set(id, {keys: new Set(keys), buttons: new Set(buttons)});
      this.sync();
    }
    removeSource(id) { this.sources.delete(id); this.sync(); }
    key(code, down) {
      const keyCode = keyCodes[code];
      const key = keyNames[code] || code.replace('Key','').replace('Digit','').toLowerCase();
      const event = new KeyboardEvent(down ? 'keydown' : 'keyup', {key,code,location:code==='ShiftLeft'?1:0,bubbles:true,cancelable:true,keyCode,which:keyCode});
      // Legacy Unity reads keyCode/which rather than only KeyboardEvent.code.
      Object.defineProperties(event, {keyCode:{value:keyCode},which:{value:keyCode}});
      this.canvas.dispatchEvent(event); this.eventCount++;
      if (down && key.length === 1) {
        const charCode = key.charCodeAt(0);
        const press = new KeyboardEvent('keypress',{key,code,charCode,keyCode:charCode,which:charCode,bubbles:true,cancelable:true});
        Object.defineProperties(press,{charCode:{value:charCode},keyCode:{value:charCode},which:{value:charCode}});
        this.canvas.dispatchEvent(press); this.eventCount++;
      }
    }
    mouse(type, button = 0, dx = 0, dy = 0, x, y) {
      const rect = this.canvas.getBoundingClientRect();
      const clientX = x ?? rect.left + rect.width / 2;
      const clientY = y ?? rect.top + rect.height / 2;
      const event = new MouseEvent(type, {button,buttons:this.mouseMask,clientX,clientY,screenX:clientX,screenY:clientY,bubbles:true,cancelable:true});
      Object.defineProperties(event, {movementX:{value:dx},movementY:{value:dy}});
      this.canvas.dispatchEvent(event); this.eventCount++;
    }
    sync() {
      const keys = new Set(), buttons = new Set();
      for (const value of this.sources.values()) {
        value.keys.forEach(k=>keys.add(k)); value.buttons.forEach(b=>buttons.add(b));
      }
      for (const code of this.sentKeys) if (!keys.has(code)) this.key(code,false);
      for (const code of keys) if (!this.sentKeys.has(code)) this.key(code,true);
      for (const button of this.sentButtons) if (!buttons.has(button)) {
        this.mouseMask &= ~(button === 2 ? 2 : 1); this.mouse('mouseup',button);
      }
      for (const button of buttons) if (!this.sentButtons.has(button)) {
        this.mouseMask |= button === 2 ? 2 : 1; this.mouse('mousedown',button);
      }
      this.sentKeys=keys; this.sentButtons=buttons;
    }
    look(dx,dy) { this.dx+=dx*this.sensitivity; this.dy+=dy*this.sensitivity; }
    flushLook() {
      this.fractionX += this.dx; this.fractionY += this.dy; this.dx=0; this.dy=0;
      const x=Math.trunc(this.fractionX), y=Math.trunc(this.fractionY);
      this.fractionX-=x; this.fractionY-=y;
      if (x||y) this.mouse('mousemove',0,x,y);
    }
    release() {
      this.sources.clear(); this.sync(); this.dx=0; this.dy=0; this.fractionX=0; this.fractionY=0;
    }
    tap(code) {
      const id = Symbol(code); this.setSource(id,[code]);
      setTimeout(()=>this.removeSource(id),100);
    }
  }
  window.RavenInput=RavenInput;
})();
