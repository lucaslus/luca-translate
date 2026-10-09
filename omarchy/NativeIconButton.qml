import QtQuick
import qs.Commons

NativeButton {
    id: root
    property string name: ""
    property bool filled: false
    foreground: Color.popups.text
    focusable: true
    implicitHeight: Math.max(Style.spacing.controlHeight, Style.font.icon + Style.spacing.xs * 2)
    implicitWidth: implicitHeight
    Accessible.name: tooltipText
    NativeIcon {
        objectName: "toolbarIcon." + root.name
        anchors.centerIn: parent
        width: Style.font.icon + Style.space(2); height: width
        name: root.name; filled: root.filled; foreground: root.foreground
    }
}
