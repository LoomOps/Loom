"""Workspace browser helpers and read-only live acceptance."""
import base64
import json
from pathlib import Path
import time
from urllib.parse import quote, unquote

import verify_browser as harness


class Workspace:
    def __init__(self, wd):
        self.wd = wd

    def js(self, script, *args):
        return self.wd("/execute/sync", {"script": script, "args": list(args)})

    def wait(self, script, *args):
        for _ in range(90):
            result = self.js(script, *args)
            if result:
                return result
            time.sleep(.15)
        raise AssertionError(f"Workspace expectation failed for {args!r}")

    def click(self, selector):
        self.wait("""const node = document.querySelector(arguments[0]);
                     if (!node) return false;
                     node.click();
                     return true;""", selector)

    def nav(self, name):
        self.wait("""const button = [...document.querySelectorAll('#pa-nav button')]
                       .find(button => button.textContent.trim() === arguments[0]);
                     if (!button) return false;
                     button.click();
                     return true;""", name)
        self.wait("return document.querySelector('#pa-breadcrumb')?.textContent.includes(arguments[0])", name)

    def frame(self):
        self.wd("/frame", {"id": self.wd("/element", {"using": "css selector", "value": "iframe"})})

    def top(self):
        self.wd("/frame", {"id": None})

    def connect(self):
        """Use the caller's native Tailscale identity or explicit dev instance."""
        self.wait("return !document.querySelector('iframe').hidden")
        assert self.js("""const frame = document.querySelector('iframe');
                          return frame.sandbox.value === 'allow-scripts' && frame.contentDocument === null
                            && !document.querySelector('#loom-access-token');""")
        self.frame()

    def entity(self, identity):
        self.wait("""const link = [...document.querySelectorAll('#pa-content [data-entity]')]
                       .find(link => link.dataset.entity === arguments[0]);
                     if (!link) return false;
                     link.click();
                     return true;""", identity)
        self.wait("""return [...document.querySelectorAll('#pa-content [data-share-route]')]
                       .some(link => link.dataset.shareRoute === arguments[0]);""", identity)

    def search(self, query, label):
        self.click(".pa-search-launch")
        self.js("""const input = document.querySelector('#pa-search');
                   input.value = arguments[0];
                   input.dispatchEvent(new Event('input', {bubbles:true}));""", query)
        self.wait("""return [...document.querySelectorAll('.pa-command-result')]
                       .some(button => button.textContent.includes(arguments[0]));""", label)
        self.js("""document.querySelector('#pa-search').dispatchEvent(
                     new KeyboardEvent('keydown', {key:'Escape', bubbles:true}));""")

    def route(self, identity):
        self.top()
        self.js("location.hash = arguments[0]", "#/entities/" + quote(identity, safe=""))
        self.frame()
        self.wait("""return [...document.querySelectorAll('#pa-content [data-share-route]')]
                       .some(node => node.dataset.shareRoute === arguments[0]);""", identity)

    def media(self, fixture):
        identity = fixture.get("entity")
        if identity:
            self.route(identity)
        media_id = fixture.get("id")
        if media_id:
            self.wait("""const button = [...document.querySelectorAll('[data-loom-media]')]
                           .find(button => button.dataset.loomMedia === arguments[0]);
                         if (!button) return false;
                         button.click();
                         return true;""", media_id)
        else:
            self.click("[data-loom-media]")
        if fixture.get("type") == "video":
            return self.wait("""const video = document.querySelector('video.pa-loom-preview');
                if (!video?.src.startsWith('blob:')) return false;
                if (video.readyState >= 1 && video.videoWidth > 0) return 'decoded';
                if (video.error?.code === 4 && !video.canPlayType('video/mp4; codecs="avc1.42E01E"')
                    && document.querySelector('[data-loom-media-download]')) return 'codec_unavailable_download_offered';
                return false;""")
        else:
            self.wait("""const image = document.querySelector('img.pa-loom-preview');
                         return image?.naturalWidth > 0 && image.src.startsWith('blob:');""")
            return "decoded"

    def share_and_open(self, base, identity, token):
        current = self.wd("/window")
        if self.js("return !!document.querySelector('.pa-entity-menu:not([open])')"):
            self.click(".pa-entity-menu>summary")
        self.click("[data-share-route]")
        self.top()
        link = self.wait("return document.querySelector('#pai-share-url')?.value")
        assert link.startswith(base + "/#/entities/") and identity in unquote(link) and token not in link
        self.click("[data-share-close]")
        self.frame()
        if self.js("return !!document.querySelector('.pa-entity-menu:not([open])')"):
            self.click(".pa-entity-menu>summary")
        previous = set(self.wd("/window/handles"))
        control = self.wd("/element", {"using": "css selector", "value": "[data-open-route]"})
        element = control["element-6066-11e4-a52e-4f735466cecf"]
        self.wd(f"/element/{element}/click", {})
        self.top()
        for _ in range(60):
            new = set(self.wd("/window/handles")) - previous
            if new:
                break
            time.sleep(.15)
        assert len(new) == 1
        self.wd("/window", {"handle": new.pop()})
        assert self.js("return location.href") == link
        self.connect()
        self.wait("""return [...document.querySelectorAll('#pa-content [data-share-route]')]
                       .some(node => node.dataset.shareRoute === arguments[0]);""", identity)
        self.top()
        self.wd("/window", {"handle": current})
        self.frame()
        return link


def exercise(wd, base, tokens, screenshot, _inactive):
    """Read-only live acceptance without machine-specific data expectations."""
    ui=Workspace(wd);token=tokens.get('owner')
    status=harness.request(base+'/api/status',token=token)
    catalog=harness.request(base+'/api/dashboard/catalog',token=token)
    ui.wd('/url',{'url':base+'/'})
    ui.connect()
    for name,selector in [('Dashboard','[data-resource-panel]'),('Explorer','.pa-finder'),
                          ('Models','[data-model-filter=query]'),('MCP','.pa-loom-mcp')]:
        ui.nav(name);ui.wait('return !!document.querySelector(arguments[0])',selector)
    if screenshot:
        ui.top();screenshot.parent.mkdir(parents=True,exist_ok=True)
        screenshot.write_bytes(base64.b64decode(ui.wd('/screenshot')))
    return {'machine_id':status['machine_id'],'api_revision':status['api_revision'],
            'catalog_entities':len(catalog['entities']),'read_only':True,'current_navigation':True}


if __name__ == '__main__':
    harness.main(exercise)
