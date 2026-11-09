# MagiDiva License Server — Cloudflare Worker 一键部署

> 网络好的时候按这个跑。全程 5 分钟。

---

## 前置条件

- ✅ 有 Cloudflare 账号（https://dash.cloudflare.com）
- ✅ magidiva.com 已在 Cloudflare DNS 管理里（Zone 添加）
- ✅ 翻墙 / VPN 能访问 Cloudflare

---

## Step 1: 安装 wrangler

```powershell
npm.cmd install -g wrangler
```

验证：
```powershell
npx.cmd wrangler --version    # 应该显示 4.x.x
```

---

## Step 2: 登录 Cloudflare

**方式 A（推荐）— API Token**：

1. 打开 https://dash.cloudflare.com/profile/api-tokens
2. **Create Token** → 选 **Edit Cloudflare Workers** 模板
3. Account 选你的账号
4. 点 **Continue to summary** → **Create Token**
5. **复制那个 token**（只显示一次！）

然后：
```powershell
$env:CLOUDFLARE_API_TOKEN = "你复制的token"
```

**方式 B — OAuth 浏览器授权**：
```powershell
npx.cmd wrangler login
# 浏览器会开，点 Allow 就行
```

---

## Step 3: 创建 KV Namespace

```powershell
cd D:\2025Project\Lucid\license-server
npx.cmd wrangler kv:namespace create MAGIDIVA_LICENSES
```

输出里找 `namespace_id`，类似：
```
🌀 Creating namespace with title "magidiva-license-MAGIDIVA_LICENSES"
✨ Success!
Add the following to your configuration file in the kv_namespaces array:
{ binding = "MAGIDIVA_LICENSES", id = "abc123def456..." }
```

**复制那个 id**，然后编辑 `wrangler.toml`：

```diff
  kv_namespaces = [
-   { binding = "MAGIDIVA_LICENSES", id = "YOUR_KV_NAMESPACE_ID" }
+   { binding = "MAGIDIVA_LICENSES", id = "你刚才复制的id" }
  ]
```

---

## Step 4: 部署 Worker

```powershell
npx.cmd wrangler deploy
```

成功后会显示：
```
Deployed magidiva-license (abc123...)
  https://magidiva-license.xxx.workers.dev
```

**复制那个 `.workers.dev` URL**。

---

## Step 5: 绑自定义域名（可选，推荐）

**方式 A — 在 wrangler.toml 里加 route**：

取消注释 wrangler.toml 里的 routes：
```toml
routes = [
  { pattern = "license.magidiva.com/*", zone_name = "magidiva.com" }
]
```

然后：
```powershell
npx.cmd wrangler deploy
```

**方式 B — Cloudflare 控制台手动**：
1. https://dash.cloudflare.com → magidiva.com → DNS
2. 加一条 CNAME: `license` → `magidiva-license.xxx.workers.dev`
3. 或者 Workers & Pages → Triggers → Custom Domains → Add custom domain → `license.magidiva.com`

---

## Step 6: 设置 Secrets

Worker 需要两个 secret：

```powershell
# Admin key — 你自己用，管理 license
npx.cmd wrangler secret put ADMIN_KEY
# 输入一个长随机字符串，比如: magidiva-admin-$(openssl rand -hex 32)

# HMAC key — 用于签名 license，防止伪造
npx.cmd wrangler secret put SERVER_HMAC_KEY
# 另一个长随机字符串
```

生成随机字符串（Windows）：
```powershell
-join ((48..57) + (65..90) + (97..122) | Get-Random -Count 64 | ForEach-Object { [char]$_ })
```

---

## Step 7: 改 Rust 代码 + 重构建

Worker URL 确定后，编辑 MagiDiva 的 `src-tauri/src/license.rs`：

```diff
- const LICENSE_SERVER: &str = "https://license.magidiva.com";
+ const LICENSE_SERVER: &str = "https://你最终的WorkerURL";
```

然后重构建 release：
```powershell
cd D:\2025Project\Lucid
pnpm pro-release
```

新的 Pro installer 会有激活功能，需要上传回 GitHub Release：
```powershell
# 下载旧 assets 名，用新构建的替换
# 然后重新 upload
```

---

## Step 8: Creem Webhook

在 Creem 后台：
1. Products → MagiDiva Pro → Webhooks
2. Add webhook URL: `https://license.magidiva.com/api/license/issue`
3. Event: `payment.success` 或你 Creem 里的对应事件

这样用户买完 Pro，Worker 会自动发 license key 到他邮箱。

---

## 🧪 测试

部署完后，测试 API：

```powershell
# Test issue (mock webhook payload)
$body = @{ event = "payment.success"; email = "test@example.com"; product = "pro" } | ConvertTo-Json
Invoke-RestMethod -Uri "https://license.magidiva.com/api/license/issue" -Method POST -ContentType "application/json" -Body $body

# Test verify
$body2 = @{ email = "test@example.com"; key = "上一步生成的key" } | ConvertTo-Json
Invoke-RestMethod -Uri "https://license.magidiva.com/api/license/verify" -Method POST -ContentType "application/json" -Body $body2
```

---

## 🔁 如果想卸载

```powershell
npx.cmd wrangler delete
npx.cmd wrangler kv:namespace delete <namespace_id>
```
