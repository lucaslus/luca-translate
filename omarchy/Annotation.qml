import QtQuick
import QtQuick.Layouts
import qs.Commons
import qs.Ui as Ui
import "Strings.js" as Strings

ColumnLayout {
    id: root
    objectName: "annotationView"
    required property var service
    property string tool: "pen"
    property string inkRole: "urgent"
    property var marks: []
    property var draft: null
    property bool exporting: false
    property bool grabbing: false
    readonly property var annotation: service ? service.annotation : null
    readonly property string source: annotation ? "file://" + annotation.path : ""
    readonly property color ink: inkRole === "urgent" ? Color.urgent : inkRole === "accent" ? Color.accent : Color.foreground
    readonly property bool imageReady: image.status === Image.Ready
    signal finished()
    spacing: Style.spacing.md
    function tr(key) { return service ? service.tr(key) : Strings.text(key, "en") }
    onSourceChanged: { marks = []; draft = null; exporting = false; grabbing = false; canvas.requestPaint() }
    onMarksChanged: canvas.requestPaint()
    onDraftChanged: canvas.requestPaint()

    function begin(x, y) {
        if (tool === "text" && !label.text.trim()) { label.forceActiveFocus(); return }
        draft = {tool:tool,ink:ink.toString(),width:Math.max(2, Style.space(3)),fontSize:Math.max(16,Style.font.heading),
            fontFamily:Style.font.family,text:label.text,x:x,y:y,toX:x,toY:y,points:[{x:x,y:y}]}
    }
    function move(x, y) {
        if (!draft) return
        var next = Object.assign({}, draft, {toX:x,toY:y})
        if (tool === "pen") next.points = draft.points.concat([{x:x,y:y}])
        draft = next
    }
    function commit() { if (draft) { marks = marks.concat([draft]); draft = null } }
    function paintMark(context, mark) {
        context.save()
        context.strokeStyle = mark.ink; context.fillStyle = mark.ink
        context.lineWidth = mark.width; context.lineCap = "round"; context.lineJoin = "round"
        context.beginPath()
        if (mark.tool === "pen") {
            mark.points.forEach(function(point, index) { if (index === 0) context.moveTo(point.x,point.y); else context.lineTo(point.x,point.y) })
            if (mark.points.length === 1) context.lineTo(mark.x + 0.1, mark.y + 0.1)
            context.stroke()
        } else if (mark.tool === "rectangle") context.strokeRect(mark.x,mark.y,mark.toX-mark.x,mark.toY-mark.y)
        else if (mark.tool === "ellipse") {
            context.ellipse(Math.min(mark.x,mark.toX),Math.min(mark.y,mark.toY),Math.abs(mark.toX-mark.x),Math.abs(mark.toY-mark.y)); context.stroke()
        } else if (mark.tool === "arrow") {
            var angle = Math.atan2(mark.toY-mark.y, mark.toX-mark.x)
            var size = mark.width * 5
            context.moveTo(mark.x,mark.y); context.lineTo(mark.toX,mark.toY)
            context.moveTo(mark.toX-size*Math.cos(angle-0.5),mark.toY-size*Math.sin(angle-0.5))
            context.lineTo(mark.toX,mark.toY)
            context.lineTo(mark.toX-size*Math.cos(angle+0.5),mark.toY-size*Math.sin(angle+0.5)); context.stroke()
        } else {
            context.font = mark.fontSize + "px " + mark.fontFamily
            context.fillText(mark.text,mark.x,mark.y)
        }
        context.restore()
    }
    function exportImage() {
        if (!imageReady || exporting || !annotation) return
        commit(); exporting = true; canvas.requestPaint()
    }

    Flow {
        Layout.fillWidth: true; spacing: Style.spacing.xs
        Repeater {
            model: ["pen","arrow","rectangle","ellipse","textTool"]
            Ui.Button {
                required property string modelData
                readonly property string toolName: modelData === "textTool" ? "text" : modelData
                objectName: "annotation.tool." + toolName
                text: root.tr(modelData); selected: root.tool === toolName; focusable: true
                onClicked: root.tool = toolName
            }
        }
    }
    RowLayout {
        Layout.fillWidth: true
        Ui.Dropdown {
            Layout.fillWidth: true; showLabel: false; value: root.inkRole
            options: [{value:"urgent",label:root.tr("urgent")},{value:"accent",label:root.tr("accent")},{value:"foreground",label:root.tr("ink")}]
            onChanged: function(value) { root.inkRole = value }
        }
        Ui.Button { objectName: "annotation.undo"; text: root.tr("undo"); focusable: true; enabled: root.marks.length > 0; onClicked: root.marks = root.marks.slice(0,-1) }
        Ui.Button { objectName: "annotation.clear"; text: root.tr("clear"); focusable: true; onClicked: root.marks = [] }
    }
    Ui.TextField { id: label; objectName: "annotation.label"; visible: root.tool === "text"; placeholderText: root.tr("annotationText"); Layout.fillWidth: true }
    Item {
        id: viewport
        Layout.fillWidth: true; Layout.fillHeight: true; clip: true
        Image {
            id: image
            visible: false; source: root.source; cache: false
            onStatusChanged: if (status === Image.Ready) canvas.requestPaint()
        }
        Item {
            width: image.sourceSize.width || 1; height: image.sourceSize.height || 1
            readonly property real fit: Math.min(viewport.width/width, viewport.height/height)
            scale: Math.min(1,fit)
            anchors.centerIn: parent
            Canvas {
                id: canvas
                anchors.fill: parent
                renderTarget: Canvas.Image
                onPaint: {
                    if (!root.imageReady) return
                    var context = getContext("2d")
                    context.reset(); context.clearRect(0,0,width,height)
                    context.drawImage(image,0,0,width,height)
                    root.marks.forEach(function(mark) { root.paintMark(context,mark) })
                    if (root.draft) root.paintMark(context,root.draft)
                }
                onPainted: {
                    if (!root.exporting || root.grabbing || !root.annotation) return
                    root.grabbing = true
                    var annotation = root.annotation
                    var started = canvas.grabToImage(function(result) {
                        root.exporting = false; root.grabbing = false
                        if (!root.annotation || root.annotation.annotation_id !== annotation.annotation_id) return
                        if (!result.saveToFile(annotation.output_path)) { root.service.error = root.tr("exportFailed"); return }
                        root.service.request("annotation.copy", {annotation_id:annotation.annotation_id}, function(ok) {
                            if (ok) { root.service.annotation = null; root.service.notice = root.tr("copied"); root.finished() }
                        })
                    }, Qt.size(canvas.width, canvas.height))
                    if (!started) { root.exporting = false; root.grabbing = false; root.service.error = root.tr("exportFailed") }
                }
                MouseArea {
                    objectName: "annotation.draw"
                    anchors.fill: parent; cursorShape: Qt.CrossCursor; enabled: root.imageReady && !root.exporting
                    onPressed: function(mouse) { root.begin(mouse.x,mouse.y) }
                    onPositionChanged: function(mouse) { if (pressed) root.move(mouse.x,mouse.y) }
                    onReleased: root.commit()
                    onCanceled: root.draft = null
                }
            }
        }
    }
    RowLayout {
        Layout.fillWidth: true
        Ui.Button { objectName: "annotation.cancel"; text: root.tr("cancel"); focusable: true; onClicked: root.finished() }
        Item { Layout.fillWidth: true }
        Ui.Button { objectName: "annotation.copy"; text: root.tr("copy"); selected: true; focusable: true; enabled: root.imageReady && !root.exporting; onClicked: root.exportImage() }
    }
}
