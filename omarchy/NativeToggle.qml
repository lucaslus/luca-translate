import QtQuick
import qs.Commons
import qs.Ui as Ui
import "Typography.js" as Typography

Ui.Toggle {
    fontFamily: Typography.family
    id: root
    property bool rowHovered: false
    foreground: Color.popups.text
    titleSize: Style.font.body
    implicitHeight: Style.spacing.controlHeight + Style.spacing.md
    color: Style.controlFill(activeFocus, rowHovered, foreground, accent)
    borderSpec: activeFocus || rowHovered
        ? Border.controlSpec(activeFocus ? "focus" : "hover-cursor", foreground, accent) : Border.none()
    onHovered: function(hovered) { rowHovered = hovered }
}
