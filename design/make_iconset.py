#!/usr/bin/env python3
"""从选中的「分割桥环」生成全套图标：icns / png 全尺寸 / ico / 托盘 template 图"""
from PIL import Image, ImageDraw
import os, subprocess

SRC = "/Users/lucas/Lushuo/lucas-translate/design/icons/2-分割桥环.png"
ICONS = "/Users/lucas/Lushuo/lucas-translate/app/src-tauri/icons"
os.makedirs(ICONS, exist_ok=True)
img = Image.open(SRC).convert("RGBA")

# ---- 1. icon.png (1024) ----
img.save(f"{ICONS}/icon.png")

# ---- 2. mac iconset -> icns ----
sizes = [16, 32, 64, 128, 256, 512, 1024]
iconset = "/tmp/lucas.iconset"
os.makedirs(iconset, exist_ok=True)
for s in sizes:
    im = img.resize((s, s), Image.LANCZOS)
    im.save(f"{iconset}/icon_{s}x{s}.png")
    if s <= 512:
        im2 = img.resize((s*2, s*2), Image.LANCZOS) if s*2 <= 1024 else img
        im2.save(f"{iconset}/icon_{s}x{s}@2x.png")
subprocess.run(["iconutil", "-c", "icns", iconset, "-o", f"{ICONS}/icon.icns"], check=True)

# ---- 3. 常规 png / ico ----
img.resize((32, 32), Image.LANCZOS).save(f"{ICONS}/32x32.png")
img.resize((128, 128), Image.LANCZOS).save(f"{ICONS}/128x128.png")
img.resize((256, 256), Image.LANCZOS).save(f"{ICONS}/128x128@2x.png")
img.resize((256, 256), Image.LANCZOS).save(f"{ICONS}/icon.ico", sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])

# ---- 4. 托盘 template 图：只有圆环 + 两点，纯黑 alpha ----
# 44pt 逻辑尺寸，2x 输出 88px；macOS template 图自动适配深浅菜单栏
T = 88
tray = Image.new("RGBA", (T, T), (0, 0, 0, 0))
d = ImageDraw.Draw(tray)
cx, cy = T // 2, T // 2
ring_r = 24          # 圆环半径
stroke = 7
d.ellipse([cx-ring_r, cy-ring_r, cx+ring_r, cy+ring_r], outline=(0, 0, 0, 255), width=stroke)
dot_r = 8
for dx in (-38, 38):  # 两侧圆点
    d.ellipse([cx+dx-dot_r, cy-dot_r, cx+dx+dot_r, cy+dot_r], fill=(0, 0, 0, 255))
tray.save(f"{ICONS}/tray.png")
print("icons generated:", os.listdir(ICONS))
