#!/usr/bin/env python3
"""A tiny stand-in for the Bluesky XRPC API, for recording the README demo.

Everything here is fictional: handles live under the reserved `.example`
TLD, images are generated, and writes are acknowledged but not stored
beyond the process. Python standard library only.

    python3 demo/mock_server.py [port]      # default 47821
"""

import json
import math
import struct
import sys
import zlib
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 47821
BASE = f"http://127.0.0.1:{PORT}"
NOW = datetime.now(timezone.utc)


def ago(minutes):
    return (NOW - timedelta(minutes=minutes)).isoformat().replace("+00:00", "Z")


# --- people ------------------------------------------------------------------

PEOPLE = {
    "demo": ("did:plc:demo", "demo.example", "Demo Account"),
    "alt": ("did:plc:alt", "alt.example", "Demo Alt"),
    "wren": ("did:plc:wren", "wren.example", "Wren Okafor"),
    "ines": ("did:plc:ines", "ines.example", "Ines Calder"),
    "dmitri": ("did:plc:dmitri", "dmitri.example", "Dmitri Vale"),
    "juno": ("did:plc:juno", "juno.example", "Juno Park"),
    "tomas": ("did:plc:tomas", "tomas.example", "Tomás Reyes"),
    "basil": ("did:plc:basil", "basil.example", ""),
    "sundial": ("did:plc:sundial", "sundial.example", "Pixel Sundial"),
    "marg": ("did:plc:marg", "marg.example", "Marguerite Ng"),
}
FOLLOWED = {"wren", "ines", "dmitri", "juno", "tomas", "basil", "sundial", "marg"}


def author(key):
    did, handle, name = PEOPLE[key]
    viewer = {"following": f"at://did:plc:demo/app.bsky.graph.follow/{key}"} if key in FOLLOWED else {}
    out = {"did": did, "handle": handle, "viewer": viewer}
    if name:
        out["displayName"] = name
    return out


POSTS = {}


def post(key, rkey, who, text, minutes, replies=0, reposts=0, likes=0, quotes=0,
         embed=None, reply_to=None, liked=False, reposted=False):
    did = PEOPLE[who][0]
    uri = f"at://{did}/app.bsky.feed.post/{rkey}"
    record = {"$type": "app.bsky.feed.post", "text": text, "createdAt": ago(minutes)}
    if reply_to:
        parent = POSTS[reply_to]
        root_ref = parent["record"].get("reply", {}).get("root") or {"uri": parent["uri"], "cid": parent["cid"]}
        record["reply"] = {"root": root_ref, "parent": {"uri": parent["uri"], "cid": parent["cid"]}}
    view = {
        "uri": uri,
        "cid": f"cid-{rkey}",
        "author": author(who),
        "record": record,
        "replyCount": replies,
        "repostCount": reposts,
        "likeCount": likes,
        "quoteCount": quotes,
        "indexedAt": ago(minutes),
        "viewer": {},
    }
    if liked:
        view["viewer"]["like"] = f"at://did:plc:demo/app.bsky.feed.like/{rkey}"
    if reposted:
        view["viewer"]["repost"] = f"at://did:plc:demo/app.bsky.feed.repost/{rkey}"
    if embed:
        view["embed"] = embed
    POSTS[key] = view
    return view


def images(name, alt):
    url = f"{BASE}/img/{name}.png"
    return {"$type": "app.bsky.embed.images#view",
            "images": [{"thumb": url, "fullsize": url, "alt": alt}]}


def external(uri, title, description):
    return {"$type": "app.bsky.embed.external#view",
            "external": {"uri": uri, "title": title, "description": description}}


def quote(key):
    quoted = POSTS[key]
    return {"$type": "app.bsky.embed.record#view",
            "record": {"$type": "app.bsky.embed.record#viewRecord",
                       "uri": quoted["uri"], "cid": quoted["cid"], "author": quoted["author"],
                       "value": quoted["record"], "indexedAt": quoted["indexedAt"]}}


# --- posts ---------------------------------------------------------------------

post("launch", "launch", "wren",
     "Shipped a single-column Bluesky client for the terminal. j/k to move, l to open a "
     "thread, h to go back. Feels like reading mail in 1998, in the best way. #rust #tui",
     12, replies=6, reposts=41, likes=248, quotes=3)
post("kingfisher", "kingfisher", "juno",
     "First kingfisher of the season. Two hours in the reeds for this one.",
     34, replies=12, reposts=88, likes=1312, quotes=4,
     embed=images("kingfisher", "A kingfisher perched on a reed above still water, its orange "
                  "chest catching the low morning sun."), liked=True)
post("tree", "tree", "dmitri", "Does it do threads properly? Tree guides or bust.",
     10, replies=1, likes=9, reply_to="launch")
post("protocol", "protocol", "tomas",
     "The nice thing about an open protocol: the timeline is just JSON, and anyone can draw it "
     "however they like. Even in 100 columns.",
     9, replies=2, reposts=7, likes=63, embed=quote("launch"))
post("printat", "printat", "ines",
     "Wrote up how BASIC's PRINT AT shaped every terminal UI that came after it.",
     51, replies=4, reposts=19, likes=140,
     embed=external("https://ines.example/print-at", "PRINT AT: a short history of cursor addressing",
                    "From the ZX Spectrum to curses to ratatui."), reposted=True)
post("editor", "editor", "basil",
     "it's 2am and I just learned you can ^E out to $EDITOR from the composer. vi forever.",
     63, replies=3, likes=27)
post("sundial", "sundial", "sundial", "Today's sundial: 1,440 rings, one per minute of the day.",
     95, replies=2, reposts=31, likes=402,
     embed=images("sundial", "Concentric rings of colour shading from deep violet at the edge "
                  "to warm gold at the centre."))
post("colors", "colors", "marg",
     "Reading @wren.example's notes on terminal colour: stick to the 16 named ANSI colours and "
     "the user's theme wins. #accessibility",
     120, replies=5, reposts=22, likes=171)
post("mine", "mine", "demo", "Trying out a terminal Bluesky client today. So far: no mouse needed.",
     180, replies=1, reposts=1, likes=5, quotes=1)
# thread replies under the launch post
post("tree2", "tree2", "wren", "Tree guides. ├─ and └─, like slrn.", 8, likes=14, reply_to="tree")
post("tree3", "tree3", "dmitri", "Sold.", 7, likes=6, reply_to="tree2")
post("photos", "photos", "juno", "Can it show photos? Asking for the birds.", 9, replies=1, likes=11,
     reply_to="launch")
post("photos2", "photos2", "wren",
     "Space opens media. Halfblocks everywhere, kitty graphics if your terminal speaks it.",
     6, likes=19, reply_to="photos")
post("open", "open", "tomas", "Open protocol, open client.", 5, likes=8, reply_to="launch")
post("welcome", "welcome", "wren", "Welcome aboard! N opens notifications, ? opens the menu.", 170,
     likes=2, reply_to="mine")
post("tqt", "tqt", "tomas", "Another one for the terminal club.", 160, likes=3, embed=quote("mine"))
post("hi", "hi", "juno", "Say hi to @demo.example, new to the terminal club.", 150, likes=4)


def entry(key, reason_by=None, reply=True, minutes=None):
    view = POSTS[key]
    out = {"post": view}
    if reason_by:
        out["reason"] = {"$type": "app.bsky.feed.defs#reasonRepost", "by": author(reason_by),
                         "indexedAt": ago(minutes if minutes is not None else 20)}
    rep = view["record"].get("reply")
    if reply and rep:
        parent = next(p for p in POSTS.values() if p["uri"] == rep["parent"]["uri"])
        root = next(p for p in POSTS.values() if p["uri"] == rep["root"]["uri"])
        out["reply"] = {"root": root, "parent": parent}
    return out


TIMELINE = [entry("launch"), entry("kingfisher", reason_by="ines", minutes=14), entry("tree"),
            entry("protocol"), entry("printat"), entry("editor"), entry("sundial"),
            entry("colors")]
FEEDS = {
    "at://did:plc:juno/app.bsky.feed.generator/birds": ("Birds", [entry("kingfisher"), entry("sundial")]),
    "at://did:plc:ines/app.bsky.feed.generator/retro": ("Retro Computing",
                                                         [entry("printat"), entry("editor"), entry("launch")]),
}


def thread_node(key):
    view = POSTS[key]
    children = [k for k, p in POSTS.items()
                if p["record"].get("reply", {}).get("parent", {}).get("uri") == view["uri"]]
    order = {"tree": 0, "photos": 1, "open": 2}
    children.sort(key=lambda k: order.get(k, 9))
    return {"$type": "app.bsky.feed.defs#threadViewPost", "post": view,
            "replies": [thread_node(k) for k in children]}


def thread(uri):
    key = next(k for k, p in POSTS.items() if p["uri"] == uri)
    node = thread_node(key)
    parent_ref = POSTS[key]["record"].get("reply", {}).get("parent")
    cursor = node
    while parent_ref:
        pkey = next(k for k, p in POSTS.items() if p["uri"] == parent_ref["uri"])
        cursor["parent"] = {"$type": "app.bsky.feed.defs#threadViewPost", "post": POSTS[pkey]}
        cursor = cursor["parent"]
        parent_ref = POSTS[pkey]["record"].get("reply", {}).get("parent")
    return {"thread": node}


def notification(n, who, reason, minutes, subject=None, record=None, read=False):
    return {"uri": f"at://{PEOPLE[who][0]}/app.bsky.feed.{reason}/{n}", "cid": f"n{n}",
            "author": author(who), "reason": reason, "reasonSubject": subject,
            "record": record or {"$type": f"app.bsky.feed.{reason}"}, "isRead": read,
            "indexedAt": ago(minutes)}


MINE = POSTS["mine"]["uri"]
NOTIFICATIONS = [
    notification(1, "wren", "reply", 170, MINE, POSTS["welcome"]["record"]),
    notification(2, "juno", "like", 171, MINE), notification(3, "ines", "like", 172, MINE),
    notification(4, "dmitri", "like", 173, MINE), notification(5, "tomas", "like", 174, MINE),
    notification(6, "marg", "like", 175, MINE),
    notification(7, "marg", "follow", 176), notification(8, "basil", "follow", 177),
    notification(9, "sundial", "follow", 178),
    notification(10, "tomas", "quote", 160, MINE, POSTS["tqt"]["record"], read=True),
    notification(11, "ines", "repost", 179, MINE, read=True),
    notification(12, "juno", "mention", 150, None, POSTS["hi"]["record"], read=True),
]
for n in NOTIFICATIONS:
    if n["reason"] in ("follow", "mention"):
        n.pop("reasonSubject")


def profile(actor):
    key = next((k for k, (did, handle, _) in PEOPLE.items() if actor in (did, handle)), "demo")
    did, handle, name = PEOPLE[key]
    bios = {
        "wren": "Building terminal things in Rust. Ex-mutt, still-slrn. She/her.\n"
                "Writing about TUIs at https://wren.example",
    }
    view = dict(author(key), description=bios.get(key, ""), followersCount=12_400 if key == "wren" else 310,
                followsCount=388, postsCount=2_917 if key == "wren" else 120)
    return view


def author_feed(actor):
    key = next((k for k, (did, handle, _) in PEOPLE.items() if actor in (did, handle)), "demo")
    did = PEOPLE[key][0]
    return [entry(k) for k, p in POSTS.items() if p["author"]["did"] == did]


# --- images -----------------------------------------------------------------------

def png(width, height, pixel):
    rows = b"".join(b"\x00" + b"".join(bytes(pixel(x, y)) for x in range(width)) for y in range(height))
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b""))


def mix(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))


def kingfisher(x, y):
    w, h = 320, 200
    if y < 120:  # dawn sky
        sky = mix((246, 168, 92), (64, 132, 158), y / 120)
    else:  # water with ripples
        ripple = 0.08 * math.sin(x / 9 + y / 3)
        sky = mix((40, 96, 118), (18, 52, 70), min(1, (y - 120) / 80 + ripple))
    # reed
    if abs(x - 150 - (y - 200) * 0.15) < 2.2 and y > 92:
        return (34, 52, 30)
    for rx in (40, 58, 250, 268, 290):
        if abs(x - rx - math.sin(y / 15) * 3) < 1.6 and y > 70 + (rx % 30):
            return (30, 46, 28)
    # bird: body, belly, head, beak, eye
    def inside(cx, cy, rx, ry):
        return ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 <= 1
    if inside(166, 74, 6, 2.5) and x > 166:
        return (25, 25, 25)
    if inside(155, 74, 2.2, 2.2):
        return (250, 250, 250) if not inside(155.6, 74, 1.1, 1.1) else (10, 10, 10)
    if inside(150, 76, 12, 11):
        return (24, 120, 196)
    if inside(144, 98, 16, 20):
        return (232, 116, 36) if x > 141 else (20, 104, 180)
    if inside(126, 108, 14, 4):
        return (16, 88, 160)
    return sky


def sundial(x, y):
    cx, cy = 160, 100
    r = math.hypot(x - cx, y - cy)
    a = math.atan2(y - cy, x - cx)
    t = (math.sin(r / 4.0) + 1) / 2
    base = mix((44, 20, 92), (250, 196, 72), max(0.0, 1 - r / 140))
    glow = mix(base, (255, 120, 90), 0.35 * t * (1 + math.sin(a * 3)) / 2)
    return glow


# Drawn at 3x: terminal image widgets only shrink to fit, so a small source
# would sit in a corner of the media viewer, unlike real Bluesky photos.
SCALE = 3
IMAGES = {name: png(320 * SCALE, 200 * SCALE, lambda x, y, f=f: f(x / SCALE, y / SCALE))
          for name, f in (("kingfisher", kingfisher), ("sundial", sundial))}


# --- server -----------------------------------------------------------------------

RECORD_COUNTER = [0]


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def send(self, payload, status=200, content_type="application/json"):
        body = payload if isinstance(payload, bytes) else json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        url = urlparse(self.path)
        query = parse_qs(url.query)
        if url.path.startswith("/img/"):
            name = url.path[5:].removesuffix(".png")
            return self.send(IMAGES[name], content_type="image/png") if name in IMAGES else self.send({}, 404)
        method = url.path.removeprefix("/xrpc/")
        one = lambda key: query.get(key, [""])[0]
        if method == "app.bsky.actor.getPreferences":
            return self.send({"preferences": [{"$type": "app.bsky.actor.defs#savedFeedsPrefV2", "items": [
                {"type": "timeline", "value": "following", "pinned": True},
                *[{"type": "feed", "value": uri, "pinned": True} for uri in FEEDS]]}]})
        if method == "app.bsky.feed.getFeedGenerators":
            return self.send({"feeds": [{"uri": uri, "displayName": FEEDS[uri][0]}
                                        for uri in query.get("feeds", []) if uri in FEEDS]})
        if method == "app.bsky.feed.getTimeline":
            return self.send({"feed": TIMELINE})
        if method == "app.bsky.feed.getFeed":
            return self.send({"feed": FEEDS.get(one("feed"), ("", []))[1]})
        if method == "app.bsky.feed.getAuthorFeed":
            return self.send({"feed": author_feed(one("actor"))})
        if method == "app.bsky.feed.getPostThread":
            return self.send(thread(one("uri")))
        if method == "app.bsky.feed.getPosts":
            uris = set(query.get("uris", []))
            return self.send({"posts": [p for p in POSTS.values() if p["uri"] in uris]})
        if method == "app.bsky.actor.getProfile":
            return self.send(profile(one("actor")))
        if method == "app.bsky.notification.getUnreadCount":
            return self.send({"count": sum(1 for n in NOTIFICATIONS if not n["isRead"])})
        if method == "app.bsky.notification.listNotifications":
            return self.send({"notifications": NOTIFICATIONS, "seenAt": ago(200)})
        if method == "com.atproto.identity.resolveHandle":
            handle = one("handle")
            did = next((d for d, h, _ in PEOPLE.values() if h == handle), None)
            return self.send({"did": did}) if did else self.send({"error": "InvalidRequest"}, 400)
        return self.send({"error": "MethodNotImplemented", "message": method}, 501)

    def do_POST(self):
        length = int(self.headers.get("Content-Length") or 0)
        body = json.loads(self.rfile.read(length) or b"{}")
        method = urlparse(self.path).path.removeprefix("/xrpc/")
        if method in ("com.atproto.server.createSession", "com.atproto.server.refreshSession"):
            who = "alt" if str(body.get("identifier", "")).startswith("alt") else "demo"
            if method == "com.atproto.server.refreshSession":
                token = self.headers.get("Authorization", "")
                who = "alt" if "alt" in token else "demo"
            did, handle, _ = PEOPLE[who]
            return self.send({"did": did, "handle": handle, "accessJwt": f"access-{who}",
                              "refreshJwt": f"refresh-{who}"})
        if method == "app.bsky.notification.updateSeen":
            for n in NOTIFICATIONS:
                n["isRead"] = True
            return self.send({})
        if method == "com.atproto.repo.createRecord":
            RECORD_COUNTER[0] += 1
            collection = body.get("collection", "app.bsky.feed.post")
            return self.send({"uri": f"at://{body.get('repo')}/{collection}/demo{RECORD_COUNTER[0]}",
                              "cid": f"democid{RECORD_COUNTER[0]}"})
        if method == "com.atproto.repo.deleteRecord":
            return self.send({})
        return self.send({"error": "MethodNotImplemented", "message": method}, 501)


if __name__ == "__main__":
    print(f"mock XRPC on {BASE}", flush=True)
    ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
