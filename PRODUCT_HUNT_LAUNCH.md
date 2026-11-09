# MagiDiva — Product Hunt Launch Kit

> 复制粘贴到 Product Hunt submission form 里就行

---

## 📅 推荐 Launch 时间

- **日期**: 今天 或 明天（周一 > 周日，流量高 60%）
- **时间**: **12:01 AM PT**（北京时间 3:01 PM）
- **理由**: PH 每 24h 重置排名，12:01 AM PT 发 = 抢 24h 窗口期最大曝光

---

## 📝 PH Submission Fields

### Logo
- 用你现在 MagiDiva 的图标（紫色渐变 + 人像剪影 + ✨）
- 240×240 PNG，<3MB
- 路径: `D:\2025Project\Lucid\src-tauri\icons\128x128@2x.png`（如果是 256px 就直接用）

### Tagline（≤60 字符，最重要！）

```
AI background removal — local, private, free. No uploads, no subscriptions.
```

### Description（2–3 段）

```
MagiDiva removes image backgrounds in 1 click — and your photos never leave your computer.

Built for creators, designers, and indie hackers who are tired of services that upload their photos to third-party servers (looking at you, remove.bg), charge per-image, or lock features behind subscriptions.

✨ Runs 100% local on your Windows machine
🔒 No uploads, no tracking, no cloud
🆓 Free forever for single-image use (Community edition)
💎 Pro for batch processing, folder drop, and gallery view ($49 perpetual)
🔧 Custom backgrounds, shadows, watermarks, size presets
📦 One-click installer, no account required
```

### First Comment（你自己发，置顶故事）

```
Hey PH! 👋

I built MagiDiva because I was frustrated with existing background removal tools:

❌ remove.bg charges per image AND uploads your photos to their cloud
❌ Adobe Express needs a $9.99/mo subscription just for basic cutout
❌ Most "free" tools watermark exports or have 720p limits

So I took BirefNet (the same AI model remove.bg uses), wrapped it in a Tauri desktop app, and made it run 100% locally on your machine. No uploads. No tracking. Your photos stay on your disk.

Two editions:
• **Community** — free forever, AGPLv3 open source, single image cutout
• **Pro** — $49 perpetual, batch + folder drop + gallery view

Built in 3 weeks — Tauri v2, React 19, Rust backend, ONNX Runtime with DirectML GPU acceleration.

Would love to hear what you think — happy to answer questions! 🚀
```

### Topics（选 5 个）

1. **Developer Tools** ⭐️ 最重要，indie hacker 聚集
2. **Design Tools**
3. **Photography**
4. **Open Source**
5. **Artificial Intelligence**

---

## 🖼️ Screenshots（4 张，1280×720）

### Screenshot 1 — Hero Cutout
**建议**: 选一张对比感强的 before/after
- **Before**: 红色手提包放在浅色桌面，背景杂乱
- **After**: 干净透明底的产品图（PNG 导出）
- **文字 overlay**: "Before → After · 1 Click"

### Screenshot 2 — Main Interface
**建议**: MagiDiva 主界面，加载一张图的状态
- 左侧: Upload area（或已加载的原图）
- 右侧: 抠好的预览 + 编辑面板（背景色、阴影、水印）
- 突出显示 "Local Processing · No Upload" 徽章

### Screenshot 3 — Pro Batch Mode
**建议**: 展示 Pro 版独有的批处理/画廊视图
- 左侧: 拖入的文件夹 + 进度条
- 右侧: 画廊视图（多张小图 + 选中的大图）
- 标注 "🔒 Pro Only — Batch + Gallery"

### Screenshot 4 — Export Panel
**建议**: 展示导出选项的丰富度
- Format: PNG / JPG / WebP
- Quality slider
- Custom resolution (W×H)
- Drop shadow + watermark preview

---

## 🌊 Launch Day Wave 策略

```
12:01 AM PT  Post 产品 + 立即发 First Comment（上面的故事）
             → 评论区占住第一个位置 = 高权重

12:05 AM PT  发你自己的社交渠道:
             · X (Twitter): "Just launched on Product Hunt! 🚀 [链接]"
             · 小红书 技术圈: "我做了一个本地 AI 抠图工具 MagiDiva"
             · Reddit r/indiehackers: (单独发帖故事，别只放链接)

12:15 AM PT  小群发（已加入的社群）:
             · Indie Hackers Discord
             · Solo Developer 群
             · AI Tools 群
             → 消息: "做了 3 周的 MagiDiva 今天上 PH 了，求支持 🙏"

1:00 AM PT   回到 PH 页面 — 回复每一条评论！
             → 每条评论都回 = PH 算法给你更多曝光

2:01 AM PT   （2 小时后）看排名
             → 前 10 → 稳了
             → 掉出前 20 → 再发一轮小群

全天         每小时刷一次 PH，回复评论，点赞评论的人
             → Product of the Day = 最终 10k+ UV
```

---

## 📦 Landing Page 下载链接

PH description 里的链接指向:

| 内容 | URL |
|------|-----|
| 首页 | https://magidiva.com |
| 下载 Community | https://github.com/HomerShin/magidiva/releases/download/v0.1.0/MagiDiva_Community_0.1.0_x64-setup.exe |
| 下载 Pro | https://github.com/HomerShin/magidiva/releases/download/v0.1.0/MagiDiva-Pro_0.1.0_x64-setup.exe |
| GitHub Repo | https://github.com/HomerShin/magidiva |
| Creem 付款（Pro） | https://www.creem.io/payment/prod_EUD8f3X33NPwlEf3FjPRr |

---

## 🎯 成功指标（24h 内）

| 指标 | 保守 | Product of the Day |
|------|------|-------------------|
| Upvotes | 100+ | 500+ |
| Comments | 15+ | 80+ |
| magidiva.com UV | 1k+ | 10k+ |
| GitHub stars | 50+ | 300+ |
| Community installer 下载 | 20+ | 200+ |
| Pro 付费（如果 Worker 部署了） | 0-5 | 10-50 |

---

## ⚠️ 大忌

1. ❌ 别一次性群发所有群 — PH 算法检测 spam
2. ❌ 别用 "revolutionary" / "blazing-fast" — 太 hype，PH 用户反感
3. ❌ 别周末 launch — 流量降 60-70%
4. ❌ 别在 First Comment 里放折扣码 — PH 不欢迎
5. ❌ 别自己 upvote — PH 检测到直接 shadowban

---

## 📸 快速生成 Screenshot 的方法（5 分钟）

1. 启动 MagiDiva Community:
   ```
   cd D:\2025Project\Lucid
   pnpm release
   ```
2. 打开应用，加载一张你喜欢的图
3. **Win + Shift + S** 截图
4. 截 4 张:
   - 主界面（加载完一张图 + 抠好预览）
   - 背景色 + 阴影编辑面板
   - 导出对话框（Format + Quality + Resolution）
   - GitHub Release 页面的两种 installer 对比
5. 保存到 `D:\2025Project\Lucid\_ph_screenshots\`

---

## 🔗 事后跟踪

Launch 完后 48h:

```bash
# GitHub stars 增长
curl -s https://api.github.com/repos/HomerShin/magidiva | grep stargazers

# Release 下载量
curl -s https://api.github.com/repos/HomerShin/magidiva/releases/tags/v0.1.0 | grep -E '"name"|"download_count"'

# Landing page UV（如果 Cloudflare Analytics 开了）
# https://dash.cloudflare.com → magidiva.com → Analytics
```
