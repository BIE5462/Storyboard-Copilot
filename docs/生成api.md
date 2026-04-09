# DashScope Qwen 图像生成 API 说明

## 1. 范围

本文档只描述当前项目中已经落地的 DashScope Qwen 图像方案，不再保留“Qianhai Qwen”兼容写法。

| 项目 | 说明 |
| --- | --- |
| 供应商 ID | `dashscope` |
| 供应商展示名 | `DashScope / 阿里云百炼` |
| 当前模型 | `dashscope/qwen-image-2.0-pro`、`dashscope/qwen-image-2.0` |
| 实际请求模型名 | `qwen-image-2.0-pro`、`qwen-image-2.0` |
| 上游接口 | `POST https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation` |

## 2. 鉴权

所有请求统一使用：

```http
Authorization: Bearer <DASHSCOPE_API_KEY>
Content-Type: application/json
```

设置页中 API Key 只存 `apiKeys['dashscope']`，不再存在 Qianhai-Qwen 独立 Key。

## 3. 请求协议

项目内统一走 DashScope 多模态协议：

1. `input.messages[0].content` 中，所有参考图项必须排在文本前面。
2. 每张参考图使用 `{ "image": "<https-or-data-url>" }`。
3. 文本提示词使用单个 `{ "text": "<prompt>" }`，并放在最后。
4. 所有固定参数放在 `parameters` 中。

请求体示例：

```json
{
  "model": "qwen-image-2.0-pro",
  "input": {
    "messages": [
      {
        "role": "user",
        "content": [
          { "image": "https://cdn.example.com/reference-1.png" },
          { "image": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAA..." },
          { "text": "保留江南水墨氛围，在画面右下角补充题字与印章。" }
        ]
      }
    ]
  },
  "parameters": {
    "n": 1,
    "negative_prompt": " ",
    "prompt_extend": true,
    "watermark": false,
    "size": "2048*872"
  }
}
```

## 4. 固定参数

当前版本不在前端暴露独立表单控件，适配层固定写死：

| 字段 | 固定值 |
| --- | --- |
| `parameters.n` | `1` |
| `parameters.negative_prompt` | `" "` |
| `parameters.prompt_extend` | `true` |
| `parameters.watermark` | `false` |
| `parameters.size` | 当前节点选中的合法尺寸 |

## 5. 参考图规则

| 项目 | 规则 |
| --- | --- |
| 最大数量 | 最多 `3` 张 |
| 支持来源 | `https://...`、`http://...`、`data:image/...` |
| 本地路径 | 前端先转成 Data URL，再发给 DashScope |
| Storyboard 网格图 | 自动附加的网格参考图同样计入 3 张上限 |
| 超限策略 | 前端直接报错并阻止发起请求，不做静默截断 |
| 大小限制 | 单张最终提交内容必须不超过 `10MB` |

前端归一化策略：

1. DashScope Qwen 不复用 Qianhai 的低分辨率预览策略。
2. 优先保留细节，长边目标按 `2048 -> 1792 -> 1536 -> 1280 -> 1024 -> 768 -> 512 -> 384` 逐级压缩。
3. 归一化结果优先提交 `data:image/...`，并保证不超过 `10MB`。

## 6. 尺寸白名单

项目内只允许以下推荐尺寸，统一使用 `W*H` 格式：

| 比例 | 可用尺寸 |
| --- | --- |
| `1:1` | `1024*1024`、`1536*1536` |
| `2:3` | `768*1152`、`1024*1536` |
| `3:2` | `1152*768`、`1536*1024` |
| `3:4` | `960*1280`、`1080*1440` |
| `4:3` | `1280*960`、`1440*1080` |
| `9:16` | `720*1280`、`1080*1920` |
| `16:9` | `1280*720`、`1920*1080` |
| `21:9` | `1344*576`、`2048*872` |

额外约定：

1. 历史尺寸值 `1024x1024` 会在适配层归一化为 `1024*1024`。
2. 不在白名单中的尺寸不再透传到上游。

## 7. 响应解析

成功响应时，从 `output.choices[0].message.content[]` 中提取第一项 `image` 字段作为结果图 URL。

响应示例：

```json
{
  "output": {
    "choices": [
      {
        "message": {
          "role": "assistant",
          "content": [
            {
              "image": "https://dashscope-result.example.com/tmp/generated-image.webp"
            }
          ]
        }
      }
    ]
  },
  "request_id": "dashscope-example-request-id"
}
```

失败时优先透出：

| 字段 | 用途 |
| --- | --- |
| `code` | 上游错误码 |
| `message` | 上游错误描述 |
| `request_id` | 请求追踪 ID |

## 8. cURL 示例

### 8.1 纯文生图

```bash
curl --location 'https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation' \
  --header 'Content-Type: application/json' \
  --header "Authorization: Bearer $DASHSCOPE_API_KEY" \
  --data '{
    "model": "qwen-image-2.0-pro",
    "input": {
      "messages": [
        {
          "role": "user",
          "content": [
            {
              "text": "生成一张 1:1 的江南水墨风插画，柳树、石桥、小舟，画面留白克制，不要出现文字。"
            }
          ]
        }
      ]
    },
    "parameters": {
      "n": 1,
      "negative_prompt": " ",
      "prompt_extend": true,
      "watermark": false,
      "size": "1024*1024"
    }
  }'
```

### 8.2 多参考图编辑

```bash
curl --location 'https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation' \
  --header 'Content-Type: application/json' \
  --header "Authorization: Bearer $DASHSCOPE_API_KEY" \
  --data '{
    "model": "qwen-image-2.0",
    "input": {
      "messages": [
        {
          "role": "user",
          "content": [
            {
              "image": "https://cdn.example.com/reference-1.png"
            },
            {
              "image": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAA..."
            },
            {
              "text": "在画面右下角石板路旁，以浅灰墨色手写体补一首七言绝句，并在末句右下角加一枚朱红小印。"
            }
          ]
        }
      ]
    },
    "parameters": {
      "n": 1,
      "negative_prompt": " ",
      "prompt_extend": true,
      "watermark": false,
      "size": "1536*1024"
    }
  }'
```

## 9. Smoke 测试脚本

仓库内提供本地 smoke script：

```bash
npm run test:dashscope
```

环境变量：

```bash
DASHSCOPE_API_KEY=your_api_key
```

可选追加一个本地图片路径作为参考图输入：

```bash
npm run test:dashscope -- ./path/to/reference.png
```
