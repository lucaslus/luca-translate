import QtQuick
import QtQuick.Window
import Quickshell
import qs.Commons
import "Native" as Native
import "Native/Strings.js" as Strings

ShellRoot {
    id: root
    property QtObject fake: QtObject {
        property var annotation: ({annotation_id:"fixture",path:Quickshell.env("NATIVE_TEST_IMAGE"),output_path:Quickshell.env("NATIVE_TEST_OUTPUT")})
        property string error: ""
        onErrorChanged: if (error) { console.error("NATIVE_ANNOTATION_FAIL: " + error); Qt.exit(1) }
        property string notice: ""
        function tr(key) { return Strings.text(key,"en") }
        function request(method, params, callback) {
            if (method !== "annotation.copy" || params.annotation_id !== "fixture") { console.error("NATIVE_ANNOTATION_FAIL: bad export"); Qt.exit(1); return }
            callback(true,{copied:true})
        }
    }
    property Window window: Window {
        width: 560; height: 740; visible: true
        color: Color.popups.background
        Native.Annotation {
            id: editor
            anchors.fill: parent
            service: root.fake
            onFinished: { console.log("NATIVE_ANNOTATION_PASS"); Qt.quit() }
        }
    }
    Timer {
        running: true; interval: 500
        onTriggered: {
            if (!editor.imageReady) { console.error("NATIVE_ANNOTATION_FAIL: image did not load"); Qt.exit(1); return }
            editor.tool = "rectangle"; editor.begin(10,10); editor.move(100,80); editor.commit()
            editor.tool = "arrow"; editor.begin(15,80); editor.move(100,20); editor.commit()
            editor.exportImage()
        }
    }
    Timer { running: true; interval: 5000; onTriggered: { console.error("NATIVE_ANNOTATION_FAIL: export timed out"); Qt.exit(1) } }
}
