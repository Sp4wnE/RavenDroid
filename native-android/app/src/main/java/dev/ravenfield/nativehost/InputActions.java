package dev.ravenfield.nativehost;

/** Guest ABI actions shared by touch editing and physical input mapping. */
final class InputActions {
    static final int NONE=-1, MODE=2000, PAUSE=2001, PREVIOUS=2002, NEXT=2003;
    static final int[] VALUES={NONE,1000,1002,87,83,65,68,32,17,16,82,70,13,77,88,67,81,69,49,50,51,52,53,112,113,114,115,116,117,118,119,120,121,122,MODE,PAUSE,PREVIOUS,NEXT};
    static final String[] LABELS={"—","Fire","Aim","W · Forward","S · Back","A · Left","D · Right","Space · Jump","Ctrl · Crouch","Shift · Sprint","R · Reload","F · Use / Exit","Enter · Loadout","M · Map","X · Prone / Countermeasures","C · Camera","Q","E","1","2","3","4","5","F1 · Seat 1","F2 · Seat 2","F3 · Seat 3","F4 · Seat 4","F5 · Seat 5","F6 · Seat 6","F7 · Seat 7","F8","F9","F10","F11","Menu / Gameplay","Pause","Previous weapon","Next weapon"};
    static boolean valid(int action,boolean commands){
        if(action==NONE)return true;
        if(action>=65&&action<=90||action>=48&&action<=57||action>=112&&action<=122)return true;
        if(action==8||action==9||action==13||action>=16&&action<=18||action==27||action==32||action>=37&&action<=40||action==1000||action==1002)return true;
        return commands&&action>=MODE&&action<=NEXT;
    }
    static String label(int action){for(int i=0;i<VALUES.length;i++)if(VALUES[i]==action)return LABELS[i];return "Key "+action;}
    static int[] touchValues(){
        java.util.LinkedHashSet<Integer> out=new java.util.LinkedHashSet<>();
        for(int value:VALUES)if(value>=0&&value<MODE)out.add(value);
        for(int value=65;value<=90;value++)out.add(value);
        for(int value=48;value<=57;value++)out.add(value);
        for(int value:new int[]{8,9,18,27,37,38,39,40})out.add(value);
        return out.stream().mapToInt(Integer::intValue).toArray();
    }
}
