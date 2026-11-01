// Inserted inside the reviewed Unity framework closure, immediately before run().
// No WASM instructions or game assets are altered.
var ravenPaused = false;
Module.ravenPause = function (paused) {
    paused = !!paused;
    if (ravenPaused !== paused) {
        ravenPaused = paused;
        if (paused) Browser.mainLoop.pause();
        else Browser.mainLoop.resume();
    }
    var audio = WEBAudio.audioContext;
    if (audio && audio.state !== 'closed') {
        var task = paused ? audio.suspend() : audio.resume();
        if (task && task.catch) task.catch(function (error) { console.warn('Audio lifecycle:', error); });
    }
    if (paused) fs.sync(false);
};
Module.ravenStats = function () {
    return {
        heapBytes: HEAPU8.byteLength,
        paused: ravenPaused,
        audioState: WEBAudio.audioContext ? WEBAudio.audioContext.state : 'uninitialized',
        audioSources: Object.keys(WEBAudio.audioInstances).length,
        pointerLocked: Browser.pointerLock,
        usingWasm: !!Module.usingWasm,
        frameNumber: Browser.mainLoop.currentFrameNumber,
        registeredEvents: JSEvents.eventHandlers.map(function (e) { return e.eventTypeString; })
    };
};
