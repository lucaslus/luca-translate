.pragma library

function defaultOrder() { return ["YoudaoDict","DeepLApi","AI","Bing","DeepLFree","GoogleFree"] }
function validOrder(order) {
    return Array.isArray(order) && order.length === 6 && defaultOrder().every(function(provider) { return order.filter(function(item) { return item === provider }).length === 1 })
}
function ordered(cards, order) {
    return (cards || []).slice().sort(function(a,b) {
        var left=order.indexOf(a.service), right=order.indexOf(b.service)
        return (left<0 ? order.length : left) - (right<0 ? order.length : right)
    })
}

function providerBrand(provider) {
    var name = String(provider || "").toLowerCase()
    if (["youdao", "youdaodict", "youdaotranslate"].indexOf(name) >= 0) return "youdao"
    if (name === "bing") return "bing"
    if (["deepl", "deeplfree", "deeplapi", "deepl_api"].indexOf(name) >= 0) return "deepl"
    if (["google", "googlefree"].indexOf(name) >= 0) return "google"
    return ""
}

function providerLabel(provider, language) {
    var brand = providerBrand(provider)
    if (brand === "youdao") return String(language || "").indexOf("zh") === 0 ? "有道" : "Youdao"
    if (brand === "deepl") return ["DeepLApi", "deepl_api"].indexOf(provider) >= 0 ? "DeepL API" : "DeepL"
    if (brand === "google") return "Google"
    return provider
}

function start(cards, services, only) {
    var next = only ? cards.filter(function(card) { return card.service !== only }) : []
    services.forEach(function(service) {
        next.push({service: service, pending: true, paragraphs: [], error: null})
    })
    return next
}

function result(cards, value) {
    var replaced = false
    var next = cards.map(function(card) {
        if (card.service !== value.service) return card
        replaced = true
        return Object.assign({}, value, {pending: false}, value.error && card.streaming ? {paragraphs:card.paragraphs,partial:true} : {})
    })
    if (!replaced) next.push(Object.assign({}, value, {pending: false}))
    return next
}

function finish(cards, message) {
    return cards.map(function(card) {
        return card.pending ? Object.assign({}, card, {pending: false, error: message,partial:!!card.streaming}) : card
    })
}

function translated(card) { return (card.paragraphs || []).join("\n") }

function repeatsDictionary(card) {
    if (!card.dict || !card.dict.meanings || !card.dict.meanings.length) return false
    function normalize(value) { return value.replace(/\s+/g, " ").trim() }
    return normalize(translated(card)) === normalize(card.dict.meanings.map(function(meaning) { return meaning.join(" ") }).join("\n"))
}

function key(event) {
    // The compositor manages shortcuts; the editor accepts explicit combinations.
    var modifiers = []
    if (event.modifiers & 0x10000000) modifiers.push("Super")
    if (event.modifiers & 0x04000000) modifiers.push("Ctrl")
    if (event.modifiers & 0x08000000) modifiers.push("Alt")
    if (event.modifiers & 0x02000000) modifiers.push("Shift")
    var symbol = String(event.text || "").toUpperCase()
    return modifiers.length && /^[A-Z0-9]$/.test(symbol) ? modifiers.concat([symbol]).join("+") : ""
}
