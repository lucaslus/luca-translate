import QtQuick
import qs.Commons
import qs.Ui as Ui
import "Theme.js" as Theme

Ui.TextField {
    foreground: Color.popups.text
    placeholderTextColor: Theme.secondary(Color.muted, foreground,
        Theme.over(Style.controlFill(activeFocus, hovered, foreground, accent), Color.popups.background))
}
