import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qs.Commons
import "Strings.js" as Strings
import "Theme.js" as Theme
import "Model.js" as Model

Flickable {
    id: root
    objectName: "settingsView"
    required property var service
    property var draftShortcuts: ({})
    property bool usageDirty: false
    property var usageDraft: ({service_order:["YoudaoDict","DeepLApi","AI","Bing","DeepLFree","GoogleFree"],history_enabled:true,history_days:0,history_limit:0,ocr_cleanup:false})
    property bool shortcutsDirty: false
    property bool aiDirty: false
    property bool aiSwitchDirty: false
    property bool aiChecked: false
    property bool officialChecked: false
    property string aiEndpoint: ""
    property string aiModel: ""
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
    function loadAi() {
        if (!document.ai) return
        if (!aiSwitchDirty) aiChecked=document.ai.enabled===true
        if (aiDirty) return
        aiEndpoint = document.ai.base_url || ""
        aiModel = document.ai.model || ""
    }
    function loadUsage() {
        if (!usageDirty && document.usage) usageDraft=Object.assign({},document.usage,{service_order:document.usage.service_order.slice()})
    }
    function changeUsage(key,value) { usageDirty=true; var next=Object.assign({},usageDraft); next[key]=value; usageDraft=next }
    function providerEnabled(provider) {
        if (provider==="AI") return aiChecked
        if (provider==="DeepLApi") return officialChecked
        return !!document.services && document.services[{YoudaoDict:"youdao",Bing:"bing",DeepLFree:"deepl",GoogleFree:"google"}[provider]]===true
    }
    function loadOfficial() { if (!officialDirty && document.official) officialChecked=document.official.enabled===true }
    function toggleProvider(provider) {
        if (provider==="AI") {
            aiSwitchDirty=true; aiChecked=!aiChecked
            if (!aiChecked && document.ai && document.ai.enabled) {
                saving=true
                service.save("ai.save",{enabled:false,base_url:document.ai.base_url,model:document.ai.model,api_key:null,clear_key:false},function(ok) {
                    root.saving=false; root.aiSwitchDirty=false; root.loadAi()
                })
            }
        } else if (provider==="DeepLApi") {
            officialDirty=true; officialChecked=!officialChecked
            if (!officialChecked && document.official && document.official.enabled) {
                saving=true
                service.save("official.save",{enabled:false,pro:document.official.pro,api_key:null,clear_key:false},function(ok) {
                    root.saving=false; root.officialDirty=false; root.loadOfficial()
                })
            }
        } else service.save("service.set",{id:{YoudaoDict:"youdao",Bing:"bing",DeepLFree:"deepl",GoogleFree:"google"}[provider],enabled:!providerEnabled(provider)})
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
    onDocumentChanged: { loadRouting(); loadAi(); loadOfficial(); loadUsage() }
    Component.onCompleted: { loadRouting(); loadAi(); loadOfficial(); loadUsage() }
    onVisibleChanged: if (visible && service && service.ready) { service.refreshSettings(); service.refreshShortcuts(); refreshDiagnostics() }
    Connections {
        target: root.service
        function onShortcutsChanged() { if (!root.shortcutsDirty) root.draftShortcuts = Object.assign({}, root.service.shortcuts.shortcuts) }
        function onReadyChanged() { if (root.visible && root.service.ready) root.refreshDiagnostics() }
    }

    ColumnLayout {
        id: column
        width: root.width; spacing: Style.spacing.md
        NativeDropdown {
            objectName: "interfaceLanguage"
            Layout.fillWidth: true; label: root.tr("language")
            value: root.document.preferences ? root.document.preferences.language : "auto"
            options: [{value:"auto",label:root.tr("system")},{value:"en",label:"English"},{value:"zh-CN",label:"简体中文"}]
            onChanged: function(value) { root.service.save("preferences.save", {language:value}) }
        }
        NativeSectionHeader { color: Theme.secondary(Color.muted, Color.popups.text, Color.popups.background); text: root.tr("services"); Layout.fillWidth: true }
        ServiceList {
            service: root.service; viewport: root; enabledFor: root.providerEnabled
            Layout.fillWidth: true; enabled: !root.saving && !!root.service && root.service.ready
            onToggled: function(provider) { root.toggleProvider(provider) }
        }
        NativeSectionHeader { color: Theme.secondary(Color.muted, Color.popups.text, Color.popups.background); text: root.tr("routing"); Layout.fillWidth: true }
        Repeater {
            model: root.routingRules
            ColumnLayout {
                id: route
                required property var modelData
                required property int index
                Layout.fillWidth: true; spacing: Style.spacing.xs
                RowLayout {
                    Layout.fillWidth: true
                    NativeDropdown {
                        objectName: "rule.from." + route.index
                        label: root.tr("source"); showLabel: false; Accessible.name: label; Layout.fillWidth: true; options: root.languages; value: route.modelData.from
                        onChanged: function(value) { root.changeRule(route.index,"from",value) }
                    }
                    NativeText { text: "→" }
                    NativeDropdown {
                        objectName: "rule.to." + route.index
                        label: root.tr("target"); showLabel: false; Accessible.name: label; Layout.fillWidth: true; options: root.languages; value: route.modelData.to
                        onChanged: function(value) { root.changeRule(route.index,"to",value) }
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    NativeButton { foreground: Color.popups.text; objectName: "rule.up." + route.index; text: root.tr("moveUp"); focusable: true; enabled: route.index > 0; onClicked: root.moveRule(route.index,-1) }
                    NativeButton { foreground: Color.popups.text; objectName: "rule.down." + route.index; text: root.tr("moveDown"); focusable: true; enabled: route.index < root.routingRules.length-1; onClicked: root.moveRule(route.index,1) }
                    Item { Layout.fillWidth: true }
                    NativeButton { foreground: Color.popups.text; objectName: "rule.remove." + route.index; text: root.tr("remove"); focusable: true; onClicked: root.removeRule(route.index) }
                }
            }
        }
        NativeButton { foreground: Color.popups.text; objectName: "rule.add"; text: root.tr("addRule"); focusable: true; enabled: root.routingRules.length < root.languages.length; onClicked: root.addRule() }
        NativeDropdown {
            label: root.tr("fallback"); Layout.fillWidth: true; options: root.languages; value: root.routingFallback
            onChanged: function(value) { root.routingDirty = true; root.routingFallback = value }
        }
        NativeButton { foreground: Color.popups.text;
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
        NativeSectionHeader { text: root.tr("ai"); visible: root.aiChecked; color: Theme.secondary(Color.muted,Color.popups.text,Color.popups.background); Layout.fillWidth: true }
        ColumnLayout {
            objectName: "ai.form"
            visible: root.aiChecked; Layout.fillWidth: true; spacing: Style.spacing.md
            NativeText { text: root.tr("endpoint"); Layout.fillWidth: true }
            NativeField { id: endpoint; objectName: "ai.endpoint"; Layout.fillWidth: true; text: root.aiEndpoint; placeholderText: "https://…/v1"; onTextEdited: { root.aiDirty = true; root.aiEndpoint = text } }
            NativeText { text: root.tr("model"); Layout.fillWidth: true }
            NativeField { id: model; objectName: "ai.model"; Layout.fillWidth: true; text: root.aiModel; onTextEdited: { root.aiDirty = true; root.aiModel = text } }
            NativeText { text: root.tr("apiKey"); Layout.fillWidth: true }
            NativeField { id: aiKey; objectName: "ai.key"; Layout.fillWidth: true; password: true; placeholderText: root.tr(root.document.ai && root.document.ai.has_api_key ? "keyStored" : "keyEmpty") }
            NativeToggle { visible: !!root.document.ai && root.document.ai.has_api_key === true; label: root.tr("deleteKey"); Layout.fillWidth: true; checked: root.clearAiKey; onClicked: root.clearAiKey = !root.clearAiKey }
            NativeButton { foreground: Color.popups.text;
                objectName: "ai.save"
                text: root.tr("save"); focusable: true; enabled: !root.saving
                onClicked: {
                    root.saving = true
                    root.service.save("ai.save", {enabled:root.aiChecked,base_url:endpoint.text,model:model.text,api_key:aiKey.text || null,clear_key:root.clearAiKey}, function(ok) {
                        root.saving = false
                        if (ok) { aiKey.text = ""; root.clearAiKey = false; root.aiDirty = false; root.aiSwitchDirty = false; root.loadAi() }
                    })
                }
            }
        }
        NativeSectionHeader { text: root.tr("official"); visible: root.officialChecked; color: Theme.secondary(Color.muted,Color.popups.text,Color.popups.background); Layout.fillWidth: true }
        ColumnLayout {
            objectName: "official.form"
            visible: root.officialChecked; Layout.fillWidth: true; spacing: Style.spacing.md
            NativeToggle { id: pro; objectName: "official.pro"; label: root.tr("pro"); Layout.fillWidth: true; checked: root.document.official ? root.document.official.pro === true : false; onClicked: { root.officialDirty = true; checked = !checked } }
            NativeText { text: root.tr("apiKey"); Layout.fillWidth: true }
            NativeField { id: officialKey; objectName: "official.key"; Layout.fillWidth: true; password: true; placeholderText: root.tr(root.document.official && root.document.official.has_api_key ? "keyStored" : "apiKey"); onTextEdited: root.officialDirty = true }
            NativeToggle { visible: !!root.document.official && root.document.official.has_api_key === true; label: root.tr("deleteKey"); Layout.fillWidth: true; checked: root.clearOfficialKey; onClicked: { root.officialDirty = true; root.clearOfficialKey = !root.clearOfficialKey } }
            NativeButton { foreground: Color.popups.text;
                objectName: "official.save"
                text: root.tr("save"); focusable: true; enabled: !root.saving
                onClicked: {
                    root.saving = true
                    root.service.save("official.save", {enabled:root.officialChecked,pro:pro.checked,api_key:officialKey.text || null,clear_key:root.clearOfficialKey}, function(ok) {
                        root.saving = false
                        if (ok) { officialKey.text = ""; root.clearOfficialKey = false; root.officialDirty = false; root.loadOfficial() }
                    })
                }
            }
            NativeButton { foreground: Color.popups.text;
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
        NativeSectionHeader { color: Theme.secondary(Color.muted, Color.popups.text, Color.popups.background); text: root.tr("shortcuts"); Layout.fillWidth: true }
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
                    onTextEdited: { root.shortcutsDirty = true; var next = Object.assign({}, root.draftShortcuts); next[shortcut.modelData.id] = text; root.draftShortcuts = next }
                }
                NativeText {
                    text: root.service && root.service.shortcuts.conflicts ? root.service.shortcuts.conflicts[shortcut.modelData.id] || "" : ""
                    visible: !!text; color: Color.urgent; Layout.fillWidth: true
                }
            }
        }
        NativeButton { foreground: Color.popups.text;
            objectName: "shortcut.save"
            text: root.tr("save"); focusable: true; enabled: !root.saving
            onClicked: {
                root.saving = true
                root.service.request("shortcuts.save", root.draftShortcuts, function(ok, data) {
                    root.saving = false
                    if (ok) { root.shortcutsDirty = false; root.service.shortcuts = data; root.service.notice = root.tr("saved") }
                })
            }
        }
        NativeSectionHeader { text: root.tr("usage"); color: Theme.secondary(Color.muted,Color.popups.text,Color.popups.background); Layout.fillWidth: true }
        NativeToggle { objectName: "usage.history"; label: root.tr("historyStorage"); checked: root.usageDraft.history_enabled; Layout.fillWidth: true; onClicked: root.changeUsage("history_enabled",!checked) }
        NativeDropdown { objectName: "usage.days"; label: root.tr("historyDays"); Layout.fillWidth: true; value: String(root.usageDraft.history_days); options: [{value:"0",label:root.tr("unlimited")},{value:"7",label:"7 d"},{value:"30",label:"30 d"},{value:"90",label:"90 d"},{value:"365",label:"365 d"}]; onChanged: function(value) { root.changeUsage("history_days",Number(value)) } }
        NativeDropdown { objectName: "usage.limit"; label: root.tr("historyLimit"); Layout.fillWidth: true; value: String(root.usageDraft.history_limit); options: [{value:"0",label:root.tr("unlimited")},{value:"100",label:"100"},{value:"1000",label:"1000"},{value:"10000",label:"10000"}]; onChanged: function(value) { root.changeUsage("history_limit",Number(value)) } }
        NativeToggle { objectName: "usage.ocr"; label: root.tr("ocrCleanup"); checked: root.usageDraft.ocr_cleanup; Layout.fillWidth: true; onClicked: root.changeUsage("ocr_cleanup",!checked) }
        NativeButton {
            objectName: "usage.save"; text: root.tr("save"); focusable: true; enabled: !root.saving && root.usageDirty && !root.service.orderSaving
            onClicked: {
                root.saving=true
                root.service.save("usage.save",Object.assign({},root.usageDraft,{service_order:root.service.serviceOrder.slice()}),function(ok) { root.saving=false; if (ok) { root.usageDirty=false; root.loadUsage() } })
            }
        }
        NativeSectionHeader { color: Theme.secondary(Color.muted, Color.popups.text, Color.popups.background); text: root.tr("about"); Layout.fillWidth: true }
        NativeText { text: "Lucas Translate · " + (root.service && root.service.manifest ? root.service.manifest.version : "0.1.0") + " · MIT"; Layout.fillWidth: true }
        NativeSectionHeader { color: Theme.secondary(Color.muted, Color.popups.text, Color.popups.background); text: root.tr("diagnostics"); Layout.fillWidth: true }
        NativeText {
            visible: !!root.logStatus; Layout.fillWidth: true
            text: root.logStatus ? root.tr(root.logStatus.available && !root.logStatus.write_failed ? "logsReady" : "logsFailed")
                + (root.logStatus.dropped_events ? " · " + root.tr("logsDropped") + ": " + root.logStatus.dropped_events : "") : ""
            secondary: true
            color: root.logStatus && root.logStatus.available && !root.logStatus.write_failed ? Theme.secondary(Color.muted, Color.popups.text, Color.popups.background) : Color.urgent
        }
        NativeButton { foreground: Color.popups.text;
            text: root.tr("openLogs"); focusable: true; enabled: !!root.service && root.service.ready
            onClicked: root.service.request("logs.directory", {}, function(ok, directory) {
                if (ok) Qt.openUrlExternally("file://" + directory)
            })
        }
    }
}
