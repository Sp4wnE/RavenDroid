"""Input contract tests in real Chromium; does not boot or require the game."""
import json
import os
import shutil
from pathlib import Path
import unittest
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]


class InputTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.playwright = sync_playwright().start()
        executable = os.environ.get('CHROMIUM_BINARY') or shutil.which('chromium') or shutil.which('google-chrome')
        cls.browser = cls.playwright.chromium.launch(executable_path=executable, args=['--no-sandbox'])

    @classmethod
    def tearDownClass(cls):
        cls.browser.close()
        cls.playwright.stop()

    def setUp(self):
        self.page = self.browser.new_page()
        self.page.set_content('<canvas id="canvas" width="1280" height="720"></canvas>')
        self.page.add_script_tag(path=str(ROOT / 'app/src/main/assets/web/input.js'))
        self.page.evaluate('''() => {
          window.events=[]; const canvas=document.querySelector('canvas');
          for (const type of ['keydown','keyup','mousedown','mouseup','mousemove'])
            canvas.addEventListener(type,e=>events.push({type,code:e.code,keyCode:e.keyCode,button:e.button,buttons:e.buttons,dx:e.movementX,dy:e.movementY}));
          window.input=new RavenInput(canvas);
        }''')

    def tearDown(self): self.page.close()

    def test_shared_key_is_released_only_after_last_finger(self):
        events = self.page.evaluate('''() => { input.setSource(1,['KeyW']);input.setSource(2,['KeyW']);input.removeSource(1);input.removeSource(2);return events; }''')
        self.assertEqual([(e['type'],e['keyCode']) for e in events],[('keydown',87),('keyup',87)])

    def test_movement_aim_and_fire_can_be_held_together(self):
        events = self.page.evaluate('''() => { input.setSource(1,['KeyW','KeyD']);input.setSource(2,[],[2]);input.setSource(3,[],[0]);input.release();return events; }''')
        self.assertEqual([e['buttons'] for e in events if e['type']=='mousedown'],[2,3])
        self.assertEqual(len([e for e in events if e['type']=='keyup']),2)
        self.assertEqual(events[-1]['buttons'],0)

    def test_fractional_camera_motion_accumulates_without_event_flood(self):
        events = self.page.evaluate('''() => { for(let i=0;i<4;i++) input.look(.3,-.3);input.flushLook();return events; }''')
        self.assertEqual(len(events),1)
        self.assertEqual((events[0]['dx'],events[0]['dy']),(1,-1))

    def test_pause_release_cancels_pending_look_and_buttons(self):
        state = self.page.evaluate('''() => { input.setSource(1,['Space'],[0]);input.look(30,20);input.release();input.flushLook();return {events,keys:input.sentKeys.size,buttons:input.mouseMask,sources:input.sources.size}; }''')
        self.assertEqual((state['keys'],state['buttons'],state['sources']),(0,0,0))
        self.assertFalse(any(e['type']=='mousemove' for e in state['events']))

    def test_numeric_keyboard_emits_legacy_text_event(self):
        event = self.page.evaluate('''() => {
          let result;document.querySelector('canvas').addEventListener('keypress',e=>result={key:e.key,code:e.code,charCode:e.charCode});
          input.setSource(1,['Digit4']);input.removeSource(1);return result;
        }''')
        self.assertEqual(event,{'key':'4','code':'Digit4','charCode':52})


if __name__ == '__main__': unittest.main(verbosity=2)
