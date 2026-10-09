import QtQuick
import Quickshell
import Quickshell.Io
import "Model.js" as Model
import "Strings.js" as Strings

QtObject {
    id: root
    property var shell: null
    property var manifest: null
    property bool autostart: true
    property bool ready: false
    property bool busy: false
    property bool captureBusy: false
    property string error: ""
    property string notice: ""
    property string text: ""
    property string from: "auto"
    property string to: "auto"
    property string requestId: ""
    property string retryService: ""
    property var cards: []
    property var detection: ({})
    property var settings: ({})
    property var wantedOrder: null
    property bool orderSaving: false
    property int orderRevision: 0
    readonly property var serviceOrder: wantedOrder || (settings.usage && Model.validOrder(settings.usage.service_order) ? settings.usage.service_order : Model.defaultOrder())
    onServiceOrderChanged: cards = Model.ordered(cards,serviceOrder)
    property var shortcuts: ({shortcuts:{}, conflicts:{}})
    property var pending: ({})
    property var favoriteStates: ({})
    property var favoritePending: ({})
    property var favoriteVersions: ({})
    property int favoriteRemovalEpoch: 0
    onCardsChanged: now = Date.now()
    property double now: Date.now()
    signal settingsRequested()
    property int commandTimeoutMs: 15000
    property int streamTimeoutMs: 70000
    property double streamDeadline: 0
    property int sequence: 0
    property int translationEpoch: 0
    property int failures: 0
    readonly property string language: settings.preferences && settings.preferences.language !== "auto"
        ? settings.preferences.language : (/^zh/i.test(Qt.locale().name) ? "zh-CN" : "en")
    function tr(key) { return Strings.text(key, language) }

    function request(method, params, callback) {
        if (!ready) {
            error = tr("backendFailed")
            if (callback) callback(false, error)
            return ""
        }
        if (Object.keys(pending).length >= 64) {
            error = tr("overloaded")
            if (callback) callback(false, error)
            return ""
        }
        var id = "native-" + Date.now() + "-" + (++sequence)
        var callbacks = Object.assign({}, pending)
        var timeout = method === "capture" ? (params && params.action === "annotate" ? 0 : 170000)
            : (["official.test", "speak", "shortcuts.save", "shortcuts.check"].indexOf(method) >= 0 ? 30000 : commandTimeoutMs)
        callbacks[id] = {callback:callback || function() {}, deadline:timeout ? Date.now()+timeout : 0, method:method}
        pending = callbacks
        backend.write(JSON.stringify({id:id, method:method, params:params || {}}) + "\n")
        return id
    }

    function object(value) { return value !== null && typeof value === "object" && !Array.isArray(value) }
    function strings(value) { return Array.isArray(value) && value.every(function(item) { return typeof item === "string" }) }
    function optionalString(value) { return value === undefined || value === null || typeof value === "string" }
    function validFailure(value) {
        return value === undefined || value === null || (object(value) && typeof value.code === "string"
            && (value.retry_at_ms === undefined || value.retry_at_ms === null || (typeof value.retry_at_ms === "number" && isFinite(value.retry_at_ms) && value.retry_at_ms >= 0)))
    }
    function validSettings(value) {
        return object(value) && object(value.preferences) && typeof value.preferences.language === "string"
            && object(value.services) && object(value.ai) && object(value.official) && object(value.routing)
            && typeof value.ai.enabled === "boolean" && typeof value.ai.base_url === "string" && typeof value.ai.model === "string"
            && typeof value.ai.has_api_key === "boolean" && typeof value.official.has_api_key === "boolean"
            && Array.isArray(value.routing.rules) && value.routing.rules.every(function(rule) {
                return object(rule) && typeof rule.from === "string" && typeof rule.to === "string"
            }) && typeof value.routing.fallback === "string"
    }
    function validReply(method, value) {
        if (!object(value) && !Array.isArray(value) && typeof value !== "string") return false
        if (method === "settings" || /^(preferences|routing|ai|official|usage)\.save$/.test(method) || method === "service.set") return validSettings(value)
        if (/^shortcuts\./.test(method)) return object(value) && object(value.shortcuts) && object(value.conflicts)
        if (method === "service.order") return object(value) && Model.validOrder(value.order)
        if (method === "history.list" || method === "favorites.list") return Array.isArray(value) && value.every(function(row) {
            return object(row) && typeof row.id === "number" && typeof row.text === "string" && typeof row.result === "string" && typeof row.service === "string"
                && (row.details === undefined || (Array.isArray(row.details) && row.details.length <= 8 && row.details.every(function(card) { return object(card) && typeof card.service === "string" && strings(card.paragraphs) && optionalString(card.text) && optionalString(card.error) && validFailure(card.failure) })))
        })
        if (method === "favorite.status" || method === "favorite.toggle") return object(value) && (value.id === null || (typeof value.id === "number" && isFinite(value.id) && value.id > 0 && Math.floor(value.id) === value.id))
        if (method === "logs.directory") return typeof value === "string"
        if (method === "capture") return object(value) && (value.cancelled === true || value.copied === true || value.handled === true
            || typeof value.text === "string")
        return object(value)
    }
    function complete(id, ok, data) {
        if (!pending[id]) return
        var entry = pending[id]
        var next = Object.assign({}, pending); delete next[id]; pending = next
        if (!ok && (entry.method !== "translate" || id === requestId)) error = data
        try { entry.callback(ok, data) } catch (_) { error = tr("unavailable") }
    }
    function expire() {
        var now = Date.now()
        Object.keys(pending).forEach(function(id) {
            if (pending[id] && pending[id].deadline && pending[id].deadline <= now) complete(id, false, tr("timeout"))
        })
        if (busy && streamDeadline && streamDeadline <= now) {
            stop(); error = tr("timeout")
        }
    }

    function accept(line) {
        var message
        try { message = JSON.parse(line) } catch (_) { error = tr("unavailable"); return }
        if (!object(message)) { error = tr("unavailable"); return }
        if (message.event === "ready") {
            if (!object(message.data) || message.data.protocol !== 1 || (manifest && message.data.version !== manifest.version)) { error = tr("unavailable"); return }
            ready = true; failures = 0; error = ""
            refreshSettings()
            refreshShortcuts()
            return
        }
        if (message.event === "fatal") { error = object(message.data) && typeof message.data.message === "string" ? message.data.message : tr("unavailable"); return }
        if (message.event === "translation") {
            var event = message.data
            if (!object(event) || !object(event.data)) { error = tr("unavailable"); return }
            if (!busy || event.request_id !== requestId) return
            if (event.kind === "start") {
                if (!strings(event.data.services) || event.data.services.length > 8 || new Set(event.data.services).size !== event.data.services.length) { error = tr("unavailable"); return }
                detection = event.data
                cards = Model.ordered(Model.start(cards, event.data.services, retryService),serviceOrder)
            } else if (event.kind === "result") {
                var result = event.data
                if (typeof result.service !== "string" || !strings(result.paragraphs)
                    || !validFailure(result.failure)
                    || ![result.text,result.error,result.pinyin,result.detected_from,result.detected_to].every(optionalString)
                    || !cards.some(function(card) { return card.service === result.service && card.pending })
                    || (result.dictionary_help && (!object(result.dictionary_help) || !strings(result.dictionary_help.suggestions)))
                    || (result.dict && (!object(result.dict) || typeof result.dict.word !== "string"
                        || ![result.dict.uk_phonetic,result.dict.us_phonetic,result.dict.uk_speech,result.dict.us_speech].every(optionalString)
                        || !Array.isArray(result.dict.meanings) || !result.dict.meanings.every(strings)))) { error = tr("unavailable"); return }
                cards = Model.result(cards, result)
                if (!result.error) syncFavorite(result)
            } else if (event.kind === "delta") {
                if (typeof event.data.service !== "string" || typeof event.data.text !== "string" || event.data.text.length > 2000000) { error = tr("unavailable"); return }
                cards = cards.map(function(card) { return card.service === event.data.service && card.pending ? Object.assign({},card,{paragraphs:event.data.text.split("\n"),streaming:true}) : card })
            } else if (event.kind === "error" || event.kind === "warning") {
                if (typeof event.data.message !== "string") { error = tr("unavailable"); return }
                if (event.kind === "error") { error = event.data.message; cards = Model.finish(cards, error) }
                else notice = event.data.message
            } else if (event.kind === "done") { busy = false; streamDeadline = 0; cards = Model.finish(cards, tr("interrupted")) }
            return
        }
        if (message.id && pending[message.id]) {
            if (typeof message.ok !== "boolean" || (message.ok && !validReply(pending[message.id].method, message.data))
                || (!message.ok && typeof message.error !== "string")) { complete(message.id, false, tr("unavailable")); return }
            complete(message.id, message.ok, message.ok ? message.data : message.error)
        }
    }

    function refreshSettings() {
        var revision=orderRevision
        request("settings", {}, function(ok, data) { if (ok) applySettings(data,revision) })
    }
    function applySettings(data, revision) {
        if (revision !== orderRevision && settings.usage && data.usage) data=Object.assign({},data,{usage:Object.assign({},data.usage,{service_order:settings.usage.service_order.slice()})})
        settings=data
    }
    function reorderServices(order) {
        if (!Model.validOrder(order) || JSON.stringify(order)===JSON.stringify(serviceOrder)) return
        orderRevision++; wantedOrder=order.slice(); flushOrder()
    }
    function flushOrder() {
        if (orderSaving || !wantedOrder) return
        var sent=wantedOrder.slice()
        orderSaving=true
        request("service.order",{order:sent},function(ok,data) {
            orderSaving=false
            if (ok) {
                orderRevision++
                settings=Object.assign({},settings,{usage:Object.assign({},settings.usage || {},{service_order:data.order.slice()})})
            }
            if (JSON.stringify(wantedOrder)===JSON.stringify(sent)) wantedOrder=null
            flushOrder()
        })
    }
    function refreshShortcuts() {
        request("shortcuts.status", {}, function(ok, data) { if (ok) shortcuts = data })
    }
    function save(method, params, callback) {
        error = ""; notice = ""
        var revision=orderRevision
        return request(method, params, function(ok, data) {
            if (ok) { applySettings(data,revision); notice = tr("saved") }
            if (callback) callback(ok)
        })
    }
    function translate(value, only) {
        if (!ready || !String(value).trim()) return
        stop()
        var epoch = translationEpoch
        text = value; error = ""; notice = ""; busy = true; retryService = only || ""
        streamDeadline = Date.now() + streamTimeoutMs
        if (!only) cards = []
        requestId = request("translate", {text:value, from:from, to:to, only:only || null}, function(ok) {
            if (epoch !== translationEpoch) return
            if (!ok) { busy = false; streamDeadline = 0; cards = Model.finish(cards, error) }
        })
    }
    function stop() {
        translationEpoch++
        if (busy && requestId) request("cancel", {request_id:requestId})
        busy = false; requestId = ""; streamDeadline = 0
        cards = Model.finish(cards, tr("interrupted"))
    }
    property int captureEpoch: 0
    function clear() { captureEpoch++; captureBusy = false; stop(); text = ""; cards = []; detection = ({}); error = ""; notice = "" }
    function capture(action, source) {
        if (!ready || captureBusy) return
        if (action === "selection") clear()
        var epoch = ++captureEpoch
        captureBusy = true; error = ""; notice = ""
        request("capture", {action:action, source:source || null}, function(ok, data) {
            if (epoch !== captureEpoch) return
            captureBusy = false
            if (ok && data.cancelled) return
            if (ok && action === "selection" && !data.text) {
                clear()
                if (shell) shell.summon("lucas.translate", JSON.stringify({action:"input"}))
                return
            }
            if (ok && data.text) {
                text = data.text
                translate(text)
                if (shell) shell.summon("lucas.translate", JSON.stringify({action:"captured"}))
            } else if (ok && data.copied) notice = tr("copied")
            else if (!ok && shell) shell.summon("lucas.translate", JSON.stringify({action:"error"}))
        })
    }
    function copy(value, callback) { request("copy", {text:value}, function(ok) { if (ok) notice = tr("copied"); if (callback) callback(ok) }) }
    function favoriteKey(card) { return JSON.stringify([card.text || "",Model.translated(card),card.service]) }
    function syncFavorite(card) {
        var key = favoriteKey(card), version=favoriteVersions[key] || 0, removalEpoch=favoriteRemovalEpoch
        request("favorite.status", {text:card.text || "",result:Model.translated(card),service:card.service}, function(ok,data) {
            if (ok && removalEpoch===favoriteRemovalEpoch && !favoritePending[key] && (favoriteVersions[key] || 0)===version) { var next=Object.assign({},favoriteStates); next[key]=data.id; var keys=Object.keys(next); while (keys.length>128) delete next[keys.shift()]; favoriteStates=next }
        })
    }
    function favorite(card) {
        var key = favoriteKey(card)
        if (favoritePending[key]) return
        var versions=Object.assign({},favoriteVersions); versions[key]=(versions[key] || 0)+1; var keys=Object.keys(versions); while (keys.length>128) delete versions[keys.shift()]; favoriteVersions=versions
        var waiting=Object.assign({},favoritePending); waiting[key]=true; favoritePending=waiting
        request("favorite.toggle", {text:card.text || "",result:Model.translated(card),service:card.service}, function(ok,data) {
            var next=Object.assign({},favoritePending); delete next[key]; favoritePending=next
            if (ok) { var states=Object.assign({},favoriteStates); states[key]=data.id; var keys=Object.keys(states); while (keys.length>128) delete states[keys.shift()]; favoriteStates=states; notice=tr(data.id ? "saved" : "removed") }
        })
    }
    function removeFavorite(id,callback) {
        favoriteRemovalEpoch++
        request("favorite.remove",{id:id},function(ok) {
            if (ok) { var next=Object.assign({},favoriteStates); Object.keys(next).forEach(function(key) { if (next[key]===id) next[key]=null }); favoriteStates=next }
            if (callback) callback(ok)
        })
    }
    function reconnect() { failures = 0; if (!backend.running) backend.running = true }
    property Process backend: Process {
        command: ["bash", Qt.resolvedUrl("scripts/backend.sh").toString().replace(/^file:\/\//, "")]
        running: root.autostart
        stdinEnabled: true
        stdout: SplitParser { onRead: function(line) { root.accept(line) } }
        // No stderr is relayed into the shared Shell journal.
        stderr: SplitParser { onRead: function(line) {} }
        onExited: {
            root.ready = false; root.busy = false; root.captureBusy = false
            root.requestId = ""; root.streamDeadline = 0
            root.cards = Model.finish(root.cards, root.tr("interrupted"))
            var callbacks = root.pending; root.pending = ({})
            for (var id in callbacks) {
                try { callbacks[id].callback(false, root.tr("backendFailed")) } catch (_) {}
            }
            if (!root.error) root.error = root.tr("backendFailed")
            if (root.autostart && ++root.failures <= 3) restartTimer.restart()
        }
    }
    property Timer deadlineTimer: Timer {
        interval: 100; repeat: true; running: Object.keys(root.pending).length > 0 || root.busy
        onTriggered: root.expire()
    }
    property Timer restartTimer: Timer {
        interval: Math.min(10000, 1000 * Math.pow(2, root.failures))
        onTriggered: if (!backend.running) backend.running = true
    }
    property Timer cooldownTimer: Timer {
        interval: 1000; repeat: true
        running: root.cards.some(function(card) { return card.failure && card.failure.retry_at_ms > root.now })
        onTriggered: root.now = Date.now()
    }
}
