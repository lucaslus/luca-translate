import QtQuick
import QtQuick.Window
import Quickshell
import qs.Commons
import "Native" as Native

ShellRoot {
    id: root
    property int remaining: 9
    property Window window: Window {
        width: 200; height: 40; visible: true; color: "transparent"
        Row {
            Repeater {
                id: icons
                model: ["translate", "history", "favorites", "settings", "close", "copy", "clear", "send", "stop"]
                Native.NativeIcon {
                    required property string modelData
                    name: modelData
                    width: Style.font.icon + Style.space(2); height: width
                }
            }
        }
    }
    Timer {
        interval: 250; running: true
        onTriggered: {
            for (var index=0;index<icons.count;index++) {
                var icon=icons.itemAt(index)
                exportIcon(icon)
            }
        }
    }
    function exportIcon(icon) {
        icon.grabToImage(function(result) {
            if (!result.saveToFile(Quickshell.env("NATIVE_PREVIEW_DIR")+"/icon-"+icon.name+".png")) { console.error("NATIVE_ICON_FAIL"); Qt.exit(1); return }
            if (--remaining===0) { console.log("NATIVE_ICON_PASS"); Qt.quit() }
        })
    }
}
