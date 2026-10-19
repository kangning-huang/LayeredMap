// NimbleMap glossary (WP6.10): draws window.LM_GLOSSARY and searches it as you type, in the
// browser. Matches the term, its other names and its meaning, in the page's language and in
// English (and Chinese), so "buffer", "缓冲区" or "火星坐标" all find their entry. ?q=… in the address fills the box.
(function () {
  var data = window.LM_GLOSSARY || [];
  // Other languages: glossary-<lang>.js sets LM_GLOSSARY_L10N, with the page's words (ui) and each
  // term's name, other names, meaning and place in the app (terms[id]: t, alt, d, w). Entries it
  // lacks show in English. English and Chinese live in glossary-data.js itself.
  var L = window.LM_GLOSSARY_L10N || null;
  var lang = (document.documentElement.lang || "en");
  var zh = !L && lang.indexOf("zh") === 0;
  var list = document.getElementById("glossary");
  var input = document.getElementById("q");
  var count = document.getElementById("count");
  var letters = document.getElementById("letters");
  if (!list || !input) { return; }

  var T = L ? L.ui : zh
    ? { where: "在应用里", see: "在教程中查看", all: function (n) { return "共 " + n + " 个词条"; }, some: function (n, t) { return "找到 " + n + " 个（共 " + t + " 个）"; }, none: "没有找到。试试换个说法，或者用英文搜索。", also: "又称" }
    : { where: "In the app", see: "See it in a workflow", all: function (n) { return n + " terms"; }, some: function (n, t) { return n + " of " + t + " terms"; }, none: "Nothing matches. Try another word, or search in Chinese.", also: "Also" };

  var PAGES = L ? L.pages : zh
    ? { "workflow-typhoons.html": "台风：哪些风暴最猛烈", "workflow-heatwave.html": "2023 年热浪：随时间变化", "workflow-walled-cities.html": "1866 年的中国城墙" }
    : { "workflow-typhoons.html": "Typhoons: the strongest storms", "workflow-heatwave.html": "The 2023 heatwave over time", "workflow-walled-cities.html": "Walled cities of China, 1866" };

  // Each entry's text in the page's language: name, meaning, where, and the other language's name.
  data.forEach(function (e) {
    var x = L && L.terms[e.id];
    if (x) {
      e._t = x.t; e._d = x.d; e._w = x.w; e._other = e.en; e._alt = (x.alt || []).concat(e.alt);
    } else if (zh) {
      e._t = e.zh; e._d = e.d_zh; e._w = e.w_zh; e._other = e.en; e._alt = e.alt;
    } else {
      e._t = e.en; e._d = e.d_en; e._w = e.w_en; e._other = L ? "" : e.zh; e._alt = e.alt;
    }
  });

  function strip(html) { var d = document.createElement("div"); d.innerHTML = html; return d.textContent || ""; }
  function fold(s) { return s.toLowerCase().normalize("NFKD").replace(/[\u0300-\u036f]/g, ""); }

  var sorted = data.slice().sort(function (a, b) {
    return zh ? a.zh.localeCompare(b.zh, "zh-Hans-CN") : a._t.localeCompare(b._t, lang);
  });
  sorted.forEach(function (e) {
    e._hay = fold([e._t, e.en, e.zh, e._alt.join(" "), strip(e._d), strip(e.d_en), strip(e.d_zh)].join(" \u0001 "));
    e._name = fold([e._t, e.en, e.zh, e._alt.join(" ")].join(" "));
  });

  function esc(s) { return s.replace(/[&<>"]/g, function (c) { return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]; }); }

  // Highlight the query in text nodes only, so the app-label markup stays intact.
  function mark(node, q) {
    if (!q) { return; }
    var walker = document.createTreeWalker(node, NodeFilter.SHOW_TEXT, null);
    var texts = [];
    while (walker.nextNode()) { texts.push(walker.currentNode); }
    texts.forEach(function (t) {
      var i = fold(t.nodeValue).indexOf(q);
      if (i < 0 || t.nodeValue.length !== fold(t.nodeValue).length) { return; }
      var m = document.createElement("mark");
      var after = t.splitText(i);
      after.splitText(q.length);
      m.textContent = after.nodeValue;
      after.parentNode.replaceChild(m, after);
    });
  }

  function render(q) {
    var words = fold(q.trim()).split(/\s+/).filter(Boolean);
    var hits = sorted.filter(function (e) { return words.every(function (w) { return e._hay.indexOf(w) >= 0; }); });
    // Name matches first, then matches in the meaning.
    if (words.length) {
      hits.sort(function (a, b) {
        var an = words.every(function (w) { return a._name.indexOf(w) >= 0; }) ? 0 : 1;
        var bn = words.every(function (w) { return b._name.indexOf(w) >= 0; }) ? 0 : 1;
        return an - bn;
      });
    }
    list.innerHTML = "";
    hits.forEach(function (e) {
      var div = document.createElement("div");
      div.className = "entry";
      div.id = e.id;
      var main = e._t;
      var other = e._other;
      var alt = e._alt.filter(function (a) { return a !== main && a !== other; }).slice(0, 5);
      div.innerHTML =
        "<dt>" + esc(main) + (other && other !== main ? " <span class=\"alt\">" + esc(other) + "</span>" : "") +
        " <a class=\"anchor\" href=\"#" + e.id + "\" aria-label=\"#" + e.id + "\">#</a></dt>" +
        "<dd><p>" + e._d + "</p>" +
        "<div class=\"where\"><b>" + T.where + "</b><span>" + e._w + "</span></div>" +
        (e.see ? "<div class=\"where\"><b>" + T.see + "</b><a href=\"" + e.see + "\">" + esc(PAGES[e.see.split("#")[0]] || e.see) + " →</a></div>" : "") +
        (alt.length ? "<div class=\"where\"><b>" + T.also + "</b><span>" + alt.map(esc).join(" · ") + "</span></div>" : "") +
        "</dd>";
      words.forEach(function (w) { mark(div.querySelector("dt"), w); mark(div.querySelector("dd p"), w); });
      list.appendChild(div);
    });
    if (!hits.length) { list.innerHTML = "<p class=\"empty\">" + T.none + "</p>"; }
    count.textContent = words.length ? T.some(hits.length, sorted.length) : T.all(sorted.length);
    if (letters) { letters.hidden = words.length > 0; }
  }

  // An index of first letters for quick jumps, in alphabetic scripts.
  if (letters && !zh && !/^(ja|ko)/.test(lang)) {
    var seen = {};
    sorted.forEach(function (e) {
      var c = e._t.charAt(0).toUpperCase();
      if (!seen[c]) { seen[c] = e.id; var a = document.createElement("a"); a.href = "#" + e.id; a.textContent = c; letters.appendChild(a); }
    });
  }

  var params = new URLSearchParams(location.search);
  input.value = params.get("q") || "";
  render(input.value);
  var timer;
  input.addEventListener("input", function () {
    clearTimeout(timer);
    timer = setTimeout(function () {
      render(input.value);
      var u = new URL(location.href);
      if (input.value) { u.searchParams.set("q", input.value); } else { u.searchParams.delete("q"); }
      u.hash = "";
      try { history.replaceState(null, "", u); } catch (e) { /* file:// in some browsers */ }
    }, 80);
  });
  document.addEventListener("keydown", function (ev) {
    if (ev.key === "/" && document.activeElement !== input) { ev.preventDefault(); input.focus(); }
    if (ev.key === "Escape" && document.activeElement === input) { input.value = ""; render(""); }
  });
  if (location.hash) {
    var t = document.getElementById(location.hash.slice(1));
    if (t) { t.scrollIntoView(); }
  }
})();
