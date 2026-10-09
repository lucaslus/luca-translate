.pragma library

function over(color, background) {
    return Qt.rgba(color.r * color.a + background.r * (1 - color.a),
                   color.g * color.a + background.g * (1 - color.a),
                   color.b * color.a + background.b * (1 - color.a), 1)
}

function luminance(color) {
    function linear(value) { return value <= 0.04045 ? value / 12.92 : Math.pow((value + 0.055) / 1.055, 2.4) }
    return 0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

function contrast(color, background) {
    var a = luminance(over(color, background)), b = luminance(background)
    return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05)
}

// Keep the theme's muted color when readable; otherwise blend toward its text.
// No independent palette: both endpoints remain live Omarchy theme colors.
function secondary(muted, foreground, background) {
    var start = over(muted, background)
    if (contrast(start, background) >= 4.5) return start
    var end = over(foreground, background)
    if (contrast(end, background) <= 4.5) return end
    var low = 0, high = 1
    for (var i = 0; i < 16; i++) {
        var mix = (low + high) / 2
        var color = Qt.rgba(start.r + (end.r - start.r) * mix,
                            start.g + (end.g - start.g) * mix,
                            start.b + (end.b - start.b) * mix, 1)
        if (contrast(color, background) < 4.5) low = mix
        else high = mix
    }
    return Qt.rgba(start.r + (end.r - start.r) * high,
                   start.g + (end.g - start.g) * high,
                   start.b + (end.b - start.b) * high, 1)
}
