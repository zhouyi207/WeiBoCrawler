def process_body_resp(resp):
    """处理详细页数据

    这里一般都会收到正常的响应，所以只需要处理数据即可.
    Args:
        resp (httpx.Response): 接受到的响应.

    Returns:
        list[dict]: 响应的数据, 这里使用 list 包装一下(对齐其他的process请求).
    """
    data = resp.json()
    return [data]


# def process_body_json(resp):
#     item = {}
#     data = resp.json()
#     item["mid"] = str(data["mid"])
#     item["uid"] = str(data["user"]["id"])
#     item["name"] = data["user"]["screen_name"]
#     item["text"] = data["text_raw"]
#     item["text_raw"] = data["text"]
#     item["reposts_count"] = data["reposts_count"]
#     item["comments_count"] = data["comments_count"]
#     item["attitudes_count"] = data["attitudes_count"]
#     return item
