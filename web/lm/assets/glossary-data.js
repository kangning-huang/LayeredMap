// NimbleMap glossary (WP6.10): the map words you meet in the app, in plain English and Chinese.
// Both glossary pages read this file; glossary.js draws and searches it in the browser.
// Fields: id (the #anchor), en/zh (the term), alt (other names people search for), d_en/d_zh
// (meaning), w_en/w_zh (where it shows in the app, with its labels as the app writes them),
// see (a workflow page and anchor where it is used, or "").
window.LM_GLOSSARY = [
  {
    id: "layer", en: "Layer", zh: "图层",
    alt: ["layers", "shelf", "overlay", "叠加"],
    d_en: "One set of things drawn on the map, such as every city, every storm track or a temperature grid. Layers stack like sheets of tracing paper: the one at the top of the list is drawn on top. Opening a file adds a layer; removing a layer never deletes your file.",
    d_zh: "地图上的一组内容，比如所有城市、所有台风路径，或者一张气温格网。图层像一叠描图纸那样层层叠放，列表里排在最上面的那层就画在最上面。打开一个文件就多一个图层；移除图层不会删掉你的原文件。",
    w_en: "The <span class=\"ui\">Layers</span> panel on the left (toolbar button <span class=\"ui\">Layers</span>, ⌘L). Each row has an eye to show or hide it; drag rows to reorder; right-click for <span class=\"ui\">Style…</span>, <span class=\"ui\">Zoom to layer</span>, <span class=\"ui\">Rename</span>, <span class=\"ui\">Remove from map</span>.",
    w_zh: "左侧的 <span class=\"ui\">Layers</span>（图层）面板，工具栏按钮 <span class=\"ui\">Layers</span>（⌘L）可开关。每一行前面的眼睛图标用来显示或隐藏；拖动行可以调整上下顺序；右键可选 <span class=\"ui\">Style…</span>、<span class=\"ui\">Zoom to layer</span>、<span class=\"ui\">Rename</span>、<span class=\"ui\">Remove from map</span>。",
    see: "workflow-typhoons.html#open"
  },
  {
    id: "basemap", en: "Basemap", zh: "底图",
    alt: ["background", "base map", "satellite", "tiles", "卫星图", "背景"],
    d_en: "The background map under your data: streets, coastlines, place names, or satellite imagery. It is there for orientation; it isn't one of your layers and isn't part of your data.",
    d_zh: "垫在你的数据下面的背景地图，可以是街道、海岸线和地名，也可以是卫星影像。它的作用是帮你认路，不算你的图层，也不属于你的数据。",
    w_en: "The <span class=\"ui\">Basemap</span> button at the bottom right of the map: <span class=\"ui\">Auto</span> (follows light or dark mode), <span class=\"ui\">Light</span>, <span class=\"ui\">Dark</span>, <span class=\"ui\">Satellite</span> (NASA Blue Marble) and <span class=\"ui\">Offline</span>, an outline map that needs no internet. <span class=\"ui\">Background strength</span> fades it so your data stands out.",
    w_zh: "地图右下角的 <span class=\"ui\">Basemap</span>（底图）按钮：<span class=\"ui\">Auto</span>（跟随浅色/深色模式）、<span class=\"ui\">Light</span>（浅色）、<span class=\"ui\">Dark</span>（深色）、<span class=\"ui\">Satellite</span>（卫星图，NASA “蓝色弹珠” 影像），以及 <span class=\"ui\">Offline</span>（不用联网的轮廓图）。<span class=\"ui\">Background strength</span> 可以把底图调淡，让数据更醒目。",
    see: "workflow-walled-cities.html#satellite"
  },
  {
    id: "vector", en: "Vector data", zh: "矢量数据",
    alt: ["vector", "points lines areas", "shapes", "矢量"],
    d_en: "Data made of shapes: points (a volcano), lines (a storm track) and areas (a city wall, a country). Each shape is one feature and carries its own facts. Vector data stays crisp at any zoom.",
    d_zh: "由形状组成的数据：点（一座火山）、线（一条台风路径）、面（一圈城墙、一个国家）。每个形状是一个要素，带着自己的属性信息。矢量数据怎么放大都清晰。",
    w_en: "A layer's subtitle says what it holds, for example <span class=\"ui\">4,999 lines</span> or <span class=\"ui\">1,251 points</span>. GeoJSON, GeoPackage, Shapefile, KML and CSV files with coordinates open as vector layers.",
    w_zh: "图层名称下面的小字会写明内容，例如 “4,999 lines”（4999 条线）或 “1,251 points”（1251 个点）。GeoJSON、GeoPackage、Shapefile、KML，以及带坐标的 CSV 文件，打开后都是矢量图层。",
    see: "workflow-typhoons.html#open"
  },
  {
    id: "raster", en: "Raster", zh: "栅格",
    alt: ["grid", "image", "pixels", "cells", "GeoTIFF", "格网", "像元"],
    d_en: "Data stored as a grid of cells, like the pixels of a photo, where every cell holds a number: an elevation, a temperature, a heatwave intensity. Zoom in far enough and you see the squares.",
    d_zh: "以格网形式存储的数据，就像照片由像素组成，每个格子（像元）里存一个数值：海拔、气温、热浪强度等。放得足够大时，能看到一个个小方块。",
    w_en: "Click a raster and the card shows that cell's value and where it stands (for example, how it compares with the rest of the grid). The <span class=\"ui\">Style</span> tab sets its <span class=\"ui\">Palette</span> and <span class=\"ui\">Range</span>.",
    w_zh: "点一下栅格，右侧卡片会显示这个格子的数值，以及它在整张格网里大概处于什么水平。在 <span class=\"ui\">Style</span>（样式）标签页可以调 <span class=\"ui\">Palette</span>（配色）和 <span class=\"ui\">Range</span>（取值范围）。",
    see: "workflow-heatwave.html#click"
  },
  {
    id: "feature", en: "Feature", zh: "要素",
    alt: ["object", "record", "row", "shape", "对象", "记录"],
    d_en: "One thing in a vector layer: one city, one storm, one walled town. On the map it is a shape; in the table it is one row.",
    d_zh: "矢量图层里的一个 “东西”：一座城市、一场台风、一座有城墙的县城。在地图上它是一个形状，在表格里它是一行。",
    w_en: "Click a feature on the map to select it. The card on the right shows its name, its facts, and up to three insight lines such as “highest of 4,999”.",
    w_zh: "在地图上点击一个要素即可选中。右侧卡片会显示它的名称、各项属性，以及最多三行对比信息，例如 “highest of 4,999”（4999 个里最高）。",
    see: "workflow-typhoons.html#click"
  },
  {
    id: "attribute", en: "Attribute", zh: "属性",
    alt: ["field", "column", "property", "value", "字段", "列"],
    d_en: "A fact stored with each feature, such as a storm's name, its season or its top wind speed. All the values of one attribute make a field, which is one column of the table.",
    d_zh: "跟每个要素存在一起的一项信息，例如台风的名字、年份、最大风速。同一项属性的所有值合起来叫一个字段，也就是表格里的一列。",
    w_en: "The card's facts list, the <span class=\"ui\">Values</span> part of a layer's <span class=\"ui\">Overview</span>, and the columns of <span class=\"ui\">View data</span>. <span class=\"ui\">Details</span> › <span class=\"ui\">Fields</span> lists each field and how the app reads it (<span class=\"ui\">Value</span>, <span class=\"ui\">Category</span>, <span class=\"ui\">Name</span>, <span class=\"ui\">Date</span>…).",
    w_zh: "卡片上的属性列表、图层 <span class=\"ui\">Overview</span>（概览）里的 <span class=\"ui\">Values</span> 部分，以及 <span class=\"ui\">View data</span>（查看数据）表格里的各列。<span class=\"ui\">Details</span> › <span class=\"ui\">Fields</span> 会列出每个字段，以及应用把它当作什么来理解（数值、类别、名称、日期等）。",
    see: "workflow-typhoons.html#table"
  },
  {
    id: "table", en: "Attribute table", zh: "属性表",
    alt: ["table", "spreadsheet", "view data", "表格", "数据表"],
    d_en: "The spreadsheet view of a layer: one row per feature, one column per attribute. Selecting a row selects the feature on the map, and the other way round.",
    d_zh: "图层的表格视图：一行是一个要素，一列是一项属性。在表格里选中一行，地图上对应的要素也会选中，反过来也一样。",
    w_en: "<span class=\"ui\">View data</span> on a card, or the layer's right-click menu. The drawer opens at the bottom of the map; click a column heading to sort.",
    w_zh: "卡片上的 <span class=\"ui\">View data</span>，或图层右键菜单里的同名项。表格从地图底部拉出；点列标题可以排序。",
    see: "workflow-typhoons.html#table"
  },
  {
    id: "filter", en: "Filter", zh: "筛选",
    alt: ["query", "select by attribute", "where", "show only", "过滤", "查询"],
    d_en: "Showing only the features that meet a condition, such as “strength is Category 5”. The others are hidden, not deleted, and the count of what is shown stays on screen.",
    d_zh: "只显示满足某个条件的要素，比如 “强度为五级”。其余的只是暂时隐藏，并没有删除；屏幕上会一直显示当前显示了多少个。",
    w_en: "The <span class=\"ui\">Filter</span> button on a card writes a sentence you finish, like <span class=\"ui\">Show storms where strength is any of Category 5</span>, then <span class=\"ui\">Apply</span>. The layer then reads <span class=\"ui\">Filtered:</span> with a <span class=\"ui\">Clear</span> link.",
    w_zh: "卡片上的 Filter（筛选）按钮会给出一句待补全的话，例如 “Show storms where strength is any of Category 5”（只显示强度为五级的台风），补全后点 <span class=\"ui\">Apply</span>。图层上随即出现 <span class=\"ui\">Filtered:</span>（已筛选）字样和 <span class=\"ui\">Clear</span>（清除）链接。",
    see: "workflow-typhoons.html#filter"
  },
  {
    id: "join", en: "Join", zh: "连接（表连接）",
    alt: ["table join", "attribute join", "merge", "match", "关联", "合并表格", "匹配"],
    d_en: "Attaching a table of facts to places by a column they share, such as a spreadsheet of GDP by country name added to a map of countries. NimbleMap finds the shared column itself and says how many places matched.",
    d_zh: "按双方共有的一列，把一张表格里的信息接到地图上的地方去。例如把一张按国家名称列出 GDP 的表格，接到国家地图上。凌波微图会自己找出这个共有列，并告诉你有多少个地方匹配上了。",
    w_en: "Open a table (a CSV or Excel file without coordinates) while a map layer is open. The table appears indented under the layer, for example <span class=\"ui\">joined · 172 of 177 matched</span>. Its menu has <span class=\"ui\">Show joined fields</span> and <span class=\"ui\">Unjoin</span>. A join needs a column whose values are unique in the layer (each country once); a column that repeats, like a type, can't be the key.",
    w_zh: "在已打开地图图层的情况下，再打开一张表格（不带坐标的 CSV 或 Excel 文件）。表格会缩进显示在图层下方，例如 “joined · 172 of 177 matched”（已连接，177 个中匹配 172 个）。它的菜单里有 <span class=\"ui\">Show joined fields</span>（显示连接的字段）和 <span class=\"ui\">Unjoin</span>（取消连接）。连接需要一列在图层里各不相同的值（每个国家只出现一次）；像 “类型” 这种会重复的列不能当作连接键。",
    see: ""
  },
  {
    id: "legend", en: "Legend", zh: "图例",
    alt: ["key", "colour key", "color key", "说明"],
    d_en: "The key that says what each colour or size means. When colours show a value, the legend shows the range; when they show categories, it lists each one.",
    d_zh: "说明每种颜色或大小代表什么的那块小图。颜色表示数值时，图例显示数值范围；颜色表示类别时，图例逐一列出各个类别。",
    w_en: "The <span class=\"ui\">Legend</span> section of a layer's <span class=\"ui\">Overview</span>, and in exported map images.",
    w_zh: "图层 <span class=\"ui\">Overview</span>（概览）里的 <span class=\"ui\">Legend</span> 部分；导出的地图图片里也有。",
    see: "workflow-walled-cities.html#color"
  },
  {
    id: "style", en: "Style (symbology)", zh: "样式（符号化）",
    alt: ["symbology", "symbol", "colour by", "color by", "choropleth", "配色", "专题图", "分级设色"],
    d_en: "How a layer looks: one colour, or colours that follow a value (dark red for strong storms) or a category (one colour per type of town). Colouring areas by a value is what cartographers call a choropleth map.",
    d_zh: "图层的外观：统一用一种颜色，或者让颜色跟着某个数值变化（越强的台风越红），或者按类别上色（每种城市类型一种颜色）。按数值给区域上色，制图学上叫分级设色图。",
    w_en: "The <span class=\"ui\">Style</span> tab: <span class=\"ui\">Color</span>, <span class=\"ui\">Palette</span>, <span class=\"ui\">Size</span>, <span class=\"ui\">Labels</span>, <span class=\"ui\">Opacity</span>, <span class=\"ui\">Outline</span>. The card's <span class=\"ui\">Color by a value</span> button is the quick way in.",
    w_zh: "<span class=\"ui\">Style</span>（样式）标签页：<span class=\"ui\">Color</span>（颜色）、<span class=\"ui\">Palette</span>（配色）、<span class=\"ui\">Size</span>（大小）、<span class=\"ui\">Labels</span>（标注）、<span class=\"ui\">Opacity</span>（不透明度）、<span class=\"ui\">Outline</span>（轮廓）。卡片上的 Color by a value（按数值上色）按钮是最快的入口。",
    see: "workflow-walled-cities.html#color"
  },
  {
    id: "projection", en: "Projection", zh: "投影",
    alt: ["map projection", "Web Mercator", "Mercator", "墨卡托", "地图投影"],
    d_en: "The way the round Earth is flattened onto a flat map. Every flat map stretches something: on the usual web map (Web Mercator) Greenland looks as big as Africa, though Africa is about 14 times larger. NimbleMap draws in Web Mercator, like most map apps, but measures distances and areas on the real globe.",
    d_zh: "把圆圆的地球摊平成平面地图的方法。任何平面地图都会有变形：在常见的网络地图（Web 墨卡托投影）上，格陵兰看上去和非洲差不多大，其实非洲约是它的 14 倍。凌波微图和大多数地图应用一样用 Web 墨卡托投影来显示，但计算距离和面积时按真实的地球来算。",
    w_en: "You don't need to choose one. A layer's <span class=\"ui\">Details</span> tab shows the file's own <span class=\"ui\">Coordinate system</span>; the app reprojects it for display and leaves the file untouched.",
    w_zh: "不需要你来选。图层的 <span class=\"ui\">Details</span>（详情）标签页会显示文件自带的 <span class=\"ui\">Coordinate system</span>（坐标系）；应用会自动换算以便显示，原文件不会被改动。",
    see: ""
  },
  {
    id: "crs", en: "Coordinate system (CRS)", zh: "坐标系（CRS）",
    alt: ["CRS", "coordinate reference system", "EPSG", "datum", "spatial reference", "坐标参考系", "大地基准"],
    d_en: "The rules that turn a pair of numbers into a place on Earth. The same numbers mean different places under different systems, so a file has to say which one it uses. The most common is WGS 84, the system GPS uses.",
    d_zh: "把一对数字对应到地球上某个位置的规则。同样的数字放在不同坐标系下，指的是不同的地方，所以文件需要说明自己用的是哪一种。最常见的是 WGS 84，也就是 GPS 用的那一种。",
    w_en: "<span class=\"ui\">Details</span> › <span class=\"ui\">Coordinate system</span>. When the app has to guess, the layer's card says so in one line under <span class=\"ui\">Assumptions</span>, with a <span class=\"ui\">Change</span> link.",
    w_zh: "<span class=\"ui\">Details</span> › <span class=\"ui\">Coordinate system</span>。如果应用只能推测，图层卡片的 <span class=\"ui\">Assumptions</span>（假设）里会用一行字说明，旁边有 <span class=\"ui\">Change</span>（更改）链接。",
    see: ""
  },
  {
    id: "wgs84", en: "WGS 84", zh: "WGS 84",
    alt: ["GPS coordinates", "EPSG:4326", "latitude longitude", "GPS 坐标"],
    d_en: "The worldwide coordinate system behind GPS: latitude and longitude in degrees. If a spreadsheet has latitude and longitude columns, they are almost always WGS 84.",
    d_zh: "GPS 背后的全球通用坐标系，用经纬度（单位为度）表示位置。表格里如果有经度、纬度两列，几乎都是 WGS 84。",
    w_en: "Cards say <span class=\"ui\">Located using longitude and latitude as GPS coordinates (WGS 84)</span>, with <span class=\"ui\">Change</span>.",
    w_zh: "卡片上会写 “Located using longitude and latitude as GPS coordinates (WGS 84)”（已按 GPS 坐标 WGS 84 用经纬度定位），后面有 <span class=\"ui\">Change</span>（更改）。",
    see: ""
  },
  {
    id: "gcj02", en: "GCJ-02 and BD-09", zh: "GCJ-02 与 BD-09（火星坐标、百度坐标）",
    alt: ["Mars coordinates", "Amap", "Gaode", "Tencent", "Baidu", "offset", "火星坐标", "高德", "腾讯", "百度", "坐标偏移"],
    d_en: "The coordinate systems used by maps in mainland China. GCJ-02 (Amap/Gaode, Tencent) shifts positions by a few hundred metres from GPS; BD-09 (Baidu) shifts them a little more. Points copied from these apps land a few streets off on any other map unless they are converted.",
    d_zh: "中国大陆地图使用的坐标系。GCJ-02（俗称 “火星坐标”，高德、腾讯地图使用）相对 GPS 偏移几百米；BD-09（百度地图使用）在此基础上又多偏一点。从这些地图里复制出来的点，如果不转换，放到别的地图上就会偏出几条街。",
    w_en: "When points look like Amap or Baidu coordinates, the app asks once: <span class=\"ui\">Amap/Tencent coordinates (GCJ-02): convert</span> or <span class=\"ui\">GPS coordinates (WGS 84): keep where they are</span>, and offers <span class=\"ui\">Compare on the map first</span>.",
    w_zh: "当点位看起来像高德或百度坐标时，应用会问你一次：选 “Amap/Tencent coordinates (GCJ-02): convert”（按高德/腾讯坐标转换），还是 “GPS coordinates (WGS 84): keep where they are”（按 GPS 坐标保持原位），也可以先点 <span class=\"ui\">Compare on the map first</span>（先在地图上对比）。",
    see: ""
  },
  {
    id: "latlon", en: "Latitude and longitude", zh: "纬度和经度",
    alt: ["lat", "lon", "lng", "coordinates", "degrees", "经纬度", "坐标"],
    d_en: "A place's address on the globe. Latitude runs from −90° at the South Pole to 90° at the North Pole; longitude from −180° to 180° east or west of Greenwich. Shanghai is about 31.2° N, 121.5° E.",
    d_zh: "地球上一个地点的 “门牌号”。纬度从南极的 −90° 到北极的 90°；经度从格林尼治向东、向西各 180°（−180° 到 180°）。上海大约在北纬 31.2°、东经 121.5°。",
    w_en: "A CSV with latitude and longitude columns (any common spelling: lat, lng, 纬度…) opens straight onto the map. Exported images can carry <span class=\"ui\">Lat/lon labels</span> and <span class=\"ui\">Grid lines</span>.",
    w_zh: "带经纬度列的 CSV（lat、lng、纬度等常见写法都认）会直接显示在地图上。导出图片时可以加 <span class=\"ui\">Lat/lon labels</span>（经纬度标注）和 <span class=\"ui\">Grid lines</span>（经纬网）。",
    see: ""
  },
  {
    id: "extent", en: "Extent", zh: "范围（外包框）",
    alt: ["bounds", "bounding box", "bbox", "边界", "外接矩形"],
    d_en: "The rectangle that just contains all of a layer's data, from its westernmost to easternmost and southernmost to northernmost point.",
    d_zh: "刚好把一个图层全部数据框住的矩形，从最西到最东、从最南到最北。",
    w_en: "<span class=\"ui\">Details</span> › <span class=\"ui\">Extent</span>; <span class=\"ui\">Zoom to layer</span> and View › <span class=\"ui\">Zoom to All Data</span> (⌘0) fly to it.",
    w_zh: "<span class=\"ui\">Details</span> › <span class=\"ui\">Extent</span>；<span class=\"ui\">Zoom to layer</span>（缩放到图层）和 View 菜单里的 <span class=\"ui\">Zoom to All Data</span>（⌘0）都会飞到这个范围。",
    see: ""
  },
  {
    id: "scale", en: "Scale and zoom level", zh: "比例尺与缩放级别",
    alt: ["zoom", "scale bar", "resolution", "缩放", "比例"],
    d_en: "How much of the Earth fits on the screen. Zoom level 0 shows the whole world in one tile; each step in doubles the detail. The scale bar shows what a distance on screen is on the ground.",
    d_zh: "屏幕上能装下多大一块地球。缩放级别 0 用一张瓦片显示整个世界，每放大一级，细节翻一倍。比例尺告诉你屏幕上的一段长度对应地面上多远。",
    w_en: "The scale bar at the bottom left of the map; <span class=\"ui\">Zoom in</span> and <span class=\"ui\">Zoom out</span> at the bottom right (⌘+ and ⌘−). View › <span class=\"ui\">Metric Units</span> or <span class=\"ui\">Imperial Units</span>.",
    w_zh: "地图左下角是比例尺；右下角是 <span class=\"ui\">Zoom in</span> / <span class=\"ui\">Zoom out</span>（放大/缩小，⌘+ 和 ⌘−）。在 View 菜单里可选 <span class=\"ui\">Metric Units</span>（公制）或 <span class=\"ui\">Imperial Units</span>（英制）。",
    see: ""
  },
  {
    id: "geodesic", en: "Geodesic", zh: "测地线（大地线）",
    alt: ["great circle", "true distance", "ellipsoid", "大圆", "球面距离", "真实距离"],
    d_en: "Measured along the curved surface of the Earth, the way a plane actually flies, instead of along a straight line on a flat map. On a flat map the shortest route from Shanghai to New York looks curved; geodesically, it is the straight one. NimbleMap measures distances and areas this way, so a 100 km circle is 100 km everywhere, near the equator or near the poles.",
    d_zh: "沿着弯曲的地球表面来量，就像飞机真正飞的航线，而不是在平面地图上拉一条直线。在平面地图上，上海飞纽约的最短航线看起来是弯的；按测地线算，它才是 “直” 的。凌波微图计算距离和面积都用这种方法，所以 100 公里的圆，在赤道附近和在两极附近都是真正的 100 公里。",
    w_en: "Behind every distance and area the app shows: the scale bar, area facts such as <span class=\"ui\">area · 23.4 km²</span>, and analysis distances such as “within 100 km”.",
    w_zh: "应用里显示的所有距离和面积都按这种方式计算：比例尺、“area · 23.4 km²” 这样的面积信息，以及分析里 “100 公里以内” 这类距离。",
    see: ""
  },
  {
    id: "buffer", en: "Buffer", zh: "缓冲区",
    alt: ["within a distance", "radius", "circle around", "距离范围", "周边", "半径"],
    d_en: "The area within a set distance of something: everywhere within 100 km of Hong Kong, or within 50 km of each storm track. GIS software calls it a buffer; NimbleMap calls it “Show an area within a distance”.",
    d_zh: "离某个东西一定距离以内的区域，比如香港周边 100 公里以内，或每条台风路径两侧 50 公里以内。GIS 软件里叫 “缓冲区”，凌波微图里叫 “Show an area within a distance”（显示一定距离内的区域）。",
    w_en: "The card's <span class=\"ui\">Show an area within a distance of each</span> button, free for everyone. In NimbleMap Pro you can also ask in the Ask box, for example “Show everything within 100 km of Hong Kong”.",
    w_zh: "卡片上的 <span class=\"ui\">Show an area within a distance of each</span> 按钮，所有人都能免费使用。在凌波微图 Pro 里，还可以在提问框里直接问，例如 “显示香港周边 100 公里以内的范围”。",
    see: "workflow-typhoons.html#pro"
  },
  {
    id: "clip", en: "Clip", zh: "裁剪",
    alt: ["cut", "keep inside", "crop", "intersect", "相交", "截取"],
    d_en: "Keeping only the part of a layer that lies inside an area, like cutting a map with scissors along a border. The source file is never changed; the result is a new layer.",
    d_zh: "只保留图层落在某个区域里的那部分，就像沿着边界用剪刀剪地图。原文件不会被改动，结果是一个新图层。",
    w_en: "The card's <span class=\"ui\">Keep the part of [layer] inside [other]</span> button, free for everyone; the result's recipe names the step the same way. In NimbleMap Pro you can also ask, for example “Keep the part of the storm tracks inside the area I drew”.",
    w_zh: "卡片上的 <span class=\"ui\">Keep the part of [layer] inside [other]</span> 按钮，所有人都能免费使用；结果的步骤说明里也这样写这一步。在凌波微图 Pro 里，还可以直接提问，例如 “只保留我画的区域里面那一段台风路径”。",
    see: ""
  },
  {
    id: "netcdf", en: "NetCDF", zh: "NetCDF",
    alt: [".nc", "nc4", "climate data", "ERA5", "CMIP6", "气候数据", "再分析"],
    d_en: "A file format scientists use for climate and ocean data: grids that change over time and often hold several variables (temperature, rainfall, wind). NimbleMap opens .nc, .nc4 and .cdf files like any raster, with a time slider and a variable picker.",
    d_zh: "科学家常用来存气候和海洋数据的文件格式：随时间变化的格网，里面往往有好几个变量（气温、降水、风速等）。凌波微图能像打开普通栅格一样打开 .nc、.nc4、.cdf 文件，并配有时间滑块和变量选择。",
    w_en: "The example <span class=\"ui\">Heatwaves, summer 2023</span>. Under the layer's name in the <span class=\"ui\">Layers</span> panel, the <span class=\"ui\">Variable shown</span> pop-up picks the variable; a <span class=\"ui\">Level shown</span> pop-up appears for data with depths or heights.",
    w_zh: "示例 <span class=\"ui\">Heatwaves, summer 2023</span>（2023 年夏季热浪）。在 <span class=\"ui\">Layers</span> 面板里图层名下方，<span class=\"ui\">Variable shown</span> 弹出菜单用来选变量；数据有深度或高度层时，还会出现 <span class=\"ui\">Level shown</span>（显示的层）。",
    see: "workflow-heatwave.html#open"
  },
  {
    id: "timestep", en: "Time step", zh: "时间步",
    alt: ["time slider", "frame", "date", "animation", "play", "时间轴", "时间滑块", "动画"],
    d_en: "One moment in data that changes over time: one day, one month or one year of a grid. The map shows one step at a time. The colour range stays the same for every step, so a change of colour is a real change of value.",
    d_zh: "随时间变化的数据里的一个时刻：格网中的某一天、某一月或某一年。地图一次只显示一个时间步。所有时间步共用同一套颜色范围，所以颜色变了，就说明数值真的变了。",
    w_en: "The time slider at the bottom of the map: <span class=\"ui\">Previous step (←)</span>, <span class=\"ui\">Play</span>, <span class=\"ui\">Next step (→)</span>, the date (such as “16 Jul 2023”) and the step count. The ← and → keys step too.",
    w_zh: "地图底部的时间滑块：<span class=\"ui\">Previous step (←)</span>（上一步）、<span class=\"ui\">Play</span>（播放）、<span class=\"ui\">Next step (→)</span>（下一步），以及日期（如 “16 Jul 2023”）和第几步。键盘 ← → 也能逐步切换。",
    see: "workflow-heatwave.html#slider"
  },
  {
    id: "variable", en: "Variable", zh: "变量",
    alt: ["band", "parameter", "measure", "波段", "要素变量"],
    d_en: "One kind of measurement in a gridded file, such as 2 m air temperature or heatwave intensity, with its unit. A NetCDF file can hold many; the map shows one at a time.",
    d_zh: "格网文件里的一种测量量，比如 2 米气温、热浪强度，并带有单位。一个 NetCDF 文件里可以有很多个变量，地图一次显示一个。",
    w_en: "The <span class=\"ui\">Variable shown</span> pop-up under the layer's name, for example “2 m temperature (°C)”.",
    w_zh: "图层名下方的 <span class=\"ui\">Variable shown</span> 弹出菜单，例如 “2 m temperature (°C)”。",
    see: "workflow-heatwave.html#open"
  },
  {
    id: "nodata", en: "No data", zh: "无数据（NoData）",
    alt: ["NoData", "missing value", "fill value", "null", "blank", "缺测", "空值"],
    d_en: "A cell or field with no value, which is different from zero. In the heatwave example, days and places without a heatwave are no data, so they are left clear rather than painted as “0”.",
    d_zh: "一个格子或字段没有值，这和 “0” 不是一回事。在热浪示例里，没有热浪的日子和地方就是无数据，所以那里留空不上色，而不是画成 “0”。",
    w_en: "Clicking such a place shows <span class=\"ui\">No value</span> or <span class=\"ui\">No data</span>. Summaries say how many were left out.",
    w_zh: "点到这样的位置会显示 <span class=\"ui\">No value</span> 或 <span class=\"ui\">No data</span>。统计结果会说明排除了多少个。",
    see: "workflow-heatwave.html#click"
  },
  {
    id: "resolution", en: "Resolution (cell size)", zh: "分辨率（像元大小）",
    alt: ["cell size", "pixel size", "0.5 degree", "grid spacing", "像元", "格距"],
    d_en: "How big each cell of a raster is on the ground. The heatwave and elevation examples use cells of 0.5° (about 55 km at the equator), so each value is an average over a big square, not a single spot.",
    d_zh: "栅格里每个格子在地面上有多大。热浪和高程示例的格子都是 0.5°（赤道附近约 55 公里），所以每个值是一大块方格的平均，不是某一个点的值。",
    w_en: "<span class=\"ui\">Details</span> for a raster layer.",
    w_zh: "栅格图层的 <span class=\"ui\">Details</span>（详情）标签页。",
    see: ""
  },
  {
    id: "insight", en: "Insight line", zh: "对比信息",
    alt: ["rank", "compare", "median", "where it stands", "排名", "中位数"],
    d_en: "A sentence the app writes when you select something, telling you where it stands among the rest, such as “highest of 4,999” or “lower than 80% of …”. It comes before any tool, so you learn something just by clicking.",
    d_zh: "选中某个东西时，应用自动写出的一句话，告诉你它在同类里处于什么位置，比如 “highest of 4,999”（4999 个里最高）或 “lower than 80% of …”（比 80% 的……低）。它出现在任何工具之前，点一下就能有所发现。",
    w_en: "The top of the card after you click a feature or a raster cell. <span class=\"ui\">Compare with other [layer]</span> shows it as a ranked chart.",
    w_zh: "点击要素或栅格格子后，卡片的最上方。<span class=\"ui\">Compare with other …</span>（与其他……比较）会把它画成排名图。",
    see: "workflow-typhoons.html#click"
  },
  {
    id: "assumption", en: "Assumption", zh: "假设",
    alt: ["guess", "interpretation", "detected", "推断", "识别"],
    d_en: "A choice the app made for you when opening a file, such as which columns hold coordinates or what a column means. Instead of asking questions, NimbleMap applies its best guess, says it in one line, and lets you change it in one click.",
    d_zh: "打开文件时应用替你做的判断，比如哪几列是坐标、某一列是什么意思。凌波微图不会一上来就问一堆问题，而是先按最可能的理解处理，用一行字告诉你，需要时点一下就能改。",
    w_en: "The <span class=\"ui\">Assumptions</span> part of a layer's card, each with <span class=\"ui\">Change</span>.",
    w_zh: "图层卡片里的 <span class=\"ui\">Assumptions</span>（假设）部分，每条后面都有 <span class=\"ui\">Change</span>（更改）。",
    see: ""
  },
  {
    id: "recipe", en: "Recipe", zh: "步骤说明（配方）",
    alt: ["result card", "method", "provenance", "how it was made", "方法", "来源"],
    d_en: "The plain-words record of how a result was made: which layers went in, which step ran, over what scope, and what was left out. You can copy it as text and check it.",
    d_zh: "用大白话记录一个结果是怎么得出来的：用了哪些图层、做了哪一步、范围是什么、排除了什么。可以复制成文字，方便核对。",
    w_en: "Every analysis result, from a card button or (in NimbleMap Pro) the Ask box, comes as a result card with its steps listed, a preview on the map, and <span class=\"ui\">Keep</span> or <span class=\"ui\">Discard</span>.",
    w_zh: "每个分析结果，无论来自卡片上的按钮还是凌波微图 Pro 的提问框，都带一张结果卡片，列出各个步骤，并在地图上预览，你可以选 <span class=\"ui\">Keep</span>（保留）或 <span class=\"ui\">Discard</span>（放弃）。",
    see: "workflow-typhoons.html#pro"
  },
  {
    id: "geojson", en: "GeoJSON, GeoPackage, Shapefile", zh: "GeoJSON、GeoPackage、Shapefile",
    alt: [".geojson", ".gpkg", ".shp", "file formats", "KML", "GPX", "文件格式"],
    d_en: "Common files for vector data. GeoJSON is a single text file; GeoPackage (.gpkg) is a single database file that can hold several layers; a Shapefile is a set of files with the same name (.shp, .dbf, .shx, .prj) that belong together.",
    d_zh: "常见的矢量数据文件。GeoJSON 是一个文本文件；GeoPackage（.gpkg）是一个数据库文件，可以装好几个图层；Shapefile 是一组同名文件（.shp、.dbf、.shx、.prj），要放在一起使用。",
    w_en: "Toolbar <span class=\"ui\">Open</span> (⌘O), or drag files into NimbleMap (<span class=\"ui\">Drop to add to the map</span>). <span class=\"ui\">Details</span> › <span class=\"ui\">Format</span> names the format.",
    w_zh: "工具栏 <span class=\"ui\">Open</span>（⌘O），或直接把文件拖进凌波微图（会提示 “Drop to add to the map”）。<span class=\"ui\">Details</span> › <span class=\"ui\">Format</span> 会写明格式。",
    see: ""
  },
  {
    id: "geotiff", en: "GeoTIFF", zh: "GeoTIFF",
    alt: [".tif", ".tiff", "DEM", "elevation", "satellite image", "高程", "影像"],
    d_en: "An image file that also knows where on Earth it sits. Elevation models and satellite images often come as GeoTIFFs.",
    d_zh: "一种知道自己在地球上哪个位置的图像文件。高程模型和卫星影像常常是 GeoTIFF 格式。",
    w_en: "The <span class=\"ui\">World basics</span> example's elevation layer is a GeoTIFF.",
    w_zh: "示例 <span class=\"ui\">World basics</span>（世界基础数据）里的高程图层就是 GeoTIFF。",
    see: ""
  },
  {
    id: "export", en: "Map image (export)", zh: "导出地图图片",
    alt: ["export", "share", "PNG", "save image", "print", "分享", "导出", "截图"],
    d_en: "A finished picture of your map with a title, legend, scale bar and credits, drawn at full size for a slide, a report page or a social post.",
    d_zh: "把地图做成一张完整的图片，带标题、图例、比例尺和数据来源，按实际尺寸绘制，可用于幻灯片、报告或社交媒体。",
    w_en: "Toolbar <span class=\"ui\">Share</span> › <span class=\"ui\">Map image…</span> (⌘E). Sizes: <span class=\"ui\">Slide 16:9</span>, <span class=\"ui\">Report A4</span>, <span class=\"ui\">Square</span>. Options: <span class=\"ui\">Title</span>, <span class=\"ui\">Caption</span>, <span class=\"ui\">Legend</span>, <span class=\"ui\">Scale bar</span>, <span class=\"ui\">North arrow</span>, <span class=\"ui\">Lat/lon labels</span>.",
    w_zh: "工具栏 <span class=\"ui\">Share</span> › <span class=\"ui\">Map image…</span>（⌘E）。尺寸可选 <span class=\"ui\">Slide 16:9</span>（幻灯片）、<span class=\"ui\">Report A4</span>（A4 报告）、<span class=\"ui\">Square</span>（方形）；选项有 <span class=\"ui\">Title</span>（标题）、<span class=\"ui\">Caption</span>（说明）、<span class=\"ui\">Legend</span>（图例）、<span class=\"ui\">Scale bar</span>（比例尺）、<span class=\"ui\">North arrow</span>（指北针）、<span class=\"ui\">Lat/lon labels</span>（经纬度标注）。",
    see: "workflow-heatwave.html#export"
  },
  {
    id: "offline", en: "Work offline", zh: "离线工作",
    alt: ["offline", "no internet", "network", "privacy", "断网", "隐私"],
    d_en: "NimbleMap reads your files on your own computer; nothing you open is uploaded. Online services are used only for the basemap, place search and update checks, and one switch turns all of them off.",
    d_zh: "凌波微图在你自己的电脑上读取文件，你打开的任何数据都不会上传。只有底图、地名搜索和检查更新会用到网络，而且一个开关就能把它们全部关掉。",
    w_en: "View › <span class=\"ui\">Work Offline</span>. The basemap then switches to the offline outline map (<span class=\"ui\">Working offline: online backgrounds are off</span>).",
    w_zh: "View 菜单 › <span class=\"ui\">Work Offline</span>。开启后底图会换成离线轮廓图，并提示 “Working offline: online backgrounds are off”。",
    see: ""
  },
  {
    id: "ask", en: "Ask box (Pro)", zh: "提问框（Pro）",
    alt: ["assistant", "AI", "natural language", "question", "chat", "LLM", "助手", "人工智能", "自然语言"],
    d_en: "In NimbleMap Pro, a box at the bottom of the map where you ask about your layers in plain words, typed or spoken, in English or Chinese. The assistant picks the app's own checked steps; the numbers are always computed on your computer, and only descriptions of your layers (names, fields, extent), never their values, are sent.",
    d_zh: "凌波微图 Pro 里位于地图底部的输入框，你可以用大白话（打字或语音，中英文都行）询问关于图层的问题。助手只负责挑选应用里经过检验的分析步骤；所有数字都在你的电脑上计算，发送出去的只有图层的描述（名称、字段、范围），绝不包括数据值本身。",
    w_en: "The field <span class=\"ui\">Ask about your layers</span> at the bottom of the map, with a microphone for dictation.",
    w_zh: "地图底部写着 “Ask about your layers” 的输入框，旁边有麦克风按钮可以语音输入。",
    see: "workflow-typhoons.html#pro"
  }
];
