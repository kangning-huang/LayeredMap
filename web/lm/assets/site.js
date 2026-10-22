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
  // The few words this script writes, per page language (the pages carry everything else).
  var WORDS = {
    en: { dark: "Switch to dark mode", light: "Switch to light mode", play: "Play the slideshow", pause: "Pause the slideshow", vplay: "Play the video", vpause: "Pause the video" },
    zh: { dark: "切换到深色", light: "切换到浅色", play: "继续轮播", pause: "暂停轮播", vplay: "播放视频", vpause: "暂停视频" },
    de: { dark: "Dunkles Design", light: "Helles Design", play: "Diashow abspielen", pause: "Diashow anhalten", vplay: "Video abspielen", vpause: "Video anhalten" },
    es: { dark: "Cambiar a modo oscuro", light: "Cambiar a modo claro", play: "Reanudar la presentación", pause: "Pausar la presentación", vplay: "Reproducir el vídeo", vpause: "Pausar el vídeo" },
    fr: { dark: "Passer en mode sombre", light: "Passer en mode clair", play: "Lire le diaporama", pause: "Mettre le diaporama en pause", vplay: "Lire la vidéo", vpause: "Mettre la vidéo en pause" },
    it: { dark: "Passa al tema scuro", light: "Passa al tema chiaro", play: "Riproduci la presentazione", pause: "Metti in pausa la presentazione", vplay: "Riproduci il video", vpause: "Metti in pausa il video" },
    nl: { dark: "Donkere modus", light: "Lichte modus", play: "Diavoorstelling afspelen", pause: "Diavoorstelling pauzeren", vplay: "Video afspelen", vpause: "Video pauzeren" },
    pt: { dark: "Mudar para o modo escuro", light: "Mudar para o modo claro", play: "Retomar a apresentação", pause: "Pausar a apresentação", vplay: "Reproduzir o vídeo", vpause: "Pausar o vídeo" },
    ru: { dark: "Тёмная тема", light: "Светлая тема", play: "Продолжить показ", pause: "Приостановить показ", vplay: "Воспроизвести видео", vpause: "Приостановить видео" },
    ja: { dark: "ダークモードに切り替え", light: "ライトモードに切り替え", play: "スライドショーを再生", pause: "スライドショーを一時停止", vplay: "動画を再生", vpause: "動画を一時停止" },
    ko: { dark: "다크 모드로 전환", light: "라이트 모드로 전환", play: "슬라이드쇼 재생", pause: "슬라이드쇼 일시 정지", vplay: "동영상 재생", vpause: "동영상 일시 정지" }
  };
  var W = WORDS[(root.lang || "en").split("-")[0].toLowerCase()] || WORDS.en;

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
    function label() {
      var next = current() === "dark" ? "light" : "dark";
      btn.setAttribute("aria-label", W[next]);
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

  // The language menu: closes on Escape or a click elsewhere (without JavaScript it closes on its
  // own summary, which is enough).
  document.addEventListener("DOMContentLoaded", function () {
    var menu = document.querySelector(".lang-menu");
    if (!menu) { return; }
    document.addEventListener("click", function (ev) { if (menu.open && !menu.contains(ev.target)) { menu.open = false; } });
    document.addEventListener("keydown", function (ev) {
      if (ev.key === "Escape" && menu.open) { menu.open = false; menu.querySelector("summary").focus(); }
    });
  });

  // The film under the hero: muted and looping, with a pause button. It plays while it is on screen
  // and pauses when scrolled away or when the tab is hidden. With Reduce Motion it never plays
  // (the still shows instead, site.css).
  document.addEventListener("DOMContentLoaded", function () {
    var film = document.querySelector(".hero-film video");
    if (!film) { return; }
    var button = document.querySelector(".hero-film .film-pause");
    var calm = window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    var stopped = false;
    function label() {
      if (!button) { return; }
      button.setAttribute("aria-pressed", stopped ? "true" : "false");
      button.setAttribute("aria-label", stopped ? W.vplay : W.vpause);
    }
    if (calm) {
      film.removeAttribute("autoplay");
      film.preload = "none";
      film.pause();
      return;
    }
    var seen = !("IntersectionObserver" in window);
    function update() {
      if (stopped || !seen || document.hidden) { film.pause(); return; }
      var p = film.play(); if (p && p.catch) { p.catch(function () {}); }
    }
    label();
    if (!seen) {
      film.removeAttribute("autoplay");
      film.pause();
      new IntersectionObserver(function (entries) {
        seen = entries[0].isIntersecting;
        update();
      }, { threshold: 0.4 }).observe(film);
    }
    if (button) {
      button.addEventListener("click", function () {
        stopped = !stopped;
        update();
        label();
      });
    }
    document.addEventListener("visibilitychange", update);
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
        pause.setAttribute("aria-label", v ? W.play : W.pause);
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

// Share buttons: built from the page's canonical address and title, so each language shares
// itself. Hidden without JavaScript (see .share in site.css).
document.addEventListener("DOMContentLoaded", function () {
  var links = document.querySelectorAll(".share a[data-net]");
  if (!links.length) return;
  var canon = document.querySelector('link[rel="canonical"]');
  var url = encodeURIComponent(canon ? canon.href : location.href);
  var title = encodeURIComponent(document.title);
  var to = {
    x: "https://x.com/intent/post?url=" + url + "&text=" + title,
    facebook: "https://www.facebook.com/sharer/sharer.php?u=" + url,
    reddit: "https://www.reddit.com/submit?url=" + url + "&title=" + title,
    linkedin: "https://www.linkedin.com/sharing/share-offsite/?url=" + url,
  };
  for (var i = 0; i < links.length; i++) {
    var net = links[i].getAttribute("data-net");
    if (to[net]) links[i].href = to[net];
  }
});

// Mac and Windows downloads are both available; each button names its platform.
