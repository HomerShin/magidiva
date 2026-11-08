"""
MagiDiva License Server — Local Mock (for testing before Cloudflare Worker)

Runs on http://localhost:8787
Points Tauri's LICENSE_SERVER at it (edit const in lib.rs to "http://localhost:8787")

Usage: python _license_mock_server.py
"""

import http.server
import json
import secrets
import hashlib
import hmac
import time

CLIENT_HMAC_KEY = b"magiDiva-CLIENT-PUBLIC-VERIFY-KEY-v1"
SERVER_HMAC_KEY = b"magiDiva-SERVER-PRIVATE-KEY-v1"

# In-memory DB
DB = {}


def generate_key():
    chars = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"
    groups = ["".join(secrets.choice(chars) for _ in range(4)) for _ in range(4)]
    return "MGD-" + "-".join(groups)


def hmac_sign(key: bytes, msg: str) -> str:
    return hmac.new(key, msg.encode(), hashlib.sha256).hexdigest()


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        pass  # quiet

    def _cors(self):
        return {"Access-Control-Allow-Origin": "*", "Access-Control-Allow-Headers": "Content-Type"}

    def _send(self, obj, status=200):
        data = json.dumps(obj).encode()
        self.send_response(status)
        for k, v in self._cors().items():
            self.send_header(k, v)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def _body(self):
        n = int(self.headers.get("Content-Length", 0))
        if n == 0:
            return {}
        return json.loads(self.rfile.read(n))

    def do_OPTIONS(self):
        self.send_response(204)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "POST, GET, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()

    def do_POST(self):
        url = self.path
        body = self._body()

        if url == "/api/license/issue":
            email = body.get("customer_email") or body.get("email")
            plan = "pro"
            if not email:
                self._send({"error": "missing email"}, 400)
                return
            if email in DB:
                self._send({"ok": True, "email": email, "key": DB[email]["key"], "message": "already issued"})
                return
            key = generate_key()
            now = int(time.time())
            expires = now + 365 * 24 * 3600 * 10
            token = hmac_sign(CLIENT_HMAC_KEY, f"{email}|{plan}|{expires}")
            DB[email] = {
                "email": email, "key": key, "plan": plan,
                "issued_at": now, "expires_at": expires,
                "client_token": token,
            }
            print(f"  ✓ ISSUED {email} → {key}")
            self._send({"ok": True, "email": email, "key": key, "plan": plan})
            return

        if url == "/api/license/verify":
            email = body.get("email", "")
            key = body.get("key", "")
            rec = DB.get(email)
            if not rec:
                self._send({"valid": False, "message": "No license found for this email"}, 200)
                return
            if rec["key"] != key:
                self._send({"valid": False, "message": "License key does not match"}, 200)
                return
            self._send({
                "valid": True,
                "plan": rec["plan"],
                "token": rec["client_token"],
                "message": "OK",
            })
            return

        self._send({"error": "not found"}, 404)

    def do_GET(self):
        if self.path == "/api/health":
            self._send({"ok": True, "version": "mock-1.0"})
            return
        self._send({"error": "not found"}, 404)


if __name__ == "__main__":
    print("🧪 MagiDiva License Mock Server")
    print("   POST http://localhost:8787/api/license/issue  — generate key")
    print("   POST http://localhost:8787/api/license/verify  — verify key")
    print("   http://localhost:8787/api/health               — health check")
    print()
    print("To test activation, change LICENSE_SERVER in src-tauri/src/lib.rs to:")
    print('  const LICENSE_SERVER: &str = "http://localhost:8787";')
    print()
    httpd = http.server.HTTPServer(("localhost", 8787), Handler)
    httpd.serve_forever()
