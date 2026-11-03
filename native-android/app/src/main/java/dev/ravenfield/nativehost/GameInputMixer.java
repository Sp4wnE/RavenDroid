package dev.ravenfield.nativehost;

import java.util.HashMap;
import java.util.Map;

/** Touch and physical devices may hold the same action independently. UI thread only. */
final class GameInputMixer {
    private final GameControls.Sink output;
    private final Map<Integer, Map<Integer, Boolean>> sources = new HashMap<>();
    GameInputMixer(GameControls.Sink output) { this.output = output; }
    GameControls.Sink source(int id) { return (kind,a,b) -> send(id,kind,a,b); }
    private boolean forward(int kind,int a,int b) {
        if(output.send(kind,a,b))return true;
        sources.clear();output.send(3,0,0);return false;
    }
    private boolean held(int action) {
        for (Map<Integer,Boolean> source : sources.values()) if (source.containsKey(action)) return true;
        return false;
    }
    private boolean send(int id,int kind,int a,int b) {
        Map<Integer,Boolean> source=sources.computeIfAbsent(id,ignored->new HashMap<>());
        if (kind==3) {
            boolean ok=true;
            for (int action : new java.util.ArrayList<>(source.keySet())) {
                source.remove(action);
                if (!held(action)) ok &= forward(action>=1000?2:0,action>=1000?action-1000:action,0);
            }
            if (sources.values().stream().allMatch(Map::isEmpty)) ok &= forward(3,0,0);
            return ok;
        }
        if (kind!=0 && kind!=2) return forward(kind,a,b);
        int action=kind==2?1000+a:a;
        boolean before=held(action);
        if (b!=0) source.put(action,true); else source.remove(action);
        return before==held(action) || forward(kind,a,held(action)?1:0);
    }
    void reset() { sources.clear(); output.send(3,0,0); }
}
