


def process_body_resp(resp):
    item = {}
    data = resp.json()
    item["mid"] = str(data["mid"])
    item["uid"] = str(data["user"]["id"])
    item["name"] = data["user"]["screen_name"]
    item["text"] = data["text_raw"]
    item["text_raw"] = data["text"]
    item["reposts_count"] = data["reposts_count"]
    item["comments_count"] = data["comments_count"]
    item["attitudes_count"] = data["attitudes_count"]
    return item