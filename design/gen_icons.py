#!/usr/bin/env python3
"""lucas-translate 图标方案生成器：6 个方向，4x 超采样绘制后缩至 1024。"""
from PIL import Image, ImageDraw, ImageFilter
import math, os

OUT = "/Users/lucas/Lushuo/lucas-translate/design/icons"
os.makedirs(OUT, exist_ok=True)
SS = 4                       # 超采样倍数
S = 1024 * SS                # 工作画布
M = int((1024 - 824) / 2) * SS   # 内容区边距
R = int(185 * SS)            # squircle 圆角

def lerp(a, b, t): return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))

def vgrad(w, h, c1, c2):
    img = Image.new("RGB", (w, h))
    d = ImageDraw.Draw(img)
    for y in range(h):
        d.line([(0, y), (w, y)], fill=lerp(c1, c2, y / h))
    return img

def base(c1, c2):
    """生成 macOS squircle 底板（渐变 + 内高光 + 外阴影），返回 RGBA 画布"""
    # 外阴影
    shadow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    sd = ImageDraw.Draw(shadow)
    sd.rounded_rectangle([M, M + int(24*SS), M + 824*SS, M + 824*SS + int(24*SS)], radius=R, fill=(0, 0, 0, 110))
    shadow = shadow.filter(ImageFilter.GaussianBlur(18 * SS))
    canvas = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    canvas.alpha_composite(shadow)
    # 渐变底板
    bg = vgrad(S, S, c1, c2).convert("RGBA")
    mask = Image.new("L", (S, S), 0)
    md = ImageDraw.Draw(mask)
    md.rounded_rectangle([M, M, M + 824*SS, M + 824*SS], radius=R, fill=255)
    canvas.paste(bg, (0, 0), mask)
    # 顶部高光（浅色半透明覆盖上半部）
    hl = vgrad(S, S, (255, 255, 255), (255, 255, 255))
    hl.putalpha(0)
    had = hl.load()
    for y in range(M, M + 824*SS):
        t = (y - M) / (824*SS)
        a = int(46 * (1 - t) ** 1.6)
        for_x = Image.new("L", (1, 1))
        # 逐行填充太慢，改用 paste
    # 用更快的实现：整块渐变 alpha
    alpha = Image.new("L", (S, S), 0)
    ad = ImageDraw.Draw(alpha)
    for y in range(M, M + 824*SS):
        t = (y - M) / (824*SS)
        ad.line([(M, y), (M + 824*SS, y)], fill=int(44 * (1 - t) ** 1.7))
    white = Image.new("RGBA", (S, S), (255, 255, 255, 255))
    white.putalpha(alpha)
    canvas.paste(white, (0, 0), Image.composite(alpha, Image.new("L",(S,S),0), mask))
    return canvas

def finish(canvas, name):
    img = canvas.resize((1024, 1024), Image.LANCZOS).convert("RGBA")
    img.save(f"{OUT}/{name}.png")
    print("saved", name)

C = SS  # 简写
CX, CY = S//2, S//2

# ============ 1. 对话 · 双气泡 ============
c = base((255, 179, 92), (240, 101, 79))
d = ImageDraw.Draw(c, "RGBA")
# 后面气泡（描边，右下）
bw = 26 * SS
d.ellipse([CX - 40*SS, CY - 130*SS, CX + 330*SS, CY + 200*SS], outline=(255, 255, 255, 200), width=bw)
d.polygon([(CX + 40*SS, CY + 170*SS), (CX - 10*SS, CY + 290*SS), (CX + 130*SS, CY + 190*SS)], fill=(255, 255, 255, 200))
# 前面气泡（实心，左上）
d.ellipse([CX - 330*SS, CY - 300*SS, CX + 60*SS, CY + 60*SS], fill=(255, 255, 255, 255))
d.polygon([(CX - 200*SS, CY + 30*SS), (CX - 260*SS, CY + 160*SS), (CX - 80*SS, CY + 55*SS)], fill=(255, 255, 255, 255))
# 前气泡里的两条"文字线"（用底色系）
d.rounded_rectangle([CX - 250*SS, CY - 200*SS, CX - 20*SS, CY - 168*SS], radius=16*SS, fill=(240, 130, 90, 255))
d.rounded_rectangle([CX - 250*SS, CY - 130*SS, CX - 90*SS, CY - 98*SS], radius=16*SS, fill=(245, 160, 110, 255))
finish(c, "1-对话气泡")

# ============ 2. 桥 · 对角分割 ============
c = Image.new("RGBA", (S, S), (0, 0, 0, 0))
sd = ImageDraw.Draw(c)
sd.rounded_rectangle([M, M+24*SS, M+824*SS, M+824*SS+24*SS], radius=R, fill=(0,0,0,110))
c = c.filter(ImageFilter.GaussianBlur(18*SS))
clip = Image.new("L", (S, S), 0)
cd = ImageDraw.Draw(clip)
cd.rounded_rectangle([M, M, M+824*SS, M+824*SS], radius=R, fill=255)
warm = vgrad(S, S, (255, 179, 92), (240, 101, 79)).convert("RGBA")
cool = vgrad(S, S, (62, 99, 216), (39, 67, 166)).convert("RGBA")
layer = Image.new("RGBA", (S, S), (0,0,0,0))
layer.paste(cool, (0,0), clip)
# 对角分割：右上半暖色
poly_mask = Image.new("L", (S, S), 0)
pm = ImageDraw.Draw(poly_mask)
pm.polygon([(M, M), (M+824*SS, M), (M+824*SS, M+824*SS), (M - 200*SS, M - 100*SS)], fill=255)
pm = poly_mask.point(lambda v: v)
layer.paste(warm, (0,0), Image.composite(poly_mask, Image.new("L",(S,S),0), clip))
c.alpha_composite(layer)
d = ImageDraw.Draw(c, "RGBA")
# 中央白色圆环桥
d.ellipse([CX-190*SS, CY-190*SS, CX+190*SS, CY+190*SS], outline=(255,255,255,255), width=34*SS)
# 左右两枚色点（两种语言）被桥环扣住
d.ellipse([CX-320*SS, CY-52*SS, CX-216*SS, CY+52*SS], fill=(255,255,255,235))
d.ellipse([CX+216*SS, CY-52*SS, CX+320*SS, CY+52*SS], fill=(255,255,255,235))
finish(c, "2-分割桥环")

# ============ 3. 透镜 · 抽象 ============
c = base((47, 53, 105), (31, 27, 72))
d = ImageDraw.Draw(c, "RGBA")
# 外圈玻璃环
d.ellipse([CX-250*SS, CY-250*SS, CX+250*SS, CY+250*SS], outline=(255,255,255,90), width=18*SS)
# 玻璃内部：柔和径向亮斑
glow = Image.new("RGBA", (S, S), (0,0,0,0))
gd = ImageDraw.Draw(glow)
gd.ellipse([CX-200*SS, CY-200*SS, CX+200*SS, CY+200*SS], fill=(120, 160, 255, 70))
glow = glow.filter(ImageFilter.GaussianBlur(40*SS))
c.alpha_composite(glow)
d.ellipse([CX-250*SS, CY-250*SS, CX+250*SS, CY+250*SS], outline=(255,255,255,120), width=10*SS)
# 双色光点（两种语言在透镜中交汇）
d.ellipse([CX-120*SS, CY-60*SS, CX-16*SS, CY+44*SS], fill=(255, 170, 80, 255))
d.ellipse([CX+16*SS, CY-44*SS, CX+120*SS, CY+60*SS], fill=(90, 200, 250, 255))
# 高光弧
d.arc([CX-230*SS, CY-230*SS, CX+230*SS, CY+230*SS], start=200, end=290, fill=(255,255,255,220), width=14*SS)
finish(c, "3-透镜交汇")

# ============ 4. 小人物 · 吉祥物机器人 ============
c = base((120, 220, 190), (47, 169, 140))
d = ImageDraw.Draw(c, "RGBA")
# 天线
d.line([(CX, CY-250*SS), (CX, CY-170*SS)], fill=(255,255,255,230), width=14*SS)
d.ellipse([CX-26*SS, CY-302*SS, CX+26*SS, CY-250*SS], fill=(255, 209, 102, 255))
# 头（圆角矩形）
d.rounded_rectangle([CX-210*SS, CY-170*SS, CX+210*SS, CY+230*SS], radius=120*SS, fill=(255,255,255,255))
# 眼睛（两块"文字"屏幕）
d.rounded_rectangle([CX-140*SS, CY-70*SS, CX-40*SS, CY+50*SS], radius=28*SS, fill=(45, 55, 72, 255))
d.rounded_rectangle([CX+40*SS, CY-70*SS, CX+140*SS, CY+50*SS], radius=28*SS, fill=(45, 55, 72, 255))
# 眼睛高光
d.ellipse([CX-120*SS, CY-50*SS, CX-84*SS, CY-14*SS], fill=(255,255,255,230))
d.ellipse([CX+60*SS, CY-50*SS, CX+96*SS, CY-14*SS], fill=(255,255,255,230))
# 微笑
d.arc([CX-70*SS, CY+90*SS, CX+70*SS, CY+190*SS], start=15, end=165, fill=(45,55,72,255), width=14*SS)
# 腮红
d.ellipse([CX-180*SS, CY+80*SS, CX-130*SS, CY+120*SS], fill=(255, 170, 150, 160))
d.ellipse([CX+130*SS, CY+80*SS, CX+180*SS, CY+120*SS], fill=(255, 170, 150, 160))
finish(c, "4-小机器人")

# ============ 5. 波纹 · 声波图案 ============
c = base((251, 247, 240), (238, 229, 214))
d = ImageDraw.Draw(c, "RGBA")
colors = [(240, 130, 90), (232, 120, 150), (90, 150, 240)]
for i, col in enumerate(colors):
    y0 = CY - 170*SS + i * 170*SS
    amp = (52 - i * 12) * SS
    pts = []
    for x in range(M + 60*SS, M + 764*SS, 4*SS):
        t = (x - (M + 60*SS)) / (704*SS)
        yy = y0 + amp * math.sin(t * math.pi * (2.2 + i * 0.5) - 0.4)
        pts.append((x, yy))
    d.line(pts, fill=col + (255,), width=(30 - i * 6) * SS, joint="curve")
    # 线端圆点
    d.ellipse([pts[0][0]-14*SS, pts[0][1]-14*SS, pts[0][0]+14*SS, pts[0][1]+14*SS], fill=col + (255,))
finish(c, "5-声波纹")

# ============ 6. 咬合 · 几何互换 ============
c = base((90, 98, 130), (58, 64, 92))
d = ImageDraw.Draw(c, "RGBA")
# 左：橙色圆（被右侧咬掉一块）
orange = Image.new("RGBA", (S, S), (0,0,0,0))
od = ImageDraw.Draw(orange)
od.ellipse([CX-300*SS, CY-240*SS, CX+120*SS, CY+180*SS], fill=(255, 150, 80, 255))
hole = Image.new("L", (S, S), 0)
hd = ImageDraw.Draw(hole)
hd.ellipse([CX-40*SS, CY-160*SS, CX+240*SS, CY+120*SS], fill=255)
orange.putalpha(Image.composite(Image.new("L",(S,S),0), orange.split()[3], hole))
c.alpha_composite(orange)
# 右：蓝色圆环（咬合进橙色缺口）
blue = Image.new("RGBA", (S, S), (0,0,0,0))
bd = ImageDraw.Draw(blue)
bd.ellipse([CX-80*SS, CY-120*SS, CX+320*SS, CY+280*SS], fill=(110, 160, 255, 255))
hole2 = Image.new("L", (S, S), 0)
h2d = ImageDraw.Draw(hole2)
h2d.ellipse([CX+10*SS, CY-50*SS, CX+230*SS, CY+170*SS], fill=255)
blue.putalpha(Image.composite(Image.new("L",(S,S),0), blue.split()[3], hole2))
c.alpha_composite(blue)
# 咬合处的白色月牙缝
d.ellipse([CX-40*SS, CY-160*SS, CX+240*SS, CY+120*SS], outline=(255,255,255,255), width=12*SS)
finish(c, "6-咬合几何")

# ============ 合成对比图 ============
names = ["1-对话气泡","2-分割桥环","3-透镜交汇","4-小机器人","5-声波纹","6-咬合几何"]
sheet = Image.new("RGB", (1024*3 + 40*4, 1024*2 + 40*3), (245, 245, 247))
for i, n in enumerate(names):
    img = Image.open(f"{OUT}/{n}.png")
    x = 40 + (i % 3) * (1024 + 40)
    y = 40 + (i // 3) * (1024 + 40)
    sheet.paste(img, (x, y), img)
sheet = sheet.resize((sheet.width//2, sheet.height//2), Image.LANCZOS)
sheet.save(f"{OUT}/_all.png")
print("contact sheet saved")
