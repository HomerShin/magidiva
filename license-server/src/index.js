// ──────────────────────────────────────────────────────────
// MagiDiva License Server — Cloudflare Worker
// Deploy: wrangler deploy
// KV binding: MAGIDIVA_LICENSES
// ──────────────────────────────────────────────────────────

const SERVER_HMAC_KEY = "magiDiva-SERVER-PRIVATE-KEY-v1"; // env var in production
const CLIENT_HMAC_KEY = "magiDiva-CLIENT-PUBLIC-VERIFY-KEY-v1"; // must match Rust const

// ─── Helpers ───────────────────────────────────────────────

function generateLicenseKey() {
  const chars = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"; // no 0/O/1/I confusion
  let groups = [];
  for (let g = 0; g < 4; g++) {
    let s = "";
    for (let i = 0; i < 4; i++) s += chars[Math.floor(Math.random() * chars.length)];
    groups.push(s);
  }
  return `MGD-${groups.join("-")}`;
}

async function hmacSign(key, msg) {
  const enc = new TextEncoder();
  const cryptoKey = await crypto.subtle.importKey(
    "raw", enc.encode(key), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]
  );
  const sig = await crypto.subtle.sign("HMAC", cryptoKey, enc.encode(msg));
  return Array.from(new Uint8Array(sig)).map(b => b.toString(16).padStart(2, "0")).join("");
}

function jsonResponse(obj, status = 200, headers = {}) {
  return new Response(JSON.stringify(obj), {
    status,
    headers: { "Content-Type": "application/json", ...headers },
  });
}

// ─── Routes ─────────────────────────────────────────────────

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const path = url.pathname;

    // CORS preflight
    if (request.method === "OPTIONS") {
      return new Response(null, {
        headers: {
          "Access-Control-Allow-Origin": "*",
          "Access-Control-Allow-Methods": "POST, GET, OPTIONS",
          "Access-Control-Allow-Headers": "Content-Type, Authorization",
        },
      });
    }

    // CORS for all responses
    const cors = { "Access-Control-Allow-Origin": "*" };

    try {
      // POST /api/license/issue — Creem webhook
      if (path === "/api/license/issue" && request.method === "POST") {
        return handleIssue(request, env, cors);
      }

      // POST /api/license/verify — Pro client activation
      if (path === "/api/license/verify" && request.method === "POST") {
        return handleVerify(request, env, cors);
      }

      // POST /api/license/generate — Admin manual issue
      if (path === "/api/license/generate" && request.method === "POST") {
        return handleManualIssue(request, env, cors);
      }

      // GET /api/license/status?email=xxx — Lookup
      if (path === "/api/license/status" && request.method === "GET") {
        return handleStatus(url, env, cors);
      }

      // GET /api/health
      if (path === "/api/health") {
        return jsonResponse({ ok: true, version: "1.0.0" }, 200, cors);
      }

      return jsonResponse({ error: "Not found" }, 404, cors);
    } catch (e) {
      return jsonResponse({ error: e.message }, 500, cors);
    }
  },
};

// ─── POST /api/license/issue (Creem webhook) ──────────────

async function handleIssue(request, env, cors) {
  let body;
  try { body = await request.json(); } catch {
    return jsonResponse({ error: "Invalid JSON" }, 400, cors);
  }

  // Verify Creem signature (X-Creem-Signature header)
  const creemSig = request.headers.get("x-creem-signature");
  // In dev/test mode, signature can be skipped (Creem sends it in live mode)
  // For now we trust the request — add strict verification later

  const email = body.customer_email || body.email;
  const plan = body.product_id === "prod_EUD8f3X33NPwlEf3FjPRr" ? "pro" : "unknown";
  const amount = body.amount || 0;

  if (!email) {
    return jsonResponse({ error: "Missing customer_email" }, 400, cors);
  }

  // Check if already issued for this email
  const existing = await env.MAGIDIVA_LICENSES.get(email);
  if (existing) {
    return jsonResponse({
      ok: true, email, plan,
      message: "License already issued — check your email",
    }, 200, cors);
  }

  // Generate key + token
  const key = generateLicenseKey();
  const now = Math.floor(Date.now() / 1000);
  const expiresAt = now + 365 * 24 * 3600 * 10; // perpetual (10 years)
  const token = await hmacSign(SERVER_HMAC_KEY, `${email}|${plan}|${expiresAt}`);

  const record = {
    email, key, plan, amount,
    issued_at: now,
    expires_at: expiresAt,
    client_token: await hmacSign(CLIENT_HMAC_KEY, `${email}|${plan}|${expiresAt}`),
  };

  // Store in KV (TTS: never expires)
  await env.MAGIDIVA_LICENSES.put(email, JSON.stringify(record));
  // Also index by key for reverse lookup
  await env.MAGIDIVA_LICENSES.put(`key:${key}`, email);

  // Email would be sent here via Resend API
  // For now we just log
  console.log(`ISSUED ${plan} → ${email} key=${key}`);

  return jsonResponse({ ok: true, email, key, plan }, 200, cors);
}

// ─── POST /api/license/verify (client activation) ─────────

async function handleVerify(request, env, cors) {
  let body;
  try { body = await request.json(); } catch {
    return jsonResponse({ error: "Invalid JSON" }, 400, cors);
  }

  const { email, key } = body;
  if (!email || !key) {
    return jsonResponse({ valid: false, message: "Email and key required" }, 400, cors);
  }

  const raw = await env.MAGIDIVA_LICENSES.get(email);
  if (!raw) {
    return jsonResponse({ valid: false, message: "No license found for this email" }, 200, cors);
  }

  const record = JSON.parse(raw);
  if (record.key !== key) {
    return jsonResponse({ valid: false, message: "License key does not match" }, 200, cors);
  }

  const now = Math.floor(Date.now() / 1000);
  if (record.expires_at && now > record.expires_at) {
    return jsonResponse({ valid: false, message: "License expired" }, 200, cors);
  }

  // Return signed token for client-side offline grace
  return jsonResponse({
    valid: true,
    plan: record.plan,
    token: record.client_token, // pre-computed with CLIENT_HMAC_KEY
    message: "OK",
  }, 200, cors);
}

// ─── POST /api/license/generate (admin helper) ────────────

async function handleManualIssue(request, env, cors) {
  let body;
  try { body = await request.json(); } catch {
    return jsonResponse({ error: "Invalid JSON" }, 400, cors);
  }

  // Admin auth — simple header check
  const adminKey = request.headers.get("x-admin-key");
  if (adminKey !== env.ADMIN_KEY) {
    return jsonResponse({ error: "Unauthorized" }, 401, cors);
  }

  const email = body.email;
  const plan = body.plan || "pro";

  if (!email) {
    return jsonResponse({ error: "Missing email" }, 400, cors);
  }

  const key = generateLicenseKey();
  const now = Math.floor(Date.now() / 1000);
  const expiresAt = now + 365 * 24 * 3600 * 10;
  const token = await hmacSign(CLIENT_HMAC_KEY, `${email}|${plan}|${expiresAt}`);

  const record = {
    email, key, plan, amount: body.amount || 0,
    issued_at: now, expires_at: expiresAt,
    client_token: token,
  };

  await env.MAGIDIVA_LICENSES.put(email, JSON.stringify(record));
  await env.MAGIDIVA_LICENSES.put(`key:${key}`, email);

  return jsonResponse({ ok: true, ...record }, 200, cors);
}

// ─── GET /api/license/status ──────────────────────────────

async function handleStatus(url, env, cors) {
  const email = url.searchParams.get("email");
  if (!email) return jsonResponse({ error: "Missing email param" }, 400, cors);

  const raw = await env.MAGIDIVA_LICENSES.get(email);
  if (!raw) return jsonResponse({ found: false }, 200, cors);

  const r = JSON.parse(raw);
  return jsonResponse({
    found: true, email: r.email, plan: r.plan,
    issued_at: r.issued_at, expires_at: r.expires_at,
  }, 200, cors);
}
