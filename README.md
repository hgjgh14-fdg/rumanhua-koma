# Koma 漫画源仓库

本仓库为 Koma 阅读器提供如漫画书源，订阅后可在应用内更新。

## 订阅地址

https://github.com/hgjgh14-fdg/rumanhua-koma/releases/latest/download/index.json

订阅地址保持不变；更新订阅即可获取已通过 CI 阅读链路测试并发布的书源。

## 当前书源

| 书源 | 站点 | 功能状态 |
|---|---|---|
| 如漫画 | www.rumanhua2.com | 搜索、详情、章节、阅读图片均通过 CI 实测 |

## 构建与验证

GitHub Actions 编译并测试如漫画书源，验证搜索、漫画详情、章节目录、阅读图片和图片请求头。只有完整链路通过后，才会把 .koma 包和订阅索引发布到 GitHub Releases。


## 中文书源迁移

Keiyoushi 中文源的盘点与适配进度见 [MIGRATION_PLAN.md](MIGRATION_PLAN.md)。该清单是迁移工作台，不代表其中候选源已经进入正式订阅；只有 Koma 编译和搜索、详情、章节、图片全链路测试通过后才会发布。
