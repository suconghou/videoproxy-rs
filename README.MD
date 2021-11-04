## videoproxy-rs


golang版本

https://github.com/suconghou/videoproxy

接口基本和`videoproxy`类似

接口前缀均为/video


GET `/video/{ID}.json` 

> 默认便于缓存不输出下载地址,query参数`info=all`输出下载地址 

GET `/video/{ID}/{ITAG}.mp4` `/video/{ID}/{ITAG}.webm`

> proxy指定itag的资源,如果发起的是range请求,也支持响应range

GET `/video/{ID}/{ITAG}/{TS}.ts`

> proxy指定itag的指定range片段

GET `/video/{ID}.jpg` `/video/{ID}.webp`

> proxy资源banner图

GET `/video/{ID}.mp4` `/video/{ID}.webm` 

> 默认中等清晰度的音视频流
>
> query参数`prefer`配置清晰度优先级,根据itag列表搜寻可用资源,例如`prefer=18,22`
>









