import QtQuick
import qs.Commons

Canvas {
    id: root
    property string name: ""
    property bool filled: false
    property color foreground: Color.popups.text
    implicitWidth: Style.font.icon
    implicitHeight: implicitWidth
    onNameChanged: requestPaint()
    onFilledChanged: requestPaint()
    onForegroundChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()
    onPaint: {
        var c = getContext("2d")
        c.reset(); c.scale(width / 24, height / 24)
        c.strokeStyle = foreground; c.lineWidth = 1.7; c.lineCap = "round"; c.lineJoin = "round"
        function polygon(points) {
            var xs = points.map(function(p) { return p[0] }), ys = points.map(function(p) { return p[1] })
            var left = Math.min.apply(null,xs), top = Math.min.apply(null,ys)
            var w = Math.max.apply(null,xs)-left, h = Math.max.apply(null,ys)-top
            points.forEach(function(p,index) {
                var x = 2+(p[0]-left)*20/w, y = 2+(p[1]-top)*20/h
                if (!index) c.moveTo(x,y); else c.lineTo(x,y)
            })
            c.closePath()
        }
        c.beginPath()
        if (name === "translate") {
            c.moveTo(2,7); c.lineTo(22,7); c.lineTo(17,2)
            c.moveTo(22,17); c.lineTo(2,17); c.lineTo(7,22)
        } else if (name === "history") {
            c.arc(12,12,10,0,Math.PI*2)
            c.moveTo(12,6); c.lineTo(12,12); c.lineTo(17,15)
        } else if (name === "favorites") {
            var star = []
            for (var i=0;i<10;i++) {
                var angle=-Math.PI/2+i*Math.PI/5, radius=i%2 ? 4 : 9
                star.push([radius*Math.cos(angle),radius*Math.sin(angle)])
            }
            polygon(star)
            if (filled) { c.fillStyle = foreground; c.fill() }
        } else if (name === "settings") {
            var gear = []
            for (var j=0;j<32;j++) {
                var a=-Math.PI/2+j*Math.PI/16, r=j%4===0 || j%4===3 ? 7 : 9
                gear.push([r*Math.cos(a),r*Math.sin(a)])
            }
            polygon(gear); c.moveTo(15.5,12); c.arc(12,12,3.5,0,Math.PI*2)
        } else if (name === "close") {
            c.moveTo(2,2); c.lineTo(22,22); c.moveTo(22,2); c.lineTo(2,22)
        } else if (name === "copy") {
            c.rect(3,8,13,13)
            c.moveTo(8,8); c.lineTo(8,3); c.lineTo(21,3); c.lineTo(21,16); c.lineTo(16,16)
        } else if (name === "down") {
            c.moveTo(12,3); c.lineTo(12,21)
            c.moveTo(3,12); c.lineTo(12,21); c.lineTo(21,12)
        } else if (name === "clear") {
            c.moveTo(14,10); c.lineTo(21,3)
            c.moveTo(8,10); c.lineTo(14,10); c.lineTo(18,14); c.lineTo(12,21); c.lineTo(3,16); c.closePath()
            c.moveTo(8,10); c.lineTo(18,14)
            c.moveTo(10,14); c.lineTo(6,18); c.moveTo(14,16); c.lineTo(10,20)
        } else if (name === "send") {
            c.moveTo(3,3); c.lineTo(21,12); c.lineTo(3,21); c.lineTo(6,12); c.closePath()
            c.moveTo(6,12); c.lineTo(21,12)
        } else if (name === "stop") {
            c.rect(3,3,18,18)
        }
        c.stroke()
    }
}
