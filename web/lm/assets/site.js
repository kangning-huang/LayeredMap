// NimbleMap website and help (WP6.10): the light/dark switch. No libraries, no network.
(function () {
  var root = document.documentElement;
  root.classList.add("js");
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

  // Hero showcase: one shot per kind of map data. It moves on every few seconds until the reader
  // takes over: clicking a kind stops it for good; hovering or focusing pauses it; the pause button
  // stops and restarts it. With Reduce Motion it never moves on by itself.
  document.addEventListener("DOMContentLoaded", function () {
    var box = document.querySelector(".showcase");
    if (!box) { return; }
    var slides = box.querySelectorAll(".slide");
    var kinds = box.querySelectorAll(".kind");
    var caption = box.querySelector(".kind-caption");
    var pause = box.querySelector(".pause");
    var every = parseInt(box.getAttribute("data-rotate"), 10) || 3000;
    var calm = window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    var at = 0, timer = null, stopped = calm, held = false;
    function show(i) {
      at = (i + slides.length) % slides.length;
      for (var k = 0; k < slides.length; k++) {
        var on = k === at;
        slides[k].classList.toggle("is-on", on);
        if (on) { slides[k].removeAttribute("aria-hidden"); slides[k].loading = "eager"; } else { slides[k].setAttribute("aria-hidden", "true"); }
        if (kinds[k]) { kinds[k].setAttribute("aria-pressed", on ? "true" : "false"); }
      }
      if (caption && kinds[at]) { caption.textContent = kinds[at].getAttribute("data-caption"); }
    }
    function tick() { if (!stopped && !held && !document.hidden) { show(at + 1); } }
    function run() { clearInterval(timer); timer = stopped ? null : setInterval(tick, every); }
    function setStopped(v) {
      stopped = v;
      if (pause) {
        pause.setAttribute("aria-pressed", v ? "true" : "false");
        var zh = (root.lang || "").indexOf("zh") === 0;
        pause.setAttribute("aria-label", v ? (zh ? "继续轮播" : "Play the slideshow") : (zh ? "暂停轮播" : "Pause the slideshow"));
      }
      run();
    }
    for (var k = 0; k < kinds.length; k++) {
      kinds[k].addEventListener("click", function () { setStopped(true); show(parseInt(this.getAttribute("data-i"), 10)); });
    }
    if (pause) { pause.addEventListener("click", function () { setStopped(!stopped); }); }
    box.addEventListener("mouseenter", function () { held = true; });
    box.addEventListener("mouseleave", function () { held = false; });
    box.addEventListener("focusin", function () { held = true; });
    box.addEventListener("focusout", function () { held = false; });
    setStopped(stopped);
  });
})();
