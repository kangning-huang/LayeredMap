"""Keyframed map camera paths for the v3.7 film (Google Earth Studio / After Effects style): one
continuous camera per shot, sampled at exact frame (and motion-blur sub-frame) times, rendered
offline by the app (crates/lm-ui/examples/film_render.rs).

Views are the app's own camera: a Web Mercator centre (metres) and a resolution (metres per
logical point; zoom z is res_of(z), 256-px tiles), for the map area of a 1409 x 845 pt window
(the map is the window less the 52 pt toolbar).

Moves (see Chain):
  'fly'  - van Wijk & Nuij (2003) optimal smooth zoom/pan (what Mapbox's flyTo uses), rho 1.42:
           the path parameter s is the perceived distance, traversed with an ease.
  'zoom' - a local move: the two views are related by a similarity (scale + shift), interpolated
           about its fixed point with log-resolution linear in the eased progress, so zoom changes
           at a perceptually constant rate and nothing swoops.
and a constant slow drift over each whole shot, so the map never fully stops.
"""
import math

R = 6378137.0
RES_Z0 = 2 * math.pi * R / 256.0
MAP_W, MAP_H = 1409.0, 793.0
RHO = 1.42


def merc(lon, lat):
    lat = max(-85.0511, min(85.0511, lat))
    return R * math.radians(lon), R * math.log(math.tan(math.pi / 4 + math.radians(lat) / 2))


def unmerc(x, y):
    return math.degrees(x / R), math.degrees(2 * math.atan(math.exp(y / R)) - math.pi / 2)


def res_of(z):
    return RES_Z0 / 2 ** z


def zoom_of(res):
    return math.log2(RES_Z0 / res)


class View:
    __slots__ = ('x', 'y', 'res')

    def __init__(self, x, y, res):
        self.x, self.y, self.res = x, y, res

    @staticmethod
    def ll(lon, lat, z):
        x, y = merc(lon, lat)
        return View(x, y, res_of(z))

    def lonlatz(self):
        lon, lat = unmerc(self.x, self.y)
        return lon, lat, zoom_of(self.res)

    def to_screen(self, mx, my):
        return (mx - self.x) / self.res + MAP_W / 2, (self.y - my) / self.res + MAP_H / 2

    def to_map(self, sx, sy):
        return self.x + (sx - MAP_W / 2) * self.res, self.y - (sy - MAP_H / 2) * self.res

    def __repr__(self):
        lon, lat, z = self.lonlatz()
        return f'View({lon:.4f}, {lat:.4f}, z{z:.3f})'


# ---------------------------------------------------------------- interpolants (p or s -> View)
def similarity(a, b):
    """Interpolant over p in [0,1]: log-res linear in p, about the similarity's fixed point."""
    la, lb = math.log(a.res), math.log(b.res)
    if abs(lb - la) < 1e-6:
        return (lambda p: View(a.x + (b.x - a.x) * p, a.y + (b.y - a.y) * p, a.res)), 0.0
    k = b.res / a.res
    # fixed point F: the map point at the same screen position in both views
    fx, fy = (b.x - k * a.x) / (1 - k), (b.y - k * a.y) / (1 - k)

    def at(p):
        r = math.exp(la + (lb - la) * p)
        s = r / a.res
        return View(fx + (a.x - fx) * s, fy + (a.y - fy) * s, r)
    return at, lb - la


def van_wijk(a, b, rho=RHO):
    """Interpolant over p in [0,1] (p = s/S, s the perceived distance), and S."""
    w0, w1 = a.res * MAP_W, b.res * MAP_W
    dx, dy = b.x - a.x, b.y - a.y
    u1 = math.hypot(dx, dy)
    if u1 < 1e-6 * max(w0, w1):
        S = abs(math.log(w1 / w0)) / rho
        k = 1 if w1 > w0 else -1

        def at0(p):
            return View(a.x, a.y, a.res * math.exp(k * rho * p * S))
        return at0, S
    r2 = rho * rho

    def b_(i):
        w = w1 if i else w0
        return (w1 * w1 - w0 * w0 + (-1 if i else 1) * r2 * r2 * u1 * u1) / (2 * w * r2 * u1)

    def r_(i):
        return -math.asinh(b_(i))          # = ln(-b + sqrt(b^2 + 1)), without the cancellation
    r0, r1 = r_(0), r_(1)
    S = (r1 - r0) / rho

    def at(p):
        s = p * S
        u = w0 / r2 * (math.cosh(r0) * math.tanh(rho * s + r0) - math.sinh(r0))
        w = w0 * math.cosh(r0) / math.cosh(rho * s + r0)
        f = u / u1
        return View(a.x + dx * f, a.y + dy * f, w / MAP_W)
    return at, S


# ---------------------------------------------------------------- paths
def smoothstep(u):
    u = min(1.0, max(0.0, u))
    return u * u * (3 - 2 * u)


def smootherstep(u):
    u = min(1.0, max(0.0, u))
    return u * u * u * (u * (6 * u - 15) + 10)


class Chain:
    """A shot's camera: a start view, then moves, each from the previous move's target to its own
    over [s, e] with an ease. 'zoom' moves (similarities) may overlap - the next one starting
    before the last has ended, as an animator overlaps actions - so the camera's speed never
    drops to zero between them; 'fly' moves (van Wijk) start once the moves before have ended.
    On top of everything a slow drift (dz zoom levels and dx, dy points per second, about the
    view's centre) runs for the whole shot, so the map never fully stops."""

    def __init__(self, t0, v0, drift=(0.012, 0.0, 0.0)):
        self.t0, self.v0, self.drift = t0, v0, drift
        self.moves = []
        self.last = v0

    def move(self, s, e, target, kind='zoom', ease=smoothstep):
        base = self.last
        if kind == 'fly':
            assert all(m['e'] <= s + 1e-9 for m in self.moves), 'a flight starts after the moves before it'
            at, _ = van_wijk(base, target)
            self.moves.append(dict(s=s, e=e, kind=kind, at=at, ease=ease, target=target))
        else:
            k = target.res / base.res
            if abs(k - 1) < 1e-9:
                tr = ('shift', target.x - base.x, target.y - base.y)
            else:
                tr = ('scale', k, (target.x - k * base.x) / (1 - k), (target.y - k * base.y) / (1 - k))
            self.moves.append(dict(s=s, e=e, kind=kind, tr=tr, ease=ease, target=target))
        self.last = target
        return self

    def at(self, t, drift=True):
        v = View(self.v0.x, self.v0.y, self.v0.res)
        for m in self.moves:
            p = m['ease']((t - m['s']) / (m['e'] - m['s']))
            if p <= 0:
                continue
            if m['kind'] == 'fly':
                f = m['at'](p)
                v = View(f.x, f.y, f.res)
                continue
            tr = m['tr']
            if tr[0] == 'shift':
                v = View(v.x + tr[1] * p, v.y + tr[2] * p, v.res)
            else:
                k = tr[1] ** p
                v = View(tr[2] + k * (v.x - tr[2]), tr[3] + k * (v.y - tr[3]), v.res * k)
        if drift:
            dt = max(0.0, t - self.t0)
            dz, dx, dy = self.drift
            v = View(v.x + dx * dt * v.res, v.y - dy * dt * v.res, v.res / 2 ** (dz * dt))
        return v


def wrapped(v):
    """The same view with its centre in the world copy at -180..180 (the app draws every copy)."""
    W = 2 * math.pi * R
    x = (v.x + W / 2) % W - W / 2
    return View(x, v.y, v.res)


# ---------------------------------------------------------------- measurement
_GRID = [(MAP_W * (i + 0.5) / 8, MAP_H * (j + 0.5) / 5) for i in range(8) for j in range(5)]


def screen_motion(a, b):
    """Mean displacement (points) of the map content between views a and b, over a screen grid."""
    tot = 0.0
    for sx, sy in _GRID:
        mx, my = a.to_map(sx, sy)
        bx, by = b.to_screen(mx, my)
        tot += math.hypot(bx - sx, by - sy)
    return tot / len(_GRID)
