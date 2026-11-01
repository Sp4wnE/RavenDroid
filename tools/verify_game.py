#!/usr/bin/env python3
"""Run the real build in desktop Chromium and capture evidence. Requires tools/serve.py.

This is NOT an Android device test. UI coordinates target the pinned build at 1280x720.
Screenshots need visual review; a successful script alone does not certify gameplay.
"""
import asyncio
import json
import os
from pathlib import Path
import shutil
from urllib.parse import urlsplit
from playwright.async_api import async_playwright

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'artifacts/verification'


async def main():
    OUT.mkdir(parents=True, exist_ok=True)
    external, errors, failures = [], [], []
    report = {'platform': 'desktop Chromium / software WebGL, NOT Android', 'stages': {}}
    async with async_playwright() as p:
        executable = os.environ.get('CHROMIUM_BINARY') or shutil.which('chromium') or shutil.which('google-chrome')
        browser = await p.chromium.launch(executable_path=executable, args=[
            '--no-sandbox', '--disable-dev-shm-usage', '--use-angle=swiftshader',
            '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist',
            '--autoplay-policy=no-user-gesture-required', '--disable-background-timer-throttling',
            '--disable-renderer-backgrounding', '--disable-backgrounding-occluded-windows'])
        context = await browser.new_context(viewport={'width': 1280, 'height': 720}, has_touch=True)
        async def route(request_route):
            host = urlsplit(request_route.request.url).hostname
            if host not in ('127.0.0.1', 'localhost', None):
                external.append(request_route.request.url)
                await request_route.abort()
            else: await request_route.continue_()
        await context.route('**/*', route)
        page = await context.new_page()
        log = (OUT / 'console.log').open('w')
        def console(message):
            log.write(message.type + ': ' + message.text + '\n'); log.flush()
        page.on('console', console)
        page.on('pageerror', lambda error: errors.append(str(error)))
        page.on('requestfailed', lambda request: failures.append({'url': request.url, 'error': request.failure}))
        cdp = await context.new_cdp_session(page)
        async def frames(n=5):
            current = await page.evaluate('RavenPort.diagnostics().frameNumber || 0')
            await page.wait_for_function('(target)=>(RavenPort.diagnostics().frameNumber||0)>=target', arg=current+n, timeout=120000)
        async def tap(x, y):
            await cdp.send('Input.dispatchTouchEvent', {'type':'touchStart','touchPoints':[{'x':x,'y':y,'id':1}]})
            await frames(2)
            await cdp.send('Input.dispatchTouchEvent', {'type':'touchEnd','touchPoints':[]})
            await frames(3)
        async def tap_element(selector):
            box = await page.locator(selector).bounding_box()
            assert box, selector
            await tap(box['x']+box['width']/2,box['y']+box['height']/2)
        async def capture(name):
            report['stages'][name] = await page.evaluate('RavenPort.diagnostics()')
            await page.screenshot(path=str(OUT / (name+'.png')))
            print(name, json.dumps(report['stages'][name]), flush=True)
        try:
            await page.goto('http://127.0.0.1:8080/web/index.html')
            await page.click('#start')
            await page.wait_for_function('RavenPort.diagnostics().ready', timeout=300000)
            await page.evaluate('RavenPort.setRenderHeight(360)')
            # Unity's instance-ready callback precedes the game's asynchronous
            # title scene. Wait for this pinned build's menu resources as well.
            await page.wait_for_function('RavenPort.diagnostics().audioSources > 200 && RavenPort.diagnostics().heapBytes >= 1073741824', timeout=180000)
            await frames(15)
            await capture('01-title')
            await tap(640,580)
            await tap(640,320)
            await tap(380,580)
            await capture('02-settings')
            # Use the actual port keypad to enter a small bot count on both sides.
            await tap_element('#keyboard-button')
            for y in (418,505):
                await tap(900,y)
                await tap_element('#keyboard [data-key="Backspace"]')
                await tap_element('#keyboard [data-key="Backspace"]')
                await tap_element('#keyboard [data-key="Digit2"]')
                await tap_element('#keyboard [data-key="Enter"]')
            await tap_element('#keyboard-button')
            await capture('03-low-bots')
            await tap(1190,685)
            await frames(30)
            await capture('04-deploy-menu')
            await tap(1080,455)
            await tap(980,580)
            await page.wait_for_function('RavenPort.diagnostics().virtualLock', timeout=120000)
            assert await page.locator('#controls').is_visible(), 'Touch HUD did not appear on spawn'
            await capture('05-spawn')
            joy=await page.locator('#joystick').bounding_box()
            fire=await page.locator('[data-mouse="0"]').bounding_box()
            points=[{'x':joy['x']+joy['width']/2,'y':joy['y']+10,'id':1},
                    {'x':740,'y':340,'id':2},
                    {'x':fire['x']+fire['width']/2,'y':fire['y']+fire['height']/2,'id':3}]
            await cdp.send('Input.dispatchTouchEvent',{'type':'touchStart','touchPoints':points})
            await frames(3)
            for i in range(1,6):
                points[1]['x']=740+i*15
                await cdp.send('Input.dispatchTouchEvent',{'type':'touchMove','touchPoints':points})
                await frames(2)
            await cdp.send('Input.dispatchTouchEvent',{'type':'touchEnd','touchPoints':[]})
            await frames(5)
            await page.evaluate('RavenPort.setControls("off")')
            await capture('06-move-look-fire')
            await page.evaluate('RavenPort.setControls("auto")')
            aim=await page.locator('[data-mouse="2"]').bounding_box()
            await cdp.send('Input.dispatchTouchEvent',{'type':'touchStart','touchPoints':[{'x':aim['x']+aim['width']/2,'y':aim['y']+aim['height']/2,'id':4}]})
            await frames(8)
            await capture('07-aim')
            await cdp.send('Input.dispatchTouchEvent',{'type':'touchEnd','touchPoints':[]})
            await tap_element('[data-key="Space"]')
            await capture('08-jump-input')
            await page.evaluate('RavenPort.setHostPaused(true)')
            await page.wait_for_timeout(500)
            paused=await page.evaluate('RavenPort.diagnostics()')
            await page.wait_for_timeout(1200)
            later=await page.evaluate('RavenPort.diagnostics()')
            assert paused['frameNumber']==later['frameNumber'], 'Unity kept running during pause'
            assert later['audioState']=='suspended', 'Audio was not suspended'
            assert await page.evaluate('RavenPort.input.sources.size') == 0, 'Input stuck after pause'
            await capture('09-paused')
            await page.evaluate('RavenPort.setHostPaused(false)')
            await frames(5)
            assert await page.evaluate('RavenPort.diagnostics().audioState')=='running'
            await capture('10-resumed')
            assert not external, f'External requests: {external}'
            assert not errors, f'Uncaught exceptions: {errors}'
            assert not failures, f'Resource failures: {failures}'
            report['automatedChecks']='passed; review gameplay screenshots separately'
        except Exception as error:
            report['failure']=str(error)
            await page.screenshot(path=str(OUT/'failure.png'))
            raise
        finally:
            report.update(externalRequests=external,pageErrors=errors,requestFailures=failures)
            (OUT/'report.json').write_text(json.dumps(report,indent=2)+'\n')
            await browser.close();log.close()


if __name__=='__main__': asyncio.run(main())
