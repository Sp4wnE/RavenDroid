package dev.ravenfield.nativehost;
import android.content.Context;
import android.graphics.*;
import android.view.View;
/** Original procedural tactical backdrop; no game assets are bundled here. */
final class LauncherBackdrop extends View {
    private final Paint p=new Paint(Paint.ANTI_ALIAS_FLAG);
    LauncherBackdrop(Context context){super(context);setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_NO);}
    @Override protected void onDraw(Canvas c){
        float w=getWidth(),h=getHeight();p.setShader(new LinearGradient(0,0,w,h,0xff101b2b,0xff050a12,Shader.TileMode.CLAMP));c.drawRect(0,0,w,h,p);p.setShader(null);
        p.setColor(0x123d8dcd);for(int i=0;i<6;i++){Path path=new Path();float x=w*(.55f+i*.055f);path.moveTo(x,0);path.lineTo(x+w*.2f,h);path.lineTo(x+w*.25f,h);path.lineTo(x+w*.05f,0);path.close();c.drawPath(path,p);}
        p.setColor(0x284b91c8);p.setStyle(Paint.Style.STROKE);p.setStrokeWidth(1);
        for(int i=0;i<10;i++){float x=w*.76f,y=h*.48f;RectF r=new RectF(x-w*(.14f+i*.035f),y-h*(.12f+i*.065f),x+w*(.17f+i*.035f),y+h*(.13f+i*.065f));c.drawOval(r,p);}
        p.setStyle(Paint.Style.FILL);p.setColor(0xff398ad1);c.drawRect(0,0,w*.68f,4,p);p.setColor(0xffc14850);c.drawRect(w*.68f,0,w,4,p);
    }
}
