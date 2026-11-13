# 漫蛙 Koma 书源

适配漫蛙公开漫画页面，支持搜索、详情、章节目录和阅读页图片提取。

主站候选域名来自公开漫蛙适配器的域名配置：
- `https://manwapr.cc`
- `https://manwass.cc`
- `https://manwatg.cc`
- `https://manwa.me`
- `https://manwast.cc`
- `https://manwasy.cc`

书源设置中可以切换域名。默认先尝试 `manwapr.cc`，旧版宿主会自动探测候选域名。

阅读页使用默认图片线路，不主动附加可选的加密图源参数。若站点后续强制启用加密图源，需要再实现相应图片解密流程。
