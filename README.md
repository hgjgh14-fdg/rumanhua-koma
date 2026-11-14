# Koma 漫画源仓库

本仓库为 Koma 阅读器提供独立漫画源，订阅后可在应用内更新。

## 订阅地址

https://github.com/hgjgh14-fdg/rumanhua-koma/releases/latest/download/index.json

订阅地址保持不变；更新订阅即可获取已通过完整 CI 阅读链路测试并发布的书源。

## Keiyoushi 中文源移植

Keiyoushi 中文扩展目录的逐项盘点与 Koma 适配状态见 [MIGRATION_PLAN.md](./MIGRATION_PLAN.md)。该清单区分了 Koma 已有候选、疑似同站别名、当前仓库实验源和待移植源；只有通过完整阅读链路测试的书源才会进入正式订阅索引。

## 当前书源

| 书源 | 站点 | 功能状态 |
|---|---|---|
| 如漫画 | www.rumanhua2.com | 搜索、详情、章节、阅读图片均通过 CI 实测 |
| 包子漫画 | baozimh.org | 搜索、详情、章节目录、阅读页图片实际下载与解码均通过 CI 实测 |
| 漫蛙 | manwa.xin 官方防走失页 | 入口已核实；官方入口暂不可供第三方阅读器完成公开阅读链路，实验性代码暂不发布 |

## 包子漫画接口

包子漫画书源适配 baozimh.org 的公开页面和接口：
- 搜索：/s/关键词?page=1
- 详情：/manga/slug
- 章节目录：v2.apikk.top/api/v2/manga/get
- 阅读图片：v2.apikk.top/api/v2/chapter/getinfo
- 图片 CDN：c-nd2-1.6wm.top、c-nd3-1.6wm.top

## 构建与验证

GitHub Actions 编译并测试已确认可用的书源，分别验证搜索、漫画详情、章节目录、阅读图片和图片请求头。只有完整链路通过后，才会把 .koma 包和订阅索引发布到 GitHub Releases。

## 漫蛙说明

漫蛙官方防走失页为 https://manwa.xin/，列出的入口包括 manwa01.com/VME50、rpmy.mobistatic.cc/wa188（大陆入口）和 manwa.la（非大陆入口）。截至 2026-11-13，CI 对 manwa.la 的连接超时，对 manwa01.com/VME50 的搜索路径返回 404，rpmy.mobistatic.cc/wa188 显示测试节点维护提示。旧适配器域名也出现跳转至无关站点或 HTTP 444。

官方页面明确提示会封锁其他无广告阅读器。因此，当前不绕过其访问限制、验证码、登录或付费章节控制；漫蛙代码仅保留为实验性研究，不进入订阅发布包。域名测试详情见 site-candidates.json。
