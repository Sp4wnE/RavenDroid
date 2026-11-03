package dev.ravenfield.nativehost;

import android.app.AlertDialog;
import android.content.Context;
import android.widget.Toast;

final class RenderSettings {
    static final int[] WIDTHS={640,960,1280};
    static final int[] HEIGHTS={360,540,720};
    static int selected(Context c) { int i=c.getSharedPreferences("render",Context.MODE_PRIVATE).getInt("resolution",2);return i>=0&&i<WIDTHS.length?i:2; }
    static void show(Context c) {
        String[] labels={"640 × 360", "960 × 540", "1280 × 720"};
        new AlertDialog.Builder(c).setTitle(R.string.ui_render_resolution)
            .setSingleChoiceItems(labels,selected(c),(dialog,index)->{
                c.getSharedPreferences("render",Context.MODE_PRIVATE).edit().putInt("resolution",index).apply();
                dialog.dismiss();Toast.makeText(c,R.string.ui_resolution_next_game,Toast.LENGTH_LONG).show();
            }).setNegativeButton(android.R.string.cancel,null).show();
    }
}
