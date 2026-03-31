import http.client
import json
import urllib.request
import urllib.parse

API_KEY = "sk-8bkiJfP63jpcGnJQbTfqiVvqGLT4lK6FytKZSTQ3Z323G7HI"
IMAGE_FILE = "test.jpg"

# Step 1: 上传参考图
print(">>> 正在上传参考图...")
boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW"
with open(IMAGE_FILE, "rb") as f:
    file_data = f.read()

body = (
    (
        f"--{boundary}\r\n"
        f'Content-Disposition: form-data; name="file"; filename="{IMAGE_FILE}"\r\n'
        f"Content-Type: image/jpeg\r\n\r\n"
    ).encode()
    + file_data
    + f"\r\n--{boundary}--\r\n".encode()
)

req = urllib.request.Request(
    "https://imageproxy.zhongzhuan.chat/api/upload",
    data=body,
    headers={
        "Authorization": f"Bearer {API_KEY}",
        "Content-Type": f"multipart/form-data; boundary={boundary}",
    },
    method="POST",
)

with urllib.request.urlopen(req, timeout=30) as resp:
    upload_result = json.loads(resp.read().decode())
    image_url = upload_result["url"]
    print(f">>> 上传成功: {image_url}")

# Step 2: 调用生成 API
print("\n>>> 正在调用图像生成 API...")
conn = http.client.HTTPSConnection("api.qianhai.online", timeout=120)
payload = json.dumps(
    {
        "max_tokens": 8000,
        "model": "gemini-3.1-flash-image-preview",
        
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "主题风格改成红色"},
                    {"type": "image_url", "image_url": {"url": image_url},"aspectRatio": "9:16", "imageSize": "1K"},
                ],
            }
        ],
    }
)
headers = {
    "Accept": "application/json",
    "Authorization": f"Bearer {API_KEY}",
    "Content-Type": "application/json",
}
conn.request("POST", "/v1/chat/completions", payload, headers)
res = conn.getresponse()
data = res.read().decode("utf-8")
print(f">>> 状态码: {res.status}")

# Step 3: 解析并保存结果
result = json.loads(data)
print(json.dumps(result, indent=2, ensure_ascii=False))

# 提取生成的图片并保存
import base64
import re

try:
    for choice in result.get("choices", []):
        msg = choice.get("message", {})
        content = msg.get("content", "")
        img_src = ""
        # content 可能是字符串（含 base64 图片）或列表
        if isinstance(content, list):
            for part in content:
                if part.get("type") == "image_url":
                    img_src = part["image_url"]["url"]
        elif isinstance(content, str):
            img_src = content

        # 从 markdown 或 data URI 中提取 base64
        b64_match = re.search(r"data:image/(\w+);base64,([A-Za-z0-9+/=]+)", img_src)
        if b64_match:
            ext = "jpg" if b64_match.group(1) == "jpeg" else b64_match.group(1)
            img_data = base64.b64decode(b64_match.group(2))
            img_path = f"test_result.{ext}"
            with open(img_path, "wb") as f:
                f.write(img_data)
            print(f"\n>>> 生成图片已保存到 {img_path} ({len(img_data)} bytes)")
        else:
            print(f"\n>>> 未找到 base64 图片数据，原始内容前200字符:")
            print(img_src[:200])
except Exception as e:
    print(f"\n>>> 解析/保存图片时出错: {e}")

conn.close()
