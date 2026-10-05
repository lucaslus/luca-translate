import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qs.Commons
import qs.Ui as Ui
import "Strings.js" as Strings

Flickable {
    id: root
    objectName: "settingsView"
    required property var service
    property var draftShortcuts: ({})
    property bool saving: false
    property bool clearAiKey: false
    property bool clearOfficialKey: false
    property bool officialDirty: false
    property bool routingDirty: false
    property var routingRules: []
    property string routingFallback: "zh-Hans"
    property var logStatus: null
    readonly property var document: service ? service.settings : ({})
    readonly property var languages: Strings.languages(service ? service.language : "en").filter(function(lang) { return lang.value !== "auto" })
    contentHeight: column.implicitHeight
    boundsBehavior: Flickable.StopAtBounds; clip: true
    ScrollBar.vertical: NativeScrollBar {}
    function tr(key) { return service ? service.tr(key) : Strings.text(key, "en") }
    function loadRouting() {
        if (routingDirty || !document.routing) return
        routingRules = document.routing.rules.map(function(rule) { return {from:rule.from, to:rule.to} })
        routingFallback = document.routing.fallback
    }
    function changeRule(index, key, value) {
        var rules = routingRules.map(function(rule) { return Object.assign({},rule) })
        rules[index][key] = value; routingDirty = true; routingRules = rules
    }
    function addRule() {
        var next = languages.find(function(lang) { return !routingRules.some(function(rule) { return rule.from === lang.value }) })
        if (!next) return
        var target = routingFallback === next.value ? (next.value === "en" ? "zh-Hans" : "en") : routingFallback
        routingDirty = true; routingRules = routingRules.concat([{from:next.value,to:target}])
    }
    function removeRule(index) { routingDirty = true; routingRules = routingRules.filter(function(_, row) { return row !== index }) }
    function moveRule(index, step) {
        var rules = routingRules.slice(); var rule = rules.splice(index,1)[0]
        rules.splice(index+step,0,rule); routingDirty = true; routingRules = rules
    }
    function refreshDiagnostics() {
        if (service && service.ready) service.request("diagnostics", {}, function(ok, data) { if (ok) root.logStatus = data })
    }
    onDocumentChanged: loadRouting()
    Component.onCompleted: loadRouting()
    onVisibleChanged: if (visible && service && service.ready) { service.refreshSettings(); service.refreshShortcuts(); refreshDiagnostics() }
    Connections {
        target: root.service
        function onShortcutsChanged() { root.draftShortcuts = Object.assign({}, root.service.shortcuts.shortcuts) }
        function onReadyChanged() { if (root.visible && root.service.ready) root.refreshDiagnostics() }
    }

    ColumnLayout {
        id: column
        width: root.width; spacing: Style.spacing.md
        NativeText { text: root.tr("theme"); color: Color.muted; Layout.fillWidth: true }
        NativeText { text: root.tr("shellManaged"); color: Color.muted; Layout.fillWidth: true }
        Ui.Dropdown {
            objectName: "interfaceLanguage"
            Layout.fillWidth: true; label: root.tr("language")
            value: root.document.preferences ? root.document.preferences.language : "auto"
            options: [{value:"auto",label:root.tr("system")},{value:"en",label:"English"},{value:"zh-CN",label:"简体中文"}]
            onChanged: function(value) { root.service.save("preferences.save", {language:value}) }
        }
        Ui.PanelSectionHeader { text: root.tr("services"); Layout.fillWidth: true }
        Repeater {
            model: [{id:"youdao",label:"Youdao / 有道"},{id:"bing",label:"Bing"},{id:"deepl",label:"DeepL"},{id:"google",label:"Google"}]
            Ui.Toggle {
                required property var modelData
                objectName: "service." + modelData.id
                label: modelData.label; Layout.fillWidth: true
                checked: root.document.services ? root.document.services[modelData.id] === true : false
                onClicked: root.service.save("service.set", {id:modelData.id,enabled:!checked})
            }
        }
        Ui.PanelSectionHeader { text: root.tr("routing"); Layout.fillWidth: true }
        NativeText { text: root.tr("routingHelp"); color: Color.muted; Layout.fillWidth: true }
        Repeater {
            model: root.routingRules
            ColumnLayout {
                id: route
                required property var modelData
                required property int index
                Layout.fillWidth: true; spacing: Style.spacing.xs
                RowLayout {
                    Layout.fillWidth: true
                    Ui.Dropdown {
                        objectName: "rule.from." + route.index
                        label: root.tr("source"); Layout.fillWidth: true; options: root.languages; value: route.modelData.from
                        onChanged: function(value) { root.changeRule(route.index,"from",value) }
                    }
                    NativeText { text: "→" }
                    Ui.Dropdown {
                        objectName: "rule.to." + route.index
                        label: root.tr("target"); Layout.fillWidth: true; options: root.languages; value: route.modelData.to
                        onChanged: function(value) { root.changeRule(route.index,"to",value) }
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    Ui.Button { objectName: "rule.up." + route.index; text: root.tr("moveUp"); focusable: true; enabled: route.index > 0; onClicked: root.moveRule(route.index,-1) }
                    Ui.Button { objectName: "rule.down." + route.index; text: root.tr("moveDown"); focusable: true; enabled: route.index < root.routingRules.length-1; onClicked: root.moveRule(route.index,1) }
                    Item { Layout.fillWidth: true }
                    Ui.Button { objectName: "rule.remove." + route.index; text: root.tr("remove"); focusable: true; onClicked: root.removeRule(route.index) }
                }
            }
        }
        Ui.Button { objectName: "rule.add"; text: root.tr("addRule"); focusable: true; enabled: root.routingRules.length < root.languages.length; onClicked: root.addRule() }
        Ui.Dropdown {
            label: root.tr("fallback"); Layout.fillWidth: true; options: root.languages; value: root.routingFallback
            onChanged: function(value) { root.routingDirty = true; root.routingFallback = value }
        }
        Ui.Button {
            objectName: "rule.save"
            text: root.tr("save"); focusable: true; enabled: !root.saving
            onClicked: {
                root.saving = true
                root.service.save("routing.save", {rules:root.routingRules,fallback:root.routingFallback}, function(ok) {
                    root.saving = false
                    if (ok) { root.routingDirty = false; root.loadRouting() }
                })
            }
        }
        Ui.PanelSectionHeader { text: root.tr("ai"); Layout.fillWidth: true }
        Ui.Toggle { id: aiEnabled; objectName: "ai.enabled"; label: root.tr("ai"); Layout.fillWidth: true; checked: root.document.ai ? root.document.ai.enabled === true : false; onClicked: checked = !checked }
        NativeText { text: root.tr("endpoint"); Layout.fillWidth: true }
        Ui.TextField { id: endpoint; objectName: "ai.endpoint"; Layout.fillWidth: true; text: root.document.ai ? root.document.ai.base_url || "" : ""; placeholderText: "https://…/v1" }
        NativeText { text: root.tr("model"); Layout.fillWidth: true }
        Ui.TextField { id: model; objectName: "ai.model"; Layout.fillWidth: true; text: root.document.ai ? root.document.ai.model || "" : "" }
        NativeText { text: root.tr("apiKey"); Layout.fillWidth: true }
        Ui.TextField { id: aiKey; objectName: "ai.key"; Layout.fillWidth: true; password: true; placeholderText: root.tr(root.document.ai && root.document.ai.has_api_key ? "keyStored" : "keyEmpty") }
        Ui.Toggle { label: root.tr("deleteKey"); Layout.fillWidth: true; checked: root.clearAiKey; onClicked: root.clearAiKey = !root.clearAiKey }
        Ui.Button {
            objectName: "ai.save"
            text: root.tr("save"); focusable: true; enabled: !root.saving
            onClicked: {
                root.saving = true
                root.service.save("ai.save", {enabled:aiEnabled.checked,base_url:endpoint.text,model:model.text,api_key:aiKey.text || null,clear_key:root.clearAiKey}, function(ok) {
                    root.saving = false
                    if (ok) { aiKey.text = ""; root.clearAiKey = false }
                })
            }
        }
        Ui.PanelSectionHeader { text: root.tr("official"); Layout.fillWidth: true }
        Ui.Toggle { id: officialEnabled; objectName: "official.enabled"; label: root.tr("official"); Layout.fillWidth: true; checked: root.document.official ? root.document.official.enabled === true : false; onClicked: { root.officialDirty = true; checked = !checked } }
        Ui.Toggle { id: pro; objectName: "official.pro"; label: root.tr("pro"); Layout.fillWidth: true; checked: root.document.official ? root.document.official.pro === true : false; onClicked: { root.officialDirty = true; checked = !checked } }
        NativeText { text: root.tr("apiKey"); Layout.fillWidth: true }
        Ui.TextField { id: officialKey; objectName: "official.key"; Layout.fillWidth: true; password: true; placeholderText: root.tr(root.document.official && root.document.official.has_api_key ? "keyStored" : "apiKey"); onTextEdited: root.officialDirty = true }
        Ui.Toggle { label: root.tr("deleteKey"); Layout.fillWidth: true; checked: root.clearOfficialKey; onClicked: { root.officialDirty = true; root.clearOfficialKey = !root.clearOfficialKey } }
        Ui.Button {
            objectName: "official.save"
            text: root.tr("save"); focusable: true; enabled: !root.saving
            onClicked: {
                root.saving = true
                root.service.save("official.save", {enabled:officialEnabled.checked,pro:pro.checked,api_key:officialKey.text || null,clear_key:root.clearOfficialKey}, function(ok) {
                    root.saving = false
                    if (ok) { officialKey.text = ""; root.clearOfficialKey = false; root.officialDirty = false }
                })
            }
        }
        NativeText { visible: root.officialDirty; text: root.tr("saveBeforeTest"); color: Color.muted; Layout.fillWidth: true }
        Ui.Button {
            objectName: "official.test"
            text: root.tr("testConnection"); focusable: true
            enabled: !root.saving && !root.officialDirty && !!root.document.official && root.document.official.has_api_key === true
            onClicked: {
                root.saving = true
                root.service.request("official.test", {}, function(ok) {
                    root.saving = false
                    if (ok) root.service.notice = root.tr("connected")
                })
            }
        }
        Ui.PanelSectionHeader { text: root.tr("shortcuts"); Layout.fillWidth: true }
        NativeText { text: root.tr("shortcutHelp"); color: Color.muted; Layout.fillWidth: true }
        Repeater {
            model: [{id:"input",label:"inputAction"},{id:"toggle",label:"toggle"},{id:"selection",label:"selection"},
                {id:"screenshot",label:"screenshot"},{id:"ocr",label:"ocr"},{id:"annotate",label:"annotate"}]
            ColumnLayout {
                id: shortcut
                required property var modelData
                Layout.fillWidth: true; spacing: Style.spacing.xs
                NativeText { text: root.tr(shortcut.modelData.label); Layout.fillWidth: true }
                Ui.TextField {
                    objectName: "shortcut." + shortcut.modelData.id
                    Layout.fillWidth: true
                    text: root.draftShortcuts[shortcut.modelData.id] || ""
                    onTextEdited: { var next = Object.assign({}, root.draftShortcuts); next[shortcut.modelData.id] = text; root.draftShortcuts = next }
                }
                NativeText {
                    text: root.service && root.service.shortcuts.conflicts ? root.service.shortcuts.conflicts[shortcut.modelData.id] || "" : ""
                    visible: !!text; color: Color.urgent; Layout.fillWidth: true
                }
            }
        }
        Ui.Button {
            objectName: "shortcut.save"
            text: root.tr("save"); focusable: true; enabled: !root.saving
            onClicked: {
                root.saving = true
                root.service.request("shortcuts.save", root.draftShortcuts, function(ok, data) {
                    root.saving = false
                    if (ok) { root.service.shortcuts = data; root.service.notice = root.tr("saved") }
                })
            }
        }
        Ui.PanelSectionHeader { text: root.tr("about"); Layout.fillWidth: true }
        NativeText { text: "Lucas Translate · " + (root.service && root.service.manifest ? root.service.manifest.version : "0.1.0") + " · MIT"; Layout.fillWidth: true }
        Ui.PanelSectionHeader { text: root.tr("diagnostics"); Layout.fillWidth: true }
        NativeText { text: root.tr("logsPrivate"); color: Color.muted; Layout.fillWidth: true }
        NativeText {
            visible: !!root.logStatus; Layout.fillWidth: true
            text: root.logStatus ? root.tr(root.logStatus.available && !root.logStatus.write_failed ? "logsReady" : "logsFailed")
                + (root.logStatus.dropped_events ? " · " + root.tr("logsDropped") + ": " + root.logStatus.dropped_events : "") : ""
            color: root.logStatus && root.logStatus.available && !root.logStatus.write_failed ? Color.muted : Color.urgent
        }
        Ui.Button {
            text: root.tr("openLogs"); focusable: true; enabled: !!root.service && root.service.ready
            onClicked: root.service.request("logs.directory", {}, function(ok, directory) {
                if (ok) Qt.openUrlExternally("file://" + directory)
            })
        }
    }
}
