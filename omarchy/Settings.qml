import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qs.Commons
import qs.Ui as Ui
import "Strings.js" as Strings
import "Theme.js" as Theme

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
        Ui.Dropdown {
            objectName: "interfaceLanguage"
            Layout.fillWidth: true; label: root.tr("language")
            value: root.document.preferences ? root.document.preferences.language : "auto"
            options: [{value:"auto",label:root.tr("system")},{value:"en",label:"English"},{value:"zh-CN",label:"简体中文"}]
            onChanged: function(value) { root.service.save("preferences.save", {language:value}) }
        }
        Ui.PanelSectionHeader { color: Theme.secondary(Color.muted, Color.popups.text, Color.popups.background); text: root.tr("services"); Layout.fillWidth: true }
        GridLayout {
            Layout.fillWidth: true
            columns: 2; columnSpacing: Style.spacing.md; rowSpacing: Style.spacing.xs
            Repeater {
                model: [{id:"youdao",label:"Youdao / 有道"},{id:"bing",label:"Bing"},{id:"deepl",label:"DeepL"},{id:"google",label:"Google"}]
                RowLayout {
                    required property var modelData
                    Layout.fillWidth: true
                    spacing: Style.spacing.xs
                    ProviderIcon { provider: modelData.id }
                    NativeToggle {
                        objectName: "service." + modelData.id
                        label: modelData.label; Layout.fillWidth: true
                        checked: root.document.services ? root.document.services[modelData.id] === true : false
                        onClicked: root.service.save("service.set", {id:modelData.id,enabled:!checked})
                    }
                }
            }
        }
        Ui.PanelSectionHeader { color: Theme.secondary(Color.muted, Color.popups.text, Color.popups.background); text: root.tr("routing"); Layout.fillWidth: true }
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
                        label: root.tr("source"); showLabel: false; Accessible.name: label; Layout.fillWidth: true; options: root.languages; value: route.modelData.from
                        onChanged: function(value) { root.changeRule(route.index,"from",value) }
                    }
                    NativeText { text: "→" }
                    Ui.Dropdown {
                        objectName: "rule.to." + route.index
                        label: root.tr("target"); showLabel: false; Accessible.name: label; Layout.fillWidth: true; options: root.languages; value: route.modelData.to
                        onChanged: function(value) { root.changeRule(route.index,"to",value) }
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    Ui.Button { foreground: Color.popups.text; objectName: "rule.up." + route.index; text: root.tr("moveUp"); focusable: true; enabled: route.index > 0; onClicked: root.moveRule(route.index,-1) }
                    Ui.Button { foreground: Color.popups.text; objectName: "rule.down." + route.index; text: root.tr("moveDown"); focusable: true; enabled: route.index < root.routingRules.length-1; onClicked: root.moveRule(route.index,1) }
                    Item { Layout.fillWidth: true }
                    Ui.Button { foreground: Color.popups.text; objectName: "rule.remove." + route.index; text: root.tr("remove"); focusable: true; onClicked: root.removeRule(route.index) }
                }
            }
        }
        Ui.Button { foreground: Color.popups.text; objectName: "rule.add"; text: root.tr("addRule"); focusable: true; enabled: root.routingRules.length < root.languages.length; onClicked: root.addRule() }
        Ui.Dropdown {
            label: root.tr("fallback"); Layout.fillWidth: true; options: root.languages; value: root.routingFallback
            onChanged: function(value) { root.routingDirty = true; root.routingFallback = value }
        }
        Ui.Button { foreground: Color.popups.text;
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
        NativeToggle { id: aiEnabled; objectName: "ai.enabled"; label: root.tr("ai"); Layout.fillWidth: true; checked: root.document.ai ? root.document.ai.enabled === true : false; enabled: !root.saving
            onClicked: {
                checked = !checked
                if (!checked && root.document.ai && root.document.ai.enabled) {
                    root.saving = true
                    root.service.save("ai.save", {enabled:false,base_url:root.document.ai.base_url,model:root.document.ai.model,api_key:null,clear_key:false}, function(ok) {
                        root.saving = false
                        if (!ok) aiEnabled.checked = true
                    })
                }
            }
        }
        ColumnLayout {
            objectName: "ai.form"
            visible: aiEnabled.checked; Layout.fillWidth: true; spacing: Style.spacing.md
            NativeText { text: root.tr("endpoint"); Layout.fillWidth: true }
            NativeField { id: endpoint; objectName: "ai.endpoint"; Layout.fillWidth: true; text: root.document.ai ? root.document.ai.base_url || "" : ""; placeholderText: "https://…/v1" }
            NativeText { text: root.tr("model"); Layout.fillWidth: true }
            NativeField { id: model; objectName: "ai.model"; Layout.fillWidth: true; text: root.document.ai ? root.document.ai.model || "" : "" }
            NativeText { text: root.tr("apiKey"); Layout.fillWidth: true }
            NativeField { id: aiKey; objectName: "ai.key"; Layout.fillWidth: true; password: true; placeholderText: root.tr(root.document.ai && root.document.ai.has_api_key ? "keyStored" : "keyEmpty") }
            NativeToggle { visible: !!root.document.ai && root.document.ai.has_api_key === true; label: root.tr("deleteKey"); Layout.fillWidth: true; checked: root.clearAiKey; onClicked: root.clearAiKey = !root.clearAiKey }
            Ui.Button { foreground: Color.popups.text;
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
        }
        NativeToggle { id: officialEnabled; objectName: "official.enabled"; label: root.tr("official"); Layout.fillWidth: true; checked: root.document.official ? root.document.official.enabled === true : false; enabled: !root.saving
            onClicked: {
                root.officialDirty = true; checked = !checked
                if (!checked && root.document.official && root.document.official.enabled) {
                    root.saving = true
                    root.service.save("official.save", {enabled:false,pro:root.document.official.pro,api_key:null,clear_key:false}, function(ok) {
                        root.saving = false
                        if (ok) root.officialDirty = false
                        else officialEnabled.checked = true
                    })
                }
            }
        }
        ColumnLayout {
            objectName: "official.form"
            visible: officialEnabled.checked; Layout.fillWidth: true; spacing: Style.spacing.md
            NativeToggle { id: pro; objectName: "official.pro"; label: root.tr("pro"); Layout.fillWidth: true; checked: root.document.official ? root.document.official.pro === true : false; onClicked: { root.officialDirty = true; checked = !checked } }
            NativeText { text: root.tr("apiKey"); Layout.fillWidth: true }
            NativeField { id: officialKey; objectName: "official.key"; Layout.fillWidth: true; password: true; placeholderText: root.tr(root.document.official && root.document.official.has_api_key ? "keyStored" : "apiKey"); onTextEdited: root.officialDirty = true }
            NativeToggle { visible: !!root.document.official && root.document.official.has_api_key === true; label: root.tr("deleteKey"); Layout.fillWidth: true; checked: root.clearOfficialKey; onClicked: { root.officialDirty = true; root.clearOfficialKey = !root.clearOfficialKey } }
            Ui.Button { foreground: Color.popups.text;
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
            Ui.Button { foreground: Color.popups.text;
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
        }
        Ui.PanelSectionHeader { color: Theme.secondary(Color.muted, Color.popups.text, Color.popups.background); text: root.tr("shortcuts"); Layout.fillWidth: true }
        Repeater {
            model: [{id:"input",label:"inputAction"},{id:"toggle",label:"toggle"},{id:"selection",label:"selection"},
                {id:"screenshot",label:"screenshot"},{id:"ocr",label:"ocr"},{id:"annotate",label:"annotate"}]
            ColumnLayout {
                id: shortcut
                required property var modelData
                Layout.fillWidth: true; spacing: Style.spacing.xs
                NativeText { text: root.tr(shortcut.modelData.label); Layout.fillWidth: true }
                NativeField {
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
        Ui.Button { foreground: Color.popups.text;
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
        Ui.PanelSectionHeader { color: Theme.secondary(Color.muted, Color.popups.text, Color.popups.background); text: root.tr("about"); Layout.fillWidth: true }
        NativeText { text: "Lucas Translate · " + (root.service && root.service.manifest ? root.service.manifest.version : "0.1.0") + " · MIT"; Layout.fillWidth: true }
        Ui.PanelSectionHeader { color: Theme.secondary(Color.muted, Color.popups.text, Color.popups.background); text: root.tr("diagnostics"); Layout.fillWidth: true }
        NativeText {
            visible: !!root.logStatus; Layout.fillWidth: true
            text: root.logStatus ? root.tr(root.logStatus.available && !root.logStatus.write_failed ? "logsReady" : "logsFailed")
                + (root.logStatus.dropped_events ? " · " + root.tr("logsDropped") + ": " + root.logStatus.dropped_events : "") : ""
            secondary: true
            color: root.logStatus && root.logStatus.available && !root.logStatus.write_failed ? Theme.secondary(Color.muted, Color.popups.text, Color.popups.background) : Color.urgent
        }
        Ui.Button { foreground: Color.popups.text;
            text: root.tr("openLogs"); focusable: true; enabled: !!root.service && root.service.ready
            onClicked: root.service.request("logs.directory", {}, function(ok, directory) {
                if (ok) Qt.openUrlExternally("file://" + directory)
            })
        }
    }
}
