"""Compare actual painted icon bounds, excluding transparent padding."""
import json
from pathlib import Path
import struct
import zlib


def ink_bounds(path):
    data=path.read_bytes()
    assert data[:8]==b'\x89PNG\r\n\x1a\n'
    compressed=bytearray(); offset=8
    while offset<len(data):
        size=struct.unpack_from('>I',data,offset)[0]
        kind=data[offset+4:offset+8]; body=data[offset+8:offset+8+size]
        if kind==b'IHDR':
            width,height,depth,color,_,_,interlace=struct.unpack('>IIBBBBB',body)
            assert depth==8 and color==6 and interlace==0, 'Expected Qt RGBA PNG'
        if kind==b'IDAT': compressed.extend(body)
        offset+=size+12
    raw=zlib.decompress(compressed); stride=width*4; previous=bytearray(stride)
    points=[]
    for y in range(height):
        start=y*(stride+1); filter_type=raw[start]; row=bytearray(raw[start+1:start+1+stride])
        for index in range(stride):
            left=row[index-4] if index>=4 else 0
            above=previous[index]; corner=previous[index-4] if index>=4 else 0
            if filter_type==0: predictor=0
            elif filter_type==1: predictor=left
            elif filter_type==2: predictor=above
            elif filter_type==3: predictor=(left+above)//2
            elif filter_type==4:
                p=left+above-corner; distances=[abs(p-left),abs(p-above),abs(p-corner)]
                predictor=[left,above,corner][distances.index(min(distances))]
            else: raise AssertionError('Unknown PNG filter')
            row[index]=(row[index]+predictor)&255
        points.extend((x,y) for x in range(width) if row[x*4+3]>48)
        previous=row
    assert points and len(points)<width*height*.65, f'Empty or opaque icon: {path.name}'
    xs,ys=zip(*points)
    return {'width':max(xs)-min(xs)+1,'height':max(ys)-min(ys)+1}


def verify(directory):
    bounds={name:ink_bounds(directory/f'icon-{name}.png') for name in ['translate','history','favorites','settings','close','copy','clear','send','stop']}
    for axis in ['width','height']:
        values=[size[axis] for size in bounds.values()]
        assert max(values)-min(values)<=1, f'Icon ink sizes differ: {bounds}'
    print('NATIVE_ICON_INK_PASS '+json.dumps(bounds))
    return bounds
