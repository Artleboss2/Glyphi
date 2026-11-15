/*
 * The calm layer.
 *
 * Glyphi's window is a WebView, and a blurred, permanently animated page keeps a
 * compositor busy: measured on this machine, the WebView's GPU process sat above one
 * full core while the dashboard was idle. Glyphi is a dashboard — its numbers do not
 * have to move to be read — so the shell turns down what costs CPU without changing
 * what the page says:
 *
 *   - every animation runs once, in a millisecond, instead of forever;
 *   - `backdrop-filter` is dropped: a blurred surface is re-rasterised on every paint;
 *   - transitions stay, short enough to read as immediate.
 *
 * The page is untouched: this is a stylesheet the shell adds, so the React app keeps
 * its own files and the same classes.
 */
(function () {
  var style = document.createElement("style");
  style.setAttribute("data-glyphi", "calm");
  style.textContent = [
    "*, *::before, *::after {",
    "  animation-duration: 1ms !important;",
    "  animation-delay: 0s !important;",
    "  animation-iteration-count: 1 !important;",
    "  transition-duration: 60ms !important;",
    "  transition-delay: 0s !important;",
    "  backdrop-filter: none !important;",
    "  -webkit-backdrop-filter: none !important;",
    "  scroll-behavior: auto !important;",
    "}"
  ].join("\n");

  function attach() {
    if (style.parentNode) return;
    var host = document.head || document.documentElement;
    if (host) host.appendChild(style);
  }

  attach();
  document.addEventListener("DOMContentLoaded", attach);
})();
