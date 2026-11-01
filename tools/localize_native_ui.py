"""One-time extraction of native UI text into English + system Turkish resources.
Diagnostics not listed here remain separate; never rewrites game assets or IDs.
"""
import json,re,hashlib
from pathlib import Path
from xml.sax.saxutils import escape
ROOT=Path(__file__).resolve().parents[1]
BASE=ROOT/'native-android/app/src/main'
TEXT='''SAHAYA HAZIR|READY TO DEPLOY
İLK KURULUM|FIRST SETUP
Joystick boyutu|Joystick size
▶   OYUNU BAŞLAT|▶   LAUNCH GAME
Oyun dosyaları|Game files
Oyun dosyasını seç|Select game file
Kurulu prototip bulunamadı. Mevcut oyun APK / ZIP dosyanı seçebilirsin.|The prototype is not installed. Select your existing game APK or ZIP instead.
Ravenfield APK / ZIP dosyanı bir kez seç. Hazırlanan içerik sonraki açılışlar için saklanır.|Select your Ravenfield APK or ZIP once. Imported content is kept for future launches.
Dosya seç|Choose file
Kurulu prototipten aktar|Import from installed prototype
Kendine göre ayarla|Make it yours
Kontroller ve hassasiyet  ›|Controls and sensitivity  ›
Düğme yerleşimini düzenle  ›|Edit button layout  ›
Ses  ›|Audio  ›
Oyun sesi|Game audio
Ses düzeyini telefonunun medya ses tuşlarıyla ayarlayabilirsin.|Use your phone's media volume buttons to adjust game audio.
Tamam|OK
Yardım ve oturum raporu  ›|Help and session report  ›
Ravenfield · Yardım|RavenDroid · Help
Nasıl oynanır?|How to play
Son oturum raporunu kopyala|Copy last session report
Tanılama araçları|Diagnostic tools
Ses kütüphanesi lisansı|Audio library license
Sahaya çık|Deploy
Oturum raporu kopyalandı|Session report copied
Açık kaynak ses kütüphanesi|Open-source audio library
Lisans dosyası okunamadı|Could not read license file
Tanılama · Destek için|Diagnostics · For support
Küçük ARM64 testi|Small ARM64 test
Unity + dosya sistemi testi|Unity + filesystem test
Sınırlı görüntü testi|Bounded rendering test
Son oyun testi görüntüsü|Last rendering test image
HAZIRLANIYOR|PREPARING
OYUN DOSYALARI MEVCUT|GAME FILES AVAILABLE
Hazırsan sahaya çık. Oyun içi kontrolleri üst soldan açabilirsin.|Ready when you are. Enable in-game controls from the top left.
Mevcut Ravenfield APK / ZIP dosyanı seç. Oyun verisi yalnızca bir kez aktarılır.|Select your existing Ravenfield APK or ZIP. Game data only needs to be imported once.
Dosyalar hazırlanamadı. Yardım bölümünden ayrıntıları görebilirsin.|Could not prepare files. Open Help for details.
Kamera hassasiyeti|Look sensitivity
Nişan hassasiyeti|Aim sensitivity
Düğme boyutu|Button size
Kontrol görünürlüğü|Control visibility
Dikey kamerayı ters çevir|Invert vertical look
Nişanı dokunarak aç / kapat|Tap to toggle aim
Koşmayı dokunarak aç / kapat|Tap to toggle sprint
Dokunmatik kontroller|Touch controls
Vazgeç|Cancel
Kaydet|Save
KAYDET|SAVE
SIFIRLA|RESET
VAZGEÇ|CANCEL
ARAÇ → YAYA|VEHICLE → ON FOOT
YAYA → ARAÇ|ON FOOT → VEHICLE
BOYUT / OPAKLIK|SIZE / OPACITY
Opaklık|Opacity
Düğmeleri sürükle · Ayarlamak için dokun|Drag buttons · Tap to adjust
MENÜYE DOKUN|TOUCH MENUS
KONTROLLERİ AÇ|ENABLE CONTROLS
SEÇENEKLER|OPTIONS
Dokunmatik hareket, kamera ve oyun düğmeleri. Üst soldan kontrolleri açıp kapat; üst sağdan seçenekler.|Touch movement, camera and action buttons. Toggle controls at the top left; options at the top right.
Oyun hazırlanıyor… İlk açılış birkaç dakika sürebilir.|Preparing the game… The first launch can take a few minutes.
Oyun içeriği yükleniyor…|Loading game content…
Oturum durumu okunamadı|Could not read session status
Oyuna dön|Resume game
Menülere dokun|Touch menus
Kontrolleri ayarla|Configure controls
Düğme yerleşimini düzenle|Edit button layout
Launcher’a dön|Return to launcher
Önceki native oturumun durması bekleniyor…|Waiting for the previous native session to stop…
Oyun oturumu sona erdi. Seçeneklerden launcher’a dön; rapor Yardım bölümüne kaydedildi.|The game session ended. Return to the launcher using Options; the report is saved under Help.
Canlı native iş parçacığı beklenmedik şekilde sonlandı.|The native game thread ended unexpectedly.
Canlı native oturum hazırlanıyor…|Preparing native game session…
Canlı oturum başlatılmadan kapatıldı.|The game session was stopped before starting.
Canlı oturum hatası: |Game session error:
Ravenfield|RavenDroid'''
M=dict(line.split('|',1) for line in TEXT.splitlines())
M['Canlı oturum hatası: ']+=' '
M['Play → Instant Action ile haritayı seç ve Deploy’a bas. Doğduktan sonra üst soldan kontrolleri aç. Sol joystick hareket ettirir, sağ alan kamerayı çevirir. Ateş düğmesini tutup sürükleyebilirsin. Düğme yerlerini seçeneklerden düzenleyebilirsin. Üstte YAYA / ARAÇ ile kontrol profilini değiştir. Araçta sol yön tuşlarıyla dön, sağda gaz/fren kullan. Sağ alan kule/kamera ve uçuş kontrolüdür. K1–K7 koltuk seçer; SAVUNMA yalnızca destekleyen araçlarda çalışır.']='Choose Play → Instant Action, select a map and press Deploy. Enable controls at the top left after spawning. Move with the left stick and look using the right area, or hold and drag Fire. Customize button positions in Options. Switch between ON FOOT and VEHICLE at the top. Vehicles use steering arrows, throttle and brake; the right area controls the camera, turret or flight. S1–S7 select seats. COUNTERMEASURES only works on supported vehicles.'
M['Sol başparmak: hareket. Sağ alan: kamera. Ateş düğmesini basılı tutup sürükleyerek de bakabilirsin. Araçta gaz/freni bırakmadan sürükleyerek bakabilir, yön okları arasında kaydırabilirsin. Teçhizat düğmesi silah seçimini açar. Menüye geçerken bütün basılı tuşlar bırakılır.']='Left thumb: movement. Right area: look. Hold and drag Fire to look while shooting. In vehicle mode, drag throttle/brake without releasing, or slide between steering arrows. Loadout opens weapon selection. All held inputs are released when switching to menus.'
CONTROLS={'ATEŞ':'FIRE','NİŞAN':'AIM','ZIPLA':'JUMP','DOLDUR':'RELOAD','EĞİL':'CROUCH','KULLAN':'USE','KOŞ':'SPRINT','HARİTA':'MAP','TEÇHİZAT ↵':'LOADOUT ↵','GAZ':'THROTTLE','FREN / GERİ':'BRAKE / REVERSE','BİN / İN':'ENTER / EXIT','KAMERA':'CAMERA','SAVUNMA':'COUNTERMEASURES','SİLAH 1':'WEAPON 1','SİLAH 2':'WEAPON 2',**{f'K{i}':f'S{i}' for i in range(1,8)}}
def control_key(en):return 'control_'+re.sub('[^a-z0-9]+','_',en.lower()).strip('_')
def key(text):
 slug=re.sub('[^a-z0-9]+','_',M[text].lower()).strip('_')[:42]
 return 'ui_'+slug+'_'+hashlib.sha1(text.encode()).hexdigest()[:5]
def xml(text):return escape(text).replace("'",r"\'").replace('"',r'\"').replace('\n',r'\n')
def main():
 for folder,language in [('values',False),('values-tr',True)]:
  p=BASE/'res'/folder/'ui_strings.xml';p.parent.mkdir(exist_ok=True)
  def localized(tr,en):
   if not language:return en
   return 'RavenDroid' if tr=='Ravenfield' else tr.replace('Ravenfield · Yardım','RavenDroid · Yardım')
  p.write_text('<resources>\n'+''.join(f'    <string name="{key(tr)}" formatted="false">{xml(localized(tr,en))}</string>\n' for tr,en in M.items())+''.join(f'    <string name="{control_key(en)}">{xml(tr if language else en)}</string>\n' for tr,en in CONTROLS.items())+'</resources>\n',encoding='utf-8')
 contexts={'LauncherScreen':'activity','ControlPreferences':'context','GameControlView':'getContext()','GameActivity':'GameActivity.this','MainActivity':'MainActivity.this'}
 for name,context in contexts.items():
  p=BASE/'java/dev/ravenfield/nativehost'/f'{name}.java';s=p.read_text(encoding='utf-8')
  def replace(match):
   value=json.loads(match.group())
   return f'{context}.getString(R.string.{key(value)})' if value in M else match.group()
  s=re.sub(r'"(?:[^"\\]|\\.)*"',replace,s)
  p.write_text(s,encoding='utf-8')
 diagnostics=json.loads((ROOT/'tools/native_diagnostic_english.json').read_text(encoding='utf-8'))
 for name in ['LauncherScreen','MainActivity','GameActivity','GameDataImporter','NativeAssets','NativeCrashSummary']:
  p=BASE/'java/dev/ravenfield/nativehost'/f'{name}.java';s=p.read_text(encoding='utf-8')
  s=re.sub(r'"(?:[^"\\]|\\.)*"',lambda m:json.dumps(diagnostics.get(json.loads(m.group()),json.loads(m.group())),ensure_ascii=False),s)
  p.write_text(s,encoding='utf-8')
 p=BASE/'java/dev/ravenfield/nativehost/ControlLayout.java';s=p.read_text(encoding='utf-8')
 s=re.sub(r'"(?:[^"\\]|\\.)*"',lambda m:json.dumps(CONTROLS.get(json.loads(m.group()),json.loads(m.group())),ensure_ascii=False),s)
 p.write_text(s,encoding='utf-8')
 text='package dev.ravenfield.nativehost;\n/** Labels resolved once per view, never in the native frame loop. */\nfinal class ControlLabels {\n static java.util.Map<String,String> load(android.content.Context context){\n  java.util.Map<String,String> labels=new java.util.HashMap<>();\n'
 text+=''.join(f'  labels.put({json.dumps(en,ensure_ascii=False)},context.getString(R.string.{control_key(en)}));\n' for en in CONTROLS.values())
 text+='  return labels;\n }\n}\n';(BASE/'java/dev/ravenfield/nativehost/ControlLabels.java').write_text(text,encoding='utf-8')
if __name__=='__main__':main()
