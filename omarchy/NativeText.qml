import QtQuick
import qs.Commons
import "Theme.js" as Theme

Text {
    textFormat: Text.PlainText
    property bool secondary: false
    color: secondary ? Theme.secondary(Color.muted, Color.popups.text, Color.popups.background) : Color.popups.text
    font.family: Style.font.family
    font.pixelSize: Style.font.body
    wrapMode: Text.Wrap
}
