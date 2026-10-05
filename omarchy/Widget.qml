import QtQuick
import qs.Commons
import qs.Ui as Ui

Ui.BarWidget {
    id: root
    property var shell: null
    function show(action) {
        var api = root.shell || (root.bar ? root.bar.shell : null)
        if (api) api.summon("lucas.translate", JSON.stringify({action:action}))
    }
    implicitWidth: button.implicitWidth
    implicitHeight: button.implicitHeight
    Ui.BarIconButton {
        id: button
        bar: root.bar
        text: "󰊿"
        tooltipText: "Lucas Translate"
        onPressed: function(button) { root.show(button === Qt.RightButton ? "selection" : "show") }
    }
}
