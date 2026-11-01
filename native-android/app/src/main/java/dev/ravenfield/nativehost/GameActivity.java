package dev.ravenfield.nativehost;
import android.app.Activity;
import android.os.*;
import android.graphics.Color;
import android.view.*;
import android.widget.*;
import java.io.*;
import org.json.JSONObject;

/** EGL and the VM stay on one worker; UI only sends bounded control messages. */
public final class GameActivity extends Activity implements SurfaceHolder.Callback {
    private android.media.AudioManager audioManager;private android.media.AudioFocusRequest audioFocus;private boolean audioGranted;
    private static native void audioMutedNative(long id,boolean muted);
    private void audioFocus(boolean gain){audioGranted=gain;if(session!=0)audioMutedNative(session,!gain);}
    private static native long createSessionNative();
    private static native void stopSessionNative(long id);
    private static native void releaseSessionNative(long id);
    private static native boolean pointerNative(long id,int kind,int x,int y);
    private static native String statusNative(long id);
    private static native boolean gameInputNative(long id,int kind,int a,int b);
    private static native String runLiveNative(long id,Surface surface,String artifact,String log,String data);
    private final Handler handler=new Handler(Looper.getMainLooper());
    private final TouchRouter touch=new TouchRouter();
    private SurfaceView surface;private TextView status;private GameControlView controls;
    private long session;private boolean running,surfaceReady,wantsRun,runtimeReady,exiting;
    private long firstVisible;
    private final Runnable poll=new Runnable(){@Override public void run(){
        if(!wantsRun||isFinishing()||isDestroyed())return;
        if(!running)startIfReady();
        if(session!=0)try{
            JSONObject state=new JSONObject(statusNative(session));long frames=state.optLong("presented_frames");
            if(frames==0)status.setText(GameActivity.this.getString(R.string.ui_preparing_the_game_the_first_launch_can_ta_9d11b));
            else if(state.optLong("last_frame_age_ms")>2500)status.setText(GameActivity.this.getString(R.string.ui_loading_game_content_b6921));
            else status.setText("");
            status.setVisibility(status.length()==0?View.GONE:View.VISIBLE);
        }catch(Exception ignored){status.setText(GameActivity.this.getString(R.string.ui_could_not_read_session_status_901cc));}
        handler.postDelayed(this,500);
    }};
    @Override public void onCreate(Bundle state){
        super.onCreate(state);setVolumeControlStream(android.media.AudioManager.STREAM_MUSIC);
        audioManager=(android.media.AudioManager)getSystemService(AUDIO_SERVICE);audioFocus=new android.media.AudioFocusRequest.Builder(android.media.AudioManager.AUDIOFOCUS_GAIN).setAudioAttributes(new android.media.AudioAttributes.Builder().setUsage(android.media.AudioAttributes.USAGE_GAME).setContentType(android.media.AudioAttributes.CONTENT_TYPE_MUSIC).build()).setOnAudioFocusChangeListener(change->audioFocus(change==android.media.AudioManager.AUDIOFOCUS_GAIN)).build();getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_FULLSCREEN|View.SYSTEM_UI_FLAG_HIDE_NAVIGATION|View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY|View.SYSTEM_UI_FLAG_LAYOUT_STABLE|View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN|View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
        FrameLayout root=new FrameLayout(this);root.setBackgroundColor(Color.BLACK);setContentView(root);
        root.post(this::hideBars);
        surface=new SurfaceView(this){@Override protected void onMeasure(int width,int height){int[] size=TouchRouter.fit(MeasureSpec.getSize(width),MeasureSpec.getSize(height));setMeasuredDimension(size[0],size[1]);}};
        root.addView(surface,new FrameLayout.LayoutParams(-2,-2,Gravity.CENTER));surface.getHolder().setFixedSize(1280,720);surface.getHolder().addCallback(this);
        surface.setOnTouchListener((view,event)->{
            if(controls!=null&&controls.playing()){touch.cancel();return true;}
            TouchRouter.Event pointer=null;int action=event.getActionMasked(),index=event.getActionIndex();
            if(action==android.view.MotionEvent.ACTION_DOWN)pointer=touch.down(event.getPointerId(index),event.getX(index),event.getY(index),view.getWidth(),view.getHeight());
            else if(action==android.view.MotionEvent.ACTION_MOVE){index=event.findPointerIndex(touch.active());if(index>=0)pointer=touch.move(touch.active(),event.getX(index),event.getY(index),view.getWidth(),view.getHeight());else pointer=touch.cancel();}
            else if(action==android.view.MotionEvent.ACTION_UP||action==android.view.MotionEvent.ACTION_POINTER_UP)pointer=touch.up(event.getPointerId(index),event.getX(index),event.getY(index),view.getWidth(),view.getHeight());
            else if(action==android.view.MotionEvent.ACTION_CANCEL)pointer=touch.cancel();
            if(pointer!=null&&session!=0)pointerNative(session,pointer.kind,pointer.x,pointer.y);
            return true;
        });
        status=new TextView(this);status.setTextColor(Color.WHITE);status.setBackgroundColor(0x88000000);status.setPadding(16,8,16,8);root.addView(status,new FrameLayout.LayoutParams(-2,-2,Gravity.CENTER));
        controls=new GameControlView(this,(kind,a,b)->{if(kind==3)touch.cancel();return session!=0&&gameInputNative(session,kind,a,b);},this::showPause);
        root.addView(controls,new FrameLayout.LayoutParams(-2,-2,Gravity.CENTER));
        try{System.loadLibrary("ravenfield_native_host");runtimeReady=true;}catch(LinkageError error){status.setText(error.toString());wantsRun=false;finish();}
    }
    private void escape(){if(session!=0){gameInputNative(session,0,27,1);gameInputNative(session,0,27,0);}}
    private void showPause(){
        if(controls.cancelEdit())return;
        boolean wasPlaying=controls.playing();controls.setPlaying(false);if(wasPlaying)escape();
        new android.app.AlertDialog.Builder(this).setTitle(GameActivity.this.getString(R.string.ui_ravendroid_b77d3))
            .setItems(new String[]{GameActivity.this.getString(R.string.ui_resume_game_a6329),GameActivity.this.getString(R.string.ui_touch_menus_805ee),GameActivity.this.getString(R.string.ui_configure_controls_d0cf0),GameActivity.this.getString(R.string.ui_edit_button_layout_edaa6),GameActivity.this.getString(R.string.ui_return_to_launcher_2a9bb)},(dialog,index)->{
                if(index==0){controls.resumePlaying(wasPlaying);if(wasPlaying)escape();}
                else if(index==2)ControlPreferences.show(this,()->{controls.reload();controls.resumePlaying(wasPlaying);if(wasPlaying)escape();});
                else if(index==3)controls.edit(()->{controls.resumePlaying(wasPlaying);if(wasPlaying)escape();});
                else if(index==4)exitToLauncher();
            }).setCancelable(false).show();
    }
    @Override public void onBackPressed(){showPause();}
    private void exitToLauncher(){
        if(exiting)return;exiting=true;wantsRun=false;handler.removeCallbacks(poll);
        touch.cancel();if(controls!=null)controls.releaseAll();
        if(session==0&&!running){finish();return;}
        status.setText(R.string.ui_returning_to_launcher);status.setVisibility(View.VISIBLE);
        // SurfaceView must survive EGL/swapchain retirement on the owner thread.
        if(session!=0)stopSessionNative(session);
    }
    private void hideBars(){
        if(Build.VERSION.SDK_INT>=30){getWindow().setDecorFitsSystemWindows(false);WindowInsetsController controller=getWindow().getInsetsController();if(controller!=null){controller.hide(WindowInsets.Type.systemBars());controller.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);}}
    }
    @Override public void onWindowFocusChanged(boolean focused){super.onWindowFocusChanged(focused);if(focused)hideBars();else{touch.cancel();if(session!=0)pointerNative(session,3,0,0);if(controls!=null&&controls.playing())controls.releaseAll();}}
    @Override protected void onResume(){super.onResume();audioFocus(audioManager.requestAudioFocus(audioFocus)==android.media.AudioManager.AUDIOFOCUS_REQUEST_GRANTED);wantsRun=runtimeReady&&!exiting;handler.removeCallbacks(poll);if(wantsRun)handler.post(poll);}
    @Override protected void onPause(){audioFocus(false);audioManager.abandonAudioFocusRequest(audioFocus);wantsRun=false;handler.removeCallbacks(poll);touch.cancel();if(controls!=null)controls.releaseAll();if(session!=0)stopSessionNative(session);super.onPause();}
    @Override protected void onDestroy(){handler.removeCallbacks(poll);if(session!=0)stopSessionNative(session);super.onDestroy();}
    @Override public void surfaceCreated(SurfaceHolder holder){surfaceReady=true;startIfReady();}
    @Override public void surfaceChanged(SurfaceHolder holder,int format,int width,int height){}
    @Override public void surfaceDestroyed(SurfaceHolder holder){surfaceReady=false;touch.cancel();if(controls!=null)controls.releaseAll();if(session!=0)stopSessionNative(session);}
    private void startIfReady(){
        if(!runtimeReady||!wantsRun||!surfaceReady||running||exiting||isFinishing())return;
        final long id=createSessionNative();if(id==0){status.setText(GameActivity.this.getString(R.string.ui_waiting_for_the_previous_native_session_to_4b552));return;}
        session=id;audioMutedNative(id,!audioGranted);running=true;firstVisible=0;if(controls!=null)controls.setPlaying(false);final Surface target=surface.getHolder().getSurface();
        getSharedPreferences("MainActivity",MODE_PRIVATE).edit().putBoolean("pending",true).putString("lastReport",GameActivity.this.getString(R.string.ui_preparing_native_game_session_529e8)).commit();
        new Thread(null,()->{
            String answer=GameActivity.this.getString(R.string.ui_the_native_game_thread_ended_unexpectedly_a3c86);boolean stopped=false,completed=false;
            try{
            try{
                File artifact=NativeAssets.prepareUnity(this),log=new File(getFilesDir(),"native-progress.log");
                if(new JSONObject(statusNative(id)).optBoolean("stopped"))answer=GameActivity.this.getString(R.string.ui_the_game_session_was_stopped_before_starti_fea94);
                else{
                    try(FileOutputStream output=new FileOutputStream(log)){output.getFD().sync();}
                    answer="RavenDroid "+BuildConfig.VERSION_NAME+"\n"+Build.MANUFACTURER+" "+Build.MODEL+" Android "+Build.VERSION.RELEASE+" API "+Build.VERSION.SDK_INT+"\nTest=unity-live-surface\n"+runLiveNative(id,target,artifact.getAbsolutePath(),log.getAbsolutePath(),new File(getFilesDir(),"game.data").getAbsolutePath());
                }
            }catch(Exception|LinkageError error){answer=GameActivity.this.getString(R.string.ui_game_session_error_cc8c3)+error;}
            try{stopped=new JSONObject(statusNative(id)).optBoolean("stopped");}catch(Exception ignored){}
            completed=true;
            }finally{
                try{if(completed)getSharedPreferences("MainActivity",MODE_PRIVATE).edit().putBoolean("pending",false).putString("lastReport",answer).commit();}
                finally{releaseSessionNative(id);}
            }
            final String result=answer;final boolean requestedStop=stopped;
            runOnUiThread(()->{session=0;running=false;if(isFinishing()||isDestroyed())return;if(exiting){finish();return;}if(!requestedStop){wantsRun=false;status.setText(GameActivity.this.getString(R.string.ui_the_game_session_ended_return_to_the_launc_1c13d));}else startIfReady();});
        },"RavenLiveOwner",16*1024*1024).start();
    }
}
