import sys

html = '<div id="app"><video id="player" width="1280" height="720"></video></div>'
js = 'let video = document.getElementById(\'player\'); let ms = new MediaSource(); video.src = URL.createObjectURL(ms); let sb = ms.addSourceBuffer(\'video/mp4; codecs="avc1.42E01E"\'); let dummyH264 = new ArrayBuffer(1024 * 1024); sb.appendBuffer(dummyH264);'

def fmt(arr):
    out = ""
    for i, b in enumerate(arr):
        if i % 16 == 0:
            out += "\n        "
        out += str(b) + ", "
    return out

print(fmt([ord(c) for c in html]))
print(fmt([ord(c) for c in js]))
