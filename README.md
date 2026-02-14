# StreamBridge

**Quick and dirty NDI preview in the browser.**

NDI is excellent for moving video around a network. But sometimes you just want to glance at a feed from your browser — no dedicated monitor, no NDI Tools, no installs on the viewing device.

StreamBridge picks up NDI sources on your network and streams them to any browser as JPEG frames over WebSocket. Run the server, open the page, click a source, see video.

## Good fit

- Checking what's on air from your laptop or phone
- Dropping a live preview into a web-based control panel
- Letting non-technical team members see camera feeds without installing anything
- Quick monitoring of multiple sources from any device on the network

## Not the right tool

- Broadcast-quality or frame-accurate playout — use native NDI for that
- Audio — video only for now
- Large-scale routing — this is a simple bridge, not a router

## Requirements

- [NDI 6 Runtime](https://ndi.video/tools/) on the machine running StreamBridge
- Currently tested on Windows

The built-in page at `http://localhost:9550` has live preview, API docs, and a code example.

---

NDI is a registered trademark of Vizrt NDI AB. [ndi.video](https://ndi.video)
