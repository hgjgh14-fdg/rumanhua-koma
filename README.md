# Koma 漫画源仓库

本仓库为 Koma 阅读器提供独立漫画源，订阅后可在应用内更新。

## 订阅地址

https://github.com/hgjgh14-fdg/rumanhua-koma/releases/latest/download/index.json

订阅地址保持不变；更新订阅即可获取仓库中已发布的书源。

## 当前书源

| 书源 | 站点 | 功能状态 |
|---|---|---|
| 如漫画 | www.rumanhua2.com | 搜索、详情、章节、阅读图片均通过 CI 实测 |
| 包子漫画 | baozimh.org | 搜索、详情、章节目录、阅读图片解码均通过 CI 实测 |
| 漫蛙 | manwapr.cc 等候选域名 | 正在接入，需通过完整 CI 阅读链路后才发布 |

## 包子漫画接口

包子漫画书源适配 baozimh.org 的公开页面和接口：
- 搜索：/s/关键词?page=1
- 详情：/manga/slug
- 章节目录：api-get-v2.mgsearcher.com/api/manga/get
- 阅读图片：v2.apikk.top/api/v2/chapter/getinfo
- 图片 CDN：c-nd2-1.6wm.top、c-nd3-1.6wm.top

## 构建与验证

GitHub Actions 会编译各书源，并分别测试搜索、漫画详情、章节目录、阅读图片和图片请求头。只有完整链路通过后，才会把 .koma 包和订阅索引发布到 GitHub Releases。

## 相关域名记录

site-candidates.json 记录了包子漫画、漫蛙的候选域名、访问结果和接口域名。仅首页可访问不代表书源已经可读。
