import QtQuick
import qs.Commons

Item {
    id: root
    property string label: ""
    property color foreground: Color.popups.text
    readonly property bool running: visible
    implicitWidth: Style.font.icon
    implicitHeight: implicitWidth
    Accessible.name: label

    Canvas {
        id: arc
        anchors.fill: parent
        onPaint: {
            var context = getContext("2d")
            context.reset()
            context.scale(width / 24, height / 24)
            context.strokeStyle = root.foreground
            context.lineWidth = 2
            context.lineCap = "round"
            context.beginPath()
            context.arc(12, 12, 8, -Math.PI / 2, Math.PI)
            context.stroke()
        }
        Connections {
            target: root
            function onForegroundChanged() { arc.requestPaint() }
        }
    }
    NumberAnimation on rotation {
        from: 0; to: 360; duration: 900
        loops: Animation.Infinite
        running: root.running
    }
}
