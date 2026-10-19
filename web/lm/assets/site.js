// LayeredMap website and help (WP6.10): the light/dark switch. No libraries, no network.
(function () {
  var root = document.documentElement;
  var KEY = "lm-site-theme";
  function stored() {
    try { return localStorage.getItem(KEY); } catch (e) { return null; }
  }
  function store(v) {
    try { if (v) { localStorage.setItem(KEY, v); } else { localStorage.removeItem(KEY); } } catch (e) { /* private window */ }
  }
  var t = stored();
  if (t === "light" || t === "dark") { root.setAttribute("data-theme", t); }

  function current() {
    var a = root.getAttribute("data-theme");
    if (a) { return a; }
    return window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  document.addEventListener("DOMContentLoaded", function () {
    var btn = document.querySelector(".theme-btn");
    if (!btn) { return; }
    var zh = (root.lang || "").indexOf("zh") === 0;
    function label() {
      var next = current() === "dark" ? "light" : "dark";
      btn.setAttribute("aria-label", zh ? (next === "dark" ? "切换到深色" : "切换到浅色") : "Switch to " + next + " mode");
      btn.title = btn.getAttribute("aria-label");
    }
    label();
    btn.addEventListener("click", function () {
      var next = current() === "dark" ? "light" : "dark";
      root.setAttribute("data-theme", next);
      store(next);
      label();
    });
  });
})();
