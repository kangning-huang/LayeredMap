"""Write the Data page's card illustrations: web/lm/img/data/<slug>.svg (320x180, 16:9).

    python3 -I tools/lm/site/data_art.py            # rewrite web/lm/img/data/*.svg


One shared palette (the site's tokens plus an ochre and a sage), flat shapes, a soft ground
shadow and a faint graticule behind each subject. Each file carries its own light palette and a
prefers-color-scheme: dark one, because an <img> SVG can't read the page's CSS variables.
"""
import sys
from pathlib import Path

OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parents[3] / "web" / "lm" / "img" / "data"

STYLE = """<style>
.b{fill:#efe9dd}.g{stroke:#1d232b;stroke-opacity:.07;stroke-width:1;fill:none}.sh{fill:#1d232b;fill-opacity:.09}
.k{fill:#2b323b}.k2{fill:#4a525c}.k3{fill:#8a9098}.r{fill:#d6cdbb}.r2{fill:#c4b9a3}.w{fill:#fffdf8}
.a{fill:#2563eb}.a2{fill:#6f95ee}.a3{fill:#c7d5f6}.s{fill:#b83a26}.s2{fill:#d9704f}.s3{fill:#f1c4b2}
.o{fill:#e0a33a}.o2{fill:#f4d58b}.v{fill:#5f8a6a}.v2{fill:#9fbf9c}
.ln{fill:none;stroke-linecap:round;stroke-linejoin:round}
.lk{stroke:#2b323b}.lk2{stroke:#4a525c}.lk3{stroke:#8a9098}.la{stroke:#2563eb}.la2{stroke:#6f95ee}.la3{stroke:#c7d5f6}
.ls{stroke:#b83a26}.ls2{stroke:#d9704f}.lo{stroke:#e0a33a}.lo2{stroke:#f4d58b}.lw{stroke:#fffdf8}.lr{stroke:#d6cdbb}.lb{stroke:#efe9dd}.lv{stroke:#5f8a6a}
@media (prefers-color-scheme:dark){
.b{fill:#1d2633}.g{stroke:#e8e4da;stroke-opacity:.07}.sh{fill:#000;fill-opacity:.25}
.k{fill:#10151c}.k2{fill:#5a6472}.k3{fill:#6c7480}.r{fill:#4a4f57}.r2{fill:#5c626b}.w{fill:#e9e5dc}
.a{fill:#4f82ee}.a2{fill:#7fa3f0}.a3{fill:#3b5482}.s{fill:#c9533c}.s2{fill:#dd7a5c}.s3{fill:#7a4a3e}
.o{fill:#e2a940}.o2{fill:#e9cd86}.v{fill:#6f9a78}.v2{fill:#56785c}
.lk{stroke:#e8e4da}.lk2{stroke:#a9b0ba}.lk3{stroke:#6c7480}.la{stroke:#6f9cf2}.la2{stroke:#7fa3f0}.la3{stroke:#3b5482}
.ls{stroke:#e0745f}.ls2{stroke:#dd7a5c}.lo{stroke:#e2a940}.lo2{stroke:#e9cd86}.lw{stroke:#e9e5dc}.lr{stroke:#4a4f57}.lb{stroke:#1d2633}.lv{stroke:#6f9a78}}
</style>"""

GRID = '<path class="g" d="M40 0v180M100 0v180M160 0v180M220 0v180M280 0v180M0 30h320M0 90h320M0 150h320"/>'


def svg(body, shadow=(160, 156, 74)):
    sh = f'<ellipse class="sh" cx="{shadow[0]}" cy="{shadow[1]}" rx="{shadow[2]}" ry="6"/>' if shadow else ""
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 180">{STYLE}'
            f'<rect class="b" width="320" height="180"/>{GRID}{sh}{body}</svg>\n')


ART = {}

# 1 Earthquakes: a seismogram spiking over a cracked ground block, with rings from the focus.
ART["earthquakes"] = svg("""
<circle class="ln la2" stroke-width="2.4" cx="160" cy="124" r="26" opacity=".7"/><circle class="ln la3" stroke-width="2.4" cx="160" cy="124" r="44"/><circle class="ln la3" stroke-width="2" cx="160" cy="124" r="62" opacity=".6"/>
<path class="r" d="M36 122h114l8 10-10 10 8 14H36z"/><path class="r2" d="M36 146h120l2 10H36z"/>
<path class="r" d="M164 112h120v44H168l-8-14 10-10z"/><path class="r2" d="M162 146h122v10H168z"/>
<path class="ln ls" stroke-width="3.2" d="M24 70h70l6-6 6 10 6-16 8 26 8-52 10 72 9-60 8 42 7-28 7 16 6-8 6 4h88"/>
""", shadow=None)

# 2 Fires: a three-tone flame over crossed logs.
ART["fires"] = svg("""
<rect class="k2" x="112" y="134" width="96" height="14" rx="7" transform="rotate(-10 160 141)"/>
<rect class="k3" x="112" y="134" width="96" height="14" rx="7" transform="rotate(10 160 141)"/>
<path class="s" d="M160 22c8 22 34 34 34 70 0 26-16 44-34 44s-34-18-34-44c0-20 10-30 16-42 2 12 8 18 12 18-4-18 2-34 6-46z"/>
<path class="s2" d="M161 58c6 16 22 24 22 46 0 18-10 30-23 30s-23-12-23-30c0-12 6-20 10-26 2 8 6 12 9 12-2-12 2-24 5-32z"/>
<path class="o2" d="M160 92c4 9 12 14 12 26 0 9-5 16-12 16s-12-7-12-16c0-8 4-12 6-15 1 4 3 6 5 6 0-7 0-12 1-17z"/>
""")

# 3 Tornadoes: a dark cloud base and a funnel of stacked swirls, with debris.
ART["tornadoes"] = svg("""
<g class="k3"><rect x="96" y="40" width="128" height="9" rx="4.5"/><rect x="106" y="54" width="104" height="9" rx="4.5"/><rect x="118" y="68" width="82" height="9" rx="4.5"/>
<rect x="128" y="82" width="62" height="9" rx="4.5"/><rect x="138" y="96" width="44" height="9" rx="4.5"/><rect x="146" y="110" width="30" height="9" rx="4.5"/>
<rect x="152" y="124" width="20" height="9" rx="4.5"/><rect x="156" y="138" width="12" height="9" rx="4.5"/></g>
<path class="k2" d="M60 40c0-12 14-20 26-16 6-12 24-16 36-8 10-10 32-10 42 2 12-8 32-6 40 6 14-4 30 4 30 16z"/>
<g class="k2"><rect x="120" y="140" width="6" height="4" rx="1" transform="rotate(30 123 142)"/><rect x="196" y="128" width="8" height="4" rx="1" transform="rotate(-25 200 130)"/><circle cx="186" cy="146" r="2.5"/><circle cx="132" cy="124" r="2"/></g>
""", shadow=(162, 154, 40))

# 4 Temperature anomalies: a globe of warm and cool cells beside a thermometer.
ART["temperature"] = svg("""
<circle class="a3" cx="134" cy="92" r="56"/>
<path class="s3" d="M86 70h30v22H86zM146 48h28v22h-28zM116 92h30v22h-30zM174 92h16v22h-16z"/><path class="s2" d="M116 70h30v22h-30zM146 92h28v22h-28zM146 114h24v18h-24z"/><path class="a2" d="M96 114h20v18H96zM116 48h30v22h-30z"/>
<circle class="ln lw" stroke-width="3" cx="134" cy="92" r="56"/>
<rect class="w" x="206" y="30" width="26" height="104" rx="13"/><circle class="w" cx="219" cy="136" r="20"/>
<rect class="s" x="213" y="62" width="12" height="74" rx="6"/><circle class="s" cx="219" cy="136" r="13"/>
<path class="ln lk3" stroke-width="2" d="M236 50h6M236 66h6M236 82h6M236 98h6"/>
""", shadow=(176, 158, 80))

# 5 Sea surface temperature: a striped ocean buoy riding layered waves.
ART["sea-temperature"] = svg("""
<circle class="o2" cx="250" cy="44" r="16"/>
<path class="k2" d="M158 30h4v40h-4z"/><circle class="o" cx="160" cy="28" r="6"/>
<path class="s" d="M134 70h52l8 52h-68z"/><path class="w" d="M131 88h58l2 14h-62z"/>
<path class="a2" d="M0 116c20-10 40-10 60 0s40 10 60 0 40-10 60 0 40 10 60 0 40-10 80 0v64H0z"/>
<path class="a" d="M0 136c20-8 40-8 60 0s40 8 60 0 40-8 60 0 40 8 60 0 40-8 80 0v44H0z"/>
""", shadow=None)

# 6 Rain and snow: a cloud dropping rain on one side and snowflakes on the other.
ART["precipitation"] = svg("""
<path class="w" d="M92 92c-14 0-24-10-24-22s10-22 24-22c4-16 18-26 34-26 14 0 26 8 32 20 4-2 9-3 14-3 18 0 32 13 34 30 14 1 24 11 24 23 0 0 0 0 0 0H92z"/>
<path class="ln lr" stroke-width="2" d="M92 92h136"/>
<g class="ln la" stroke-width="4"><path d="M100 108l-6 14M124 104l-6 14M112 128l-6 14M136 126l-6 14M148 106l-6 14"/></g>
<g class="ln la2" stroke-width="2.4"><path d="M184 104v20M174 114h20M177 107l14 14M191 107l-14 14"/><path d="M212 124v16M204 132h16M206 126l12 12M218 126l-12 12"/><path d="M200 146v10M195 151h10"/></g>
""", shadow=None)

# 7 Wind turbines: a near turbine and a far one on green hills.
ART["wind-turbines"] = svg("""
<path class="v2" d="M0 146c50-20 110-22 170-8s110 10 150-6v48H0z"/>
<path class="r" d="M238 74l3-0 3 66h-9z"/><circle class="r" cx="241" cy="74" r="4"/>
<g class="r"><path d="M241 74l-3-34 5 0z"/><path d="M241 74l30 16-3 4z"/><path d="M241 74l-28 18-2-4z"/></g>
<path class="w" d="M146 60h8l4 96h-16z"/><path class="r" d="M150 60h4l4 96h-8z"/><rect class="w" x="140" y="52" width="24" height="12" rx="5"/>
<g class="w"><path d="M150 58l-6-50h8z"/><path d="M150 58l46 26-4 6z"/><path d="M150 58l-44 28-3-6z"/></g><circle class="k3" cx="150" cy="58" r="5"/>
""", shadow=None)

# 8 Nuclear reactors: a cooling tower breathing steam beside a reactor dome.
ART["nuclear"] = svg("""
<path class="w" d="M120 34c-14-6-6-24 10-20 4-12 24-12 30-2 12-8 30 0 26 14 14 2 14 20 0 22-6 10-24 10-30 2-10 8-28 4-36-16z"/>
<path class="r" d="M118 150c8-30 10-62 0-96h76c-10 34-8 66 0 96z"/><path class="r2" d="M168 54h26c-10 34-8 66 0 96h-26c-4-32-4-64 0-96z"/>
<rect class="k3" x="114" y="50" width="84" height="8" rx="4"/>
<path class="w" d="M206 150v-26c0-18 12-30 26-30s26 12 26 30v26z"/><path class="ln lr" stroke-width="3" d="M206 128h52"/>
""", shadow=(180, 154, 84))

# 9 Solar farms: rows of tilted panels under the sun.
ART["solar"] = svg("""
<circle class="o" cx="242" cy="44" r="20"/><g class="ln lo" stroke-width="4"><path d="M242 12v6M242 70v6M210 44h6M268 44h6M220 22l4 4M260 62l4 4M264 22l-4 4M224 62l-4 4"/></g>
<g><path class="a" d="M58 96h70l-12 22H46z"/><path class="a" d="M134 96h70l-12 22h-70z"/><path class="a" d="M36 126h80l-14 26H22z"/><path class="a" d="M124 126h80l-14 26h-80z"/><path class="a" d="M212 126h80l-14 26h-80z"/></g>
<g class="ln la3" stroke-width="1.6"><path d="M52 107h70M128 107h70M29 139h80M117 139h80M205 139h80M81 96l-12 22M157 96l-12 22M62 126l-14 26M150 126l-14 26M238 126l-14 26"/></g>
<path class="k3" d="M82 118h4v8h-4zM158 118h4v8h-4zM66 152h4v4h-4zM154 152h4v4h-4zM242 152h4v4h-4z"/>
""", shadow=None)

# 10 Mines: an open pit's terraces behind a loaded mine cart.
ART["mines"] = svg("""
<path class="r" d="M30 70h260l-30 30H60z"/><path class="r2" d="M60 100h200l-26 22H86z"/><path class="o" d="M86 122h148l-22 18h-104z" opacity=".55"/>
<path class="ln lw" stroke-width="2" d="M44 76h232M70 104h180"/>
<path class="o" d="M126 98c4-10 14-12 20-6 6-8 18-6 22 2 8-4 18 2 18 10z"/>
<path class="k2" d="M118 104h84l-10 38h-64z"/><path class="ln lk3" stroke-width="3" d="M124 116h72"/>
<circle class="k" cx="138" cy="146" r="9"/><circle class="k" cx="182" cy="146" r="9"/><circle class="k3" cx="138" cy="146" r="3"/><circle class="k3" cx="182" cy="146" r="3"/>
""", shadow=(160, 156, 60))

# 11 World cities: a skyline with lit windows.
ART["cities"] = svg("""
<path class="a3" d="M44 156V96h22v-12h16v72zM230 156V88h18v-16h14v84z"/>
<rect class="k3" x="76" y="78" width="30" height="78"/><rect class="a2" x="110" y="56" width="34" height="100"/><rect class="k2" x="148" y="30" width="30" height="126"/>
<path class="k2" d="M160 30v-12h6v12z"/><rect class="k3" x="182" y="70" width="30" height="86"/><rect class="a2" x="214" y="98" width="24" height="58"/>
<g class="o2"><path d="M82 88h6v6h-6zM94 88h6v6h-6zM82 104h6v6h-6zM94 120h6v6h-6zM116 66h6v6h-6zM130 66h6v6h-6zM116 84h6v6h-6zM130 102h6v6h-6zM116 120h6v6h-6zM154 42h6v6h-6zM166 42h6v6h-6zM154 62h6v6h-6zM166 82h6v6h-6zM154 102h6v6h-6zM166 122h6v6h-6zM188 82h6v6h-6zM200 98h6v6h-6zM188 114h6v6h-6zM220 108h6v6h-6zM220 124h6v6h-6z"/></g>
""", shadow=(160, 157, 120))

# 12 Life expectancy: an hourglass with the sand running.
ART["life-expectancy"] = svg("""
<rect class="k2" x="116" y="20" width="88" height="12" rx="5"/><rect class="k2" x="116" y="140" width="88" height="12" rx="5"/>
<rect class="k3" x="122" y="32" width="6" height="108"/><rect class="k3" x="192" y="32" width="6" height="108"/>
<path class="w" d="M134 32h52c0 30-20 40-20 54s20 24 20 54h-52c0-30 20-40 20-54s-20-24-20-54z"/>
<path class="o" d="M144 54h32c-4 12-12 18-16 26-4-8-12-14-16-26zM159 86h2v32h-2zM136 140c2-16 12-24 24-26 12 2 22 10 24 26z"/>
""", shadow=(160, 156, 60))

# 13 Where people live: a small crowd of people.
ART["population"] = svg("""
<g class="a3"><circle cx="102" cy="72" r="12"/><path d="M82 128c0-24 8-38 20-38s20 14 20 38z"/><circle cx="218" cy="72" r="12"/><path d="M198 128c0-24 8-38 20-38s20 14 20 38z"/></g>
<circle class="o" cx="132" cy="80" r="14"/><path class="o" d="M110 146c0-28 10-44 22-44s22 16 22 44z"/>
<circle class="s2" cx="188" cy="80" r="14"/><path class="s2" d="M166 146c0-28 10-44 22-44s22 16 22 44z"/>
<circle class="a" cx="160" cy="70" r="16"/><path class="a" d="M134 152c0-32 12-50 26-50s26 18 26 50z"/>
""", shadow=(160, 156, 82))

# 14 Airports: an airliner climbing on a dotted contrail.
ART["airports"] = svg("""
<path class="ln la3" stroke-width="3" stroke-dasharray="2 9" d="M24 160c40-14 70-32 96-56"/>
<g transform="rotate(-35 170 84)"><path class="w" d="M96 84c0-7 6-10 14-10h108c14 0 26 5 26 10s-12 10-26 10H110c-8 0-14-3-14-10z"/>
<path class="a2" d="M150 80l-26-50h14l42 50zM150 88l-26 50h14l42-50z"/><path class="a2" d="M104 80l-10-22h9l18 22zM104 88l-10 22h9l18-22z"/>
<path class="a" d="M226 76h10c6 2 8 5 8 8s-2 6-8 8h-10z"/></g>
""", shadow=(170, 156, 46))

# 15 Seaports: a container ship with stacked boxes.
ART["ports"] = svg("""
<rect class="w" x="212" y="54" width="26" height="40" rx="2"/><path class="ln la2" stroke-width="3" d="M216 64h18M216 74h18"/><rect class="k3" x="220" y="40" width="6" height="14"/>
<g><rect class="a" x="86" y="78" width="28" height="16"/><rect class="s" x="114" y="78" width="28" height="16"/><rect class="o" x="142" y="78" width="28" height="16"/><rect class="v" x="170" y="78" width="28" height="16"/>
<rect class="o" x="100" y="62" width="28" height="16"/><rect class="a2" x="128" y="62" width="28" height="16"/><rect class="s2" x="156" y="62" width="28" height="16"/></g>
<path class="k2" d="M58 94h204l-18 36H82z"/><path class="s" d="M70 116h180l-6 14H82z"/>
<path class="a2" d="M0 132c20-6 40-6 60 0s40 6 60 0 40-6 60 0 40 6 60 0 40-6 80 0v48H0z"/>
""", shadow=None)

# 16 Railways: a bullet train with Mount Fuji behind.
ART["railways"] = svg("""
<path class="a3" d="M150 112l50-62c6-6 12-6 18 0l50 62z"/><path class="w" d="M188 64l12-14c6-6 12-6 18 0l12 14-8 6-7-5-7 5-8-5z"/>
<path class="w" d="M30 96h170c40 0 70 14 90 34H30z"/><path class="a" d="M30 116h250c4 4 7 8 10 14H30z"/>
<g class="k2"><path d="M232 102h22c8 2 14 6 18 10h-40z"/><rect x="44" y="102" width="18" height="10" rx="3"/><rect x="72" y="102" width="18" height="10" rx="3"/><rect x="100" y="102" width="18" height="10" rx="3"/><rect x="128" y="102" width="18" height="10" rx="3"/><rect x="156" y="102" width="18" height="10" rx="3"/><rect x="184" y="102" width="18" height="10" rx="3"/></g>
<rect class="k3" x="20" y="134" width="290" height="5" rx="2"/><path class="ln lk3" stroke-width="3" d="M40 146h10M80 146h10M120 146h10M160 146h10M200 146h10M240 146h10M280 146h10"/>
""", shadow=None)

# 17 World Heritage: a classical temple front on its steps.
ART["heritage"] = svg("""
<circle class="a3" cx="160" cy="92" r="66" opacity=".8"/><path class="w" d="M96 64l64-34 64 34z"/><path class="r" d="M114 60l46-24 46 24z"/>
<rect class="r2" x="94" y="64" width="132" height="10"/>
<g class="w"><rect x="104" y="78" width="14" height="56"/><rect x="130" y="78" width="14" height="56"/><rect x="156" y="78" width="14" height="56" x2="0"/><rect x="182" y="78" width="14" height="56"/><rect x="206" y="78" width="14" height="56"/></g>
<g class="r"><rect x="110" y="78" width="3" height="56"/><rect x="136" y="78" width="3" height="56"/><rect x="162" y="78" width="3" height="56"/><rect x="188" y="78" width="3" height="56"/><rect x="212" y="78" width="3" height="56"/></g>
<rect class="r2" x="92" y="134" width="136" height="8"/><rect class="r" x="82" y="142" width="156" height="10"/>
""", shadow=(160, 156, 86))

# 18 Ancient places: an amphora with painted bands.
ART["ancient-places"] = svg("""
<path class="s2" d="M146 24h28v12c22 12 32 34 32 58 0 26-20 46-46 58-26-12-46-32-46-58 0-24 10-46 32-58z"/>
<path class="ln ls2" stroke-width="6" d="M146 38c-18 0-22 14-16 28M174 38c18 0 22 14 16 28"/>
<rect class="s" x="142" y="20" width="36" height="8" rx="3"/>
<path class="k2" d="M116 84h88v8h-88zM120 110h80v5h-80z"/><path class="ln lk2" stroke-width="2.4" d="M126 100l8-6 8 6 8-6 8 6 8-6 8 6 8-6 8 6"/>
<path class="s3" d="M228 146l14-8 10 10-12 8zM78 150l10-10 12 6-6 8z"/>
""", shadow=(160, 158, 54))

# 19 Roman roads: a paved road running to the horizon past a milestone and cypresses.
ART["roman-roads"] = svg("""
<path class="v2" d="M0 70h320v110H0z" opacity=".55"/>
<path class="r" d="M150 70h20l70 110H80z"/><path class="ln lr" stroke-width="1" d="M0 0"/>
<g class="ln lb" stroke-width="1.6"><path d="M144 80h32M136 92h48M124 108h72M110 128h100M94 152h132"/><path d="M160 70v10M156 92l-2 16M164 108l2 20M150 128l-4 24M172 128l4 24M120 152l-6 28M200 152l6 28M160 80l0 12"/></g>
<g class="v"><path d="M58 112c-8-20-6-50 6-66 12 16 14 46 6 66z"/><path d="M262 112c-8-20-6-50 6-66 12 16 14 46 6 66z"/><path d="M96 84c-5-12-4-30 4-40 8 10 9 28 4 40z"/><path d="M216 84c-5-12-4-30 4-40 8 10 9 28 4 40z"/></g>
<rect class="w" x="238" y="112" width="20" height="44" rx="3"/><rect class="r2" x="234" y="108" width="28" height="7" rx="2"/><path class="ln lk3" stroke-width="2" d="M243 126h10M243 134h10"/>
""", shadow=None)

# 20 County seats: a Chinese city gate tower on its wall.
ART["county-seats"] = svg("""
<rect class="r2" x="40" y="100" width="240" height="54"/><path class="ln lr" stroke-width="2" d="M40 118h240M40 136h240"/>
<path class="k2" d="M146 154v-24c0-10 6-16 14-16s14 6 14 16v24z"/>
<path class="k3" d="M40 94h240v8H40z"/><g class="k3"><rect x="44" y="86" width="10" height="8"/><rect x="66" y="86" width="10" height="8"/><rect x="88" y="86" width="10" height="8"/><rect x="222" y="86" width="10" height="8"/><rect x="244" y="86" width="10" height="8"/><rect x="266" y="86" width="10" height="8"/></g>
<g class="s"><rect x="118" y="64" width="8" height="30"/><rect x="194" y="64" width="8" height="30"/><rect x="144" y="64" width="6" height="30"/><rect x="170" y="64" width="6" height="30"/></g>
<path class="k2" d="M100 66c18-2 32-10 40-18h40c8 8 22 16 40 18z"/><path class="k2" d="M118 44c12-2 22-8 28-14h28c6 6 16 12 28 14z"/><rect class="s" x="130" y="44" width="60" height="6"/>
""", shadow=None)

# 21 Buddhist temples: a five-storey pagoda.
ART["temples"] = svg("""
<path class="s" d="M146 50h28v100h-28z"/>
<g class="k2"><path d="M118 140c14 0 24-4 30-10h24c6 6 16 10 30 10z"/><path d="M122 118c12 0 22-4 28-10h20c6 6 16 10 28 10z"/><path d="M126 96c12 0 20-4 26-10h16c6 6 14 10 26 10z"/><path d="M130 74c10 0 18-4 24-10h12c6 6 14 10 24 10z"/><path d="M134 54c10 0 16-4 22-10h8c6 6 12 10 22 10z"/></g>
<path class="o" d="M159 18h2v26h-2z"/><circle class="o" cx="160" cy="24" r="3"/><circle class="o" cx="160" cy="32" r="3"/>
<path class="o2" d="M156 120h8v10h-8zM156 98h8v8h-8zM156 76h8v8h-8z"/><rect class="r2" x="128" y="148" width="64" height="8" rx="2"/>
""", shadow=(160, 157, 56))

# 22 Castles: towers, battlements, a gate and a pennant.
ART["castles"] = svg("""
<rect class="w" x="104" y="70" width="112" height="84"/><path class="r" d="M104 70h112v10H104z"/>
<g class="w"><path d="M104 62h12v8h-12zM128 62h12v8h-12zM152 62h16v8h-16zM180 62h12v8h-12zM204 62h12v8h-12z"/></g>
<rect class="w" x="82" y="50" width="34" height="104"/><rect class="w" x="204" y="50" width="34" height="104"/><rect class="r" x="104" y="50" width="12" height="104"/><rect class="r" x="226" y="50" width="12" height="104"/>
<path class="k2" d="M78 50l21-28 21 28zM200 50l21-28 21 28z"/><path class="k" d="M146 154v-30c0-8 6-14 14-14s14 6 14 14v30z"/>
<path class="k3" d="M95 76h8v14h-8zM217 76h8v14h-8zM140 86h6v10h-6zM174 86h6v10h-6z"/>
<path class="k2" d="M98 22h2V6h-2z"/><path class="s" d="M100 6h18l-6 5 6 5h-18z"/>
""", shadow=(160, 157, 92))

# 23 Species sightings: a butterfly on a dotted flight path.
ART["species"] = svg("""
<path class="ln lk3" stroke-width="2.4" stroke-dasharray="1 8" d="M30 150c30-10 40-40 70-44s40 14 60 0"/>
<path class="a" d="M158 88c-10-26-34-44-58-40-12 2-12 18-4 30 8 12 30 18 62 14z"/><path class="a2" d="M158 92c-24 0-46 6-50 22-2 12 12 18 24 12 12-6 22-18 26-34z"/>
<path class="a" d="M162 88c10-26 34-44 58-40 12 2 12 18 4 30-8 12-30 18-62 14z"/><path class="a2" d="M162 92c24 0 46 6 50 22 2 12-12 18-24 12-12-6-22-18-26-34z"/>
<circle class="o2" cx="116" cy="64" r="6"/><circle class="o2" cx="204" cy="64" r="6"/><circle class="o" cx="126" cy="112" r="4"/><circle class="o" cx="194" cy="112" r="4"/>
<rect class="k" x="156" y="70" width="8" height="52" rx="4"/><path class="ln lk" stroke-width="2" d="M158 72c-4-10-10-14-16-16M162 72c4-10 10-14 16-16"/>
""", shadow=(160, 156, 50))

# 24 White storks: a stork in flight above its tracking fixes.
ART["storks"] = svg("""
<g class="ln la2" stroke-width="2" stroke-dasharray="3 6"><path d="M20 166c50-6 90-20 140-40s90-48 140-70"/></g>
<g class="a"><circle cx="20" cy="166" r="3.5"/><circle cx="74" cy="158" r="3.5"/><circle cx="252" cy="78" r="3.5"/><circle cx="300" cy="56" r="3.5"/></g>
<path class="ln ls" stroke-width="3" d="M156 112l-6 34M164 112l6 34"/>
<path class="w" d="M152 108l8 18 8-18z"/>
<path class="k" d="M66 84c30 2 60 8 90 10v8c-30-2-62-6-88-8zM254 84c-30 2-60 8-90 10v8c30-2 62-6 88-8z"/>
<path class="w" d="M156 76c-26-8-60-12-94-6l4 14c30 2 60 8 90 10zM164 76c26-8 60-12 94-6l-4 14c-30 2-60 8-90 10z"/>
<ellipse class="w" cx="160" cy="88" rx="9" ry="26"/>
<path class="ln lw" stroke-width="7" d="M160 66V46"/><circle class="w" cx="160" cy="44" r="6.5"/>
<path class="ln ls" stroke-width="3.4" d="M160 38V16"/>
""", shadow=None)

# 25 Street trees: a round street tree in its pit beside a kerb.
ART["street-trees"] = svg("""
<rect class="r" x="20" y="146" width="280" height="10" rx="2"/><rect class="r2" x="120" y="144" width="80" height="6" rx="2"/>
<path class="k2" d="M154 146l2-50h8l2 50z"/><path class="ln lk2" stroke-width="5" d="M160 112l-16-14M161 104l18-16"/>
<circle class="v2" cx="132" cy="78" r="30"/><circle class="v2" cx="190" cy="80" r="28"/><circle class="v" cx="160" cy="58" r="38"/><circle class="v" cx="128" cy="92" r="20"/><circle class="v" cx="194" cy="94" r="20"/>
<circle class="v2" cx="148" cy="48" r="10" opacity=".7"/>
""", shadow=None)

# 26 Forest loss: a cut stump with growth rings and a felled log.
ART["forest-loss"] = svg("""
<path class="v2" d="M52 40l18 34H34zM52 60l22 40H30zM272 46l15 28h-30zM272 64l19 34h-38z" opacity=".75"/><path class="k3" d="M50 100h4v12h-4zM270 98h4v12h-4z"/>
<path class="s2" d="M112 96h64v52c-10 6-54 6-64 0z"/><path class="ln ls" stroke-width="3" d="M124 110v30M150 112v34M166 108v32"/>
<ellipse class="o2" cx="144" cy="96" rx="32" ry="12"/><ellipse class="ln lo" stroke-width="2" cx="144" cy="96" rx="22" ry="8"/><ellipse class="ln lo" stroke-width="2" cx="144" cy="96" rx="12" ry="4"/>
<rect class="s2" x="186" y="128" width="84" height="22" rx="11"/><ellipse class="o2" cx="268" cy="139" rx="8" ry="11"/><ellipse class="ln lo" stroke-width="1.6" cx="268" cy="139" rx="4" ry="6"/>
""", shadow=(180, 154, 96))

# 27 Meteorites: a rock with a fiery trail over a small crater.
ART["meteorites"] = svg("""
<path class="o2" d="M40 20l130 74-14 18z" opacity=".8"/><path class="o" d="M70 30l104 64-10 12z"/><path class="s2" d="M110 52l66 42-8 10z"/>
<path class="k2" d="M162 92c8-12 26-14 36-4 10 8 10 24 0 32-10 10-28 8-36-2-6-8-6-18 0-26z"/><path class="k3" d="M174 96c4-4 10-4 14 0-2 6-10 8-14 0zM186 110c3-2 6 0 6 3-3 2-6 1-6-3z"/>
<path class="r" d="M196 150c10-12 50-12 60 0z"/><path class="r2" d="M208 150c6-6 30-6 36 0z"/>
""", shadow=(226, 152, 40))

# 28 Eclipses: the Moon over the Sun's corona, with the path of totality behind.
ART["eclipse"] = svg("""
<path class="k" d="M0 132L320 40v42L0 174z" opacity=".07"/>
<circle class="o2" cx="160" cy="88" r="52" opacity=".55"/><circle class="o2" cx="160" cy="88" r="42"/>
<g class="ln lo" stroke-width="3"><path d="M160 22v14M160 140v14M94 88h14M212 88h14M114 42l10 10M196 124l10 10M206 42l-10 10M124 124l-10 10"/></g>
<circle class="k" cx="160" cy="88" r="34"/><circle class="w" cx="188" cy="66" r="5"/>
""", shadow=None)

# 29 Earth at night: a satellite above the dark limb of the Earth and its city lights.
ART["night-lights"] = svg("""
<path class="k2" d="M0 150c80-30 240-30 320 0v30H0z"/>
<g class="o2"><circle cx="60" cy="152" r="2"/><circle cx="70" cy="149" r="1.5"/><circle cx="118" cy="142" r="2.4"/><circle cx="126" cy="146" r="1.5"/><circle cx="174" cy="140" r="2"/><circle cx="210" cy="144" r="2.6"/><circle cx="218" cy="141" r="1.4"/><circle cx="260" cy="150" r="2"/><circle cx="150" cy="150" r="1.4"/></g>
<rect class="a" x="70" y="58" width="60" height="30" rx="2"/><rect class="a" x="190" y="58" width="60" height="30" rx="2"/>
<path class="ln la3" stroke-width="1.6" d="M90 58v30M110 58v30M70 73h60M210 58v30M230 58v30M190 73h60"/>
<rect class="k3" x="130" y="70" width="60" height="6"/>
<rect class="o" x="144" y="52" width="32" height="42" rx="4"/><path class="ln lo2" stroke-width="2" d="M148 62h24M148 72h24M148 82h24"/>
<path class="k3" d="M156 94h8l4 10h-16z"/>
""", shadow=None)

# 30 Chain stores: a takeaway coffee cup with steam and a map pin.
ART["chain-stores"] = svg("""
<path class="ln lk3" stroke-width="3" d="M146 30c-6-8 6-12 0-20M164 32c-6-8 6-12 0-20"/>
<path class="w" d="M122 52h76l-10 100h-56z"/><rect class="k2" x="116" y="42" width="88" height="12" rx="4"/>
<path class="v" d="M126 82h68l-4 40h-60z"/><circle class="w" cx="160" cy="102" r="12"/>
<path class="s" d="M232 54c-10 0-18 8-18 18 0 14 18 30 18 30s18-16 18-30c0-10-8-18-18-18z"/><circle class="w" cx="232" cy="72" r="6"/>
""", shadow=(166, 156, 68))

# 31 Anything on OpenStreetMap: a magnifier over a map with pins.
ART["openstreetmap"] = svg("""
<rect class="w" x="54" y="34" width="150" height="112" rx="6"/>
<path class="v2" d="M54 104c30-6 48 8 70 2s34-20 80-12v46c0 3-3 6-6 6H60c-3 0-6-3-6-6z"/>
<g class="ln la2" stroke-width="3"><path d="M54 70h150M120 34v112"/></g><path class="ln lr" stroke-width="2" d="M54 96c40-10 70 20 150-14"/>
<g class="s"><circle cx="92" cy="58" r="5"/><circle cx="168" cy="54" r="5"/><circle cx="146" cy="120" r="5"/></g>
<circle class="a3" cx="196" cy="94" r="34" opacity=".75"/><circle class="ln lk2" stroke-width="9" cx="196" cy="94" r="34"/>
<path class="ln lk2" stroke-width="13" d="M222 120l30 30"/><circle class="s" cx="188" cy="90" r="7"/>
""", shadow=(160, 156, 84))

# 32 City extracts: a bicycle in front of a dashed selection box.
ART["city-extract"] = svg("""
<rect class="ln la" stroke-width="2.4" stroke-dasharray="7 6" x="44" y="22" width="232" height="104" rx="4"/>
<g class="la"><circle class="a" cx="44" cy="22" r="5"/><circle class="a" cx="276" cy="22" r="5"/><circle class="a" cx="44" cy="126" r="5"/><circle class="a" cx="276" cy="126" r="5"/></g>
<circle class="ln lk2" stroke-width="7" cx="112" cy="118" r="30"/><circle class="ln lk2" stroke-width="7" cx="212" cy="118" r="30"/>
<path class="ln ls" stroke-width="7" d="M112 118l34-46h52l14 46M146 72l20 46 32-46M166 118h-54"/>
<path class="ln lk" stroke-width="7" d="M136 62h22M190 58l8 14 16-4"/><circle class="k" cx="166" cy="118" r="6"/>
""", shadow=None)

# 33 Whole countries: a tri-fold paper map with a route and a pin.
ART["country-extract"] = svg("""
<path class="w" d="M64 40l64-14v118l-64 14z"/><path class="r" d="M128 26l64 14v118l-64-14z"/><path class="w" d="M192 40l64-14v118l-64 14z"/>
<path class="v2" d="M64 92c20-6 40 4 64-6v40l-64 14zM128 86c20 4 40 14 64 8v40l-64-12zM192 94c20-8 40-4 64-14v40l-64 14z" opacity=".8"/>
<path class="ln la2" stroke-width="4" d="M64 70l64-12 64 10 64-18"/>
<path class="ln ls" stroke-width="3" stroke-dasharray="5 6" d="M86 128c20-14 40-30 66-20s46 0 70-34"/>
<path class="s" d="M222 44c-9 0-16 7-16 16 0 12 16 26 16 26s16-14 16-26c0-9-7-16-16-16z"/><circle class="w" cx="222" cy="60" r="5"/>
""", shadow=(160, 160, 100))

# 34 Hawker centres: a bowl of noodles with chopsticks and steam.
ART["hawker"] = svg("""
<path class="ln lk3" stroke-width="3" d="M132 50c-6-8 6-12 0-20M160 46c-6-8 6-12 0-20M188 50c-6-8 6-12 0-20"/>
<path class="ln ls2" stroke-width="5" d="M196 30l-40 64M216 34l-48 62"/>
<ellipse class="o2" cx="160" cy="86" rx="70" ry="14"/><path class="ln lo" stroke-width="3" d="M108 86c8-6 14 6 22 0s14 6 22 0 14 6 22 0 14 6 22 0"/>
<circle class="v" cx="132" cy="82" r="5"/><circle class="s3" cx="184" cy="82" r="6"/><circle class="w" cx="184" cy="82" r="2.5"/>
<path class="a" d="M90 88h140c-4 34-30 58-70 58s-66-24-70-58z"/><path class="ln lw" stroke-width="3" d="M100 104h120M110 120h100" opacity=".55"/>
<rect class="a2" x="138" y="144" width="44" height="8" rx="3"/>
""", shadow=(160, 156, 64))

OUT.mkdir(parents=True, exist_ok=True)
for slug, text in ART.items():
    text = "\n".join(line.strip() for line in text.splitlines() if line.strip())
    (OUT / f"{slug}.svg").write_text(text + "\n", encoding="utf-8")
print(len(ART), "illustrations")
