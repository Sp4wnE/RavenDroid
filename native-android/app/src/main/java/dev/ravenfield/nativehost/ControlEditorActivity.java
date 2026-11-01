package dev.ravenfield.nativehost;
import android.app.Activity;
import android.os.Bundle;
import android.widget.FrameLayout;
import android.view.Gravity;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
public final class ControlEditorActivity extends Activity {
    @Override public void onCreate(Bundle state){super.onCreate(state);getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_FULLSCREEN|View.SYSTEM_UI_FLAG_HIDE_NAVIGATION|View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY);FrameLayout root=new FrameLayout(this);root.setBackgroundColor(0xff243347);setContentView(root);GameControlView view=new GameControlView(this,(k,a,b)->true,()->{});root.addView(view,new FrameLayout.LayoutParams(-2,-2,Gravity.CENTER));view.edit(this::finish);}
    @Override public void onWindowFocusChanged(boolean focused){super.onWindowFocusChanged(focused);if(focused&&android.os.Build.VERSION.SDK_INT>=30){WindowInsetsController controller=getWindow().getInsetsController();if(controller!=null){controller.hide(WindowInsets.Type.systemBars());controller.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);}}}
}
