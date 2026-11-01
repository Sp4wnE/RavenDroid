package dev.ravenfield.nativehost;
import android.app.*;
import android.view.*;
import android.widget.*;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;

/** Main launcher UI. Test tools and raw reports are hidden behind Help. */
final class LauncherScreen {
    final Button play,choose,installed,smoke,unity,graphics;
    final TextView output;final ImageView frame;
    private final Activity activity;private final TextView state,detail;private final ProgressBar progress;
    private int dp(float v){return Math.round(v*activity.getResources().getDisplayMetrics().density);}
    private TextView label(String text,float size,int color){TextView v=new TextView(activity);v.setText(text);v.setTextSize(size);v.setTextColor(color);return v;}
    private GradientDrawable panel(int color,int stroke){GradientDrawable d=new GradientDrawable();d.setColor(color);d.setCornerRadius(dp(16));if(stroke!=0)d.setStroke(dp(1),stroke);return d;}
    private Button button(String text,int color,Runnable click){Button b=new Button(activity);b.setText(text);b.setTextColor(0xfff0f5fc);b.setTextSize(15);b.setAllCaps(false);b.setMinHeight(dp(50));b.setPadding(dp(16),dp(8),dp(16),dp(8));b.setBackground(panel(color,0x445c87b0));LinearLayout.LayoutParams p=new LinearLayout.LayoutParams(-1,-2);p.topMargin=dp(10);b.setLayoutParams(p);b.setOnClickListener(v->click.run());return b;}
    LauncherScreen(Activity activity,Runnable launch,Runnable select,Runnable importInstalled,Runnable copy,Runnable smallTest,Runnable unityTest,Runnable graphicsTest){
        this.activity=activity;
        LinearLayout root=new LinearLayout(activity);root.setOrientation(LinearLayout.VERTICAL);root.setBackgroundColor(0xff0c0f13);activity.setContentView(root);
        if(android.os.Build.VERSION.SDK_INT>=30){root.setOnApplyWindowInsetsListener((view,insets)->{android.graphics.Insets bars=insets.getInsets(WindowInsets.Type.systemBars()|WindowInsets.Type.displayCutout());view.setPadding(bars.left,bars.top,bars.right,bars.bottom);return WindowInsets.CONSUMED;});root.requestApplyInsets();}else root.setFitsSystemWindows(true);
        ScrollView scroll=new ScrollView(activity);scroll.setFillViewport(true);root.addView(scroll,new LinearLayout.LayoutParams(-1,-1));
        LinearLayout column=new LinearLayout(activity);column.setOrientation(LinearLayout.VERTICAL);column.setPadding(dp(22),dp(28),dp(22),dp(32));scroll.addView(column);
        ImageView logo=new ImageView(activity);logo.setImageResource(R.drawable.ravenfield_logo);logo.setContentDescription(activity.getString(R.string.ui_ravendroid_b77d3));logo.setAdjustViewBounds(true);logo.setScaleType(ImageView.ScaleType.FIT_CENTER);column.addView(logo,new LinearLayout.LayoutParams(-1,dp(190)));
        TextView tagline=label("MOBILE  /  "+BuildConfig.VERSION_NAME.split("-",2)[0],11,0xff83a9cf);tagline.setLetterSpacing(.14f);tagline.setPadding(0,dp(8),0,dp(28));column.addView(tagline);
        LinearLayout ready=card(column);ready.addView(label(activity.getString(R.string.ui_ready_to_deploy_ec5ed),20,0xfff1f3f7));
        state=label(activity.getString(R.string.ui_first_setup_91d38),11,0xff8dbfea);state.setPadding(0,dp(18),0,dp(8));ready.addView(state);
        detail=label("",15,0xffb7becb);detail.setLineSpacing(dp(4),1);ready.addView(detail);
        progress=new ProgressBar(activity);ready.addView(progress,new LinearLayout.LayoutParams(dp(28),dp(28)));progress.setVisibility(View.GONE);
        play=button(activity.getString(R.string.ui_launch_game_5e285),0xff397ec0,launch);play.setTextSize(18);column.addView(play);
        LinearLayout files=card(column);files.addView(label(activity.getString(R.string.ui_game_files_9dedc),20,0xfff1f3f7));
        TextView info=label(activity.getString(BuildConfig.BUNDLED_GAME_DATA?R.string.ui_bundled_game_info:R.string.ui_select_your_ravenfield_apk_or_zip_once_imp_e3030),14,0xffaab4c3);info.setPadding(0,dp(12),0,dp(8));files.addView(info);
        choose=button(activity.getString(BuildConfig.BUNDLED_GAME_DATA?R.string.ui_prepare_game_files:R.string.ui_choose_file_3fc11),0xff252d39,select);files.addView(choose);
        installed=button(activity.getString(R.string.ui_import_from_installed_prototype_ab243),0xff191f28,importInstalled);files.addView(installed);
        if(BuildConfig.BUNDLED_GAME_DATA)installed.setVisibility(View.GONE);
        LinearLayout settings=card(column);settings.addView(label(activity.getString(R.string.ui_make_it_yours_dc271),20,0xfff1f3f7));
        settings.addView(button(activity.getString(R.string.ui_controls_and_sensitivity_dd47b),0xff252d39,()->ControlPreferences.show(activity,null)));
        settings.addView(button(activity.getString(R.string.ui_edit_button_layout_92a0b),0xff252d39,()->activity.startActivity(new android.content.Intent(activity,ControlEditorActivity.class))));
        settings.addView(button(activity.getString(R.string.ui_audio_b3773),0xff252d39,()->new AlertDialog.Builder(activity).setTitle(activity.getString(R.string.ui_game_audio_1790a)).setMessage(activity.getString(R.string.ui_use_your_phone_s_media_volume_buttons_to_a_0b2e6)).setPositiveButton(activity.getString(R.string.ui_ok_2dfaa),null).show()));
        LinearLayout diagnostics=new LinearLayout(activity);diagnostics.setOrientation(LinearLayout.VERTICAL);diagnostics.setPadding(0,dp(24),0,0);column.addView(diagnostics);diagnostics.setVisibility(View.GONE);
        column.addView(button(activity.getString(R.string.ui_help_and_session_report_d54b4),0xff191f28,()->new AlertDialog.Builder(activity).setTitle(activity.getString(R.string.ui_ravendroid_help_689ab)).setItems(new String[]{activity.getString(R.string.ui_how_to_play_9e414),activity.getString(R.string.ui_copy_last_session_report_6e7df),activity.getString(R.string.ui_diagnostic_tools_90042),activity.getString(R.string.ui_audio_library_license_50dcc)},(dialog,index)->{
            if(index==0)new AlertDialog.Builder(activity).setTitle(activity.getString(R.string.ui_deploy_8f72e)).setMessage(activity.getString(R.string.ui_choose_play_instant_action_select_a_map_an_2c16c)).setPositiveButton(activity.getString(R.string.ui_ok_2dfaa),null).show();
            else if(index==1){copy.run();Toast.makeText(activity,activity.getString(R.string.ui_session_report_copied_94f93),Toast.LENGTH_SHORT).show();}
            else if(index==3){try(java.io.InputStream in=activity.getAssets().open("third-party-audio.txt")){java.io.ByteArrayOutputStream bytes=new java.io.ByteArrayOutputStream();byte[] buffer=new byte[2048];int count;while((count=in.read(buffer))!=-1)bytes.write(buffer,0,count);new AlertDialog.Builder(activity).setTitle(activity.getString(R.string.ui_open_source_audio_library_ddaf2)).setMessage(new String(bytes.toByteArray(),java.nio.charset.StandardCharsets.UTF_8)).setPositiveButton(activity.getString(R.string.ui_ok_2dfaa),null).show();}catch(java.io.IOException error){Toast.makeText(activity,activity.getString(R.string.ui_could_not_read_license_file_fcd69),Toast.LENGTH_SHORT).show();}}
            else diagnostics.setVisibility(diagnostics.getVisibility()==View.VISIBLE?View.GONE:View.VISIBLE);
        }).show()));
        diagnostics.addView(label(activity.getString(R.string.ui_diagnostics_for_support_adebd),18,0xffb8c9dc));
        smoke=button(activity.getString(R.string.ui_small_arm64_test_49a39),0xff162535,smallTest);diagnostics.addView(smoke);
        unity=button(activity.getString(R.string.ui_unity_filesystem_test_62867),0xff162535,unityTest);diagnostics.addView(unity);
        graphics=button(activity.getString(R.string.ui_bounded_rendering_test_246a3),0xff162535,graphicsTest);diagnostics.addView(graphics);
        frame=new ImageView(activity);frame.setAdjustViewBounds(true);frame.setContentDescription(activity.getString(R.string.ui_last_rendering_test_image_40c87));diagnostics.addView(frame);
        output=label("",12,0xffbac9da);output.setTextIsSelectable(true);diagnostics.addView(output);
    }
    private LinearLayout card(LinearLayout parent){LinearLayout c=new LinearLayout(activity);c.setOrientation(LinearLayout.VERTICAL);c.setPadding(dp(20),dp(22),dp(20),dp(22));c.setBackground(panel(0xff191e27,0));LinearLayout.LayoutParams p=new LinearLayout.LayoutParams(-1,-2);p.topMargin=dp(20);parent.addView(c,p);return c;}
    void status(boolean busy,boolean data,String report){
        state.setText(busy?activity.getString(R.string.ui_preparing_29ecb):data?activity.getString(R.string.ui_game_files_available_ebeba):activity.getString(R.string.ui_first_setup_91d38));progress.setVisibility(busy?View.VISIBLE:View.GONE);
        String message=busy?report.split("\n",2)[0]:data?activity.getString(R.string.ui_ready_when_you_are_enable_in_game_controls_0126d):activity.getString(R.string.ui_select_your_existing_ravenfield_apk_or_zip_ab713);
        if(!busy&&report.startsWith("Could not import game data"))message=activity.getString(BuildConfig.BUNDLED_GAME_DATA?R.string.ui_bundled_game_failed:R.string.ui_could_not_prepare_files_open_help_for_deta_5339a);
        else if(!busy&&report.startsWith("Unable to load ARM64 native runtime"))message=activity.getString(R.string.ui_could_not_prepare_files_open_help_for_deta_5339a);
        detail.setText(message.length()>180?message.substring(0,180):message);
    }
}
