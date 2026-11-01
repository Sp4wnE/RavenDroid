#!/usr/bin/env python3
"""Geometry QA from production ControlLayout; NOT an Android UI screenshot."""
import argparse
from pathlib import Path
import re
from PIL import Image, ImageDraw, ImageFont, PngImagePlugin
ROOT = Path(__file__).resolve().parents[1]

def render(background, output):
    source = (ROOT / 'native-android/app/src/main/java/dev/ravenfield/nativehost/ControlLayout.java').read_text(encoding='utf-8')
    buttons = re.findall(r'new Button\("([^"]+)",[^,]+,(\d+),(\d+),(\d+)\)', source)
    if len(buttons) != 13:
        raise ValueError('Review parser against changed production layout')
    image = Image.open(background).convert('RGBA')
    if image.size != (1280,720):
        raise ValueError('Expected the actual 1280x720 game capture')
    overlay = Image.new('RGBA',image.size)
    draw = ImageDraw.Draw(overlay)
    font_path = Path('C:/Windows/Fonts/arialbd.ttf')
    font = lambda size: ImageFont.truetype(str(font_path),size)
    for name,x,y,r in buttons:
        x,y,r = int(x),int(y),int(r)
        draw.ellipse((x-r,y-r,x+r,y+r), fill=(20,32,48,158), outline=(220,232,244,153),width=2)
        draw.text((x,y),name,font=font(22 if name=='ATEŞ' else 15),anchor='mm',fill=(241,246,252,255))
    draw.ellipse((96,466,264,634),fill=(20,32,48,79),outline=(220,232,244,153),width=2)
    draw.ellipse((150,520,210,580),fill=(220,232,244,170))
    for left,right,label in [(16,260,'MENÜYE DOKUN'),(1060,1264,'SEÇENEKLER')]:
        draw.rounded_rectangle((left,16,right,64),radius=14,fill=(26,38,54,181))
        draw.text(((left+right)/2,40),label,font=font(18),anchor='mm',fill=(239,245,255,255))
    info=PngImagePlugin.PngInfo()
    info.add_text('Description','Geometry preview from production ControlLayout over real Windows game capture. Not an Android screenshot or device touch validation.')
    Image.alpha_composite(image,overlay).convert('RGB').save(output,pnginfo=info)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('background',type=Path)
    parser.add_argument('output',type=Path)
    args=parser.parse_args()
    render(args.background,args.output)
