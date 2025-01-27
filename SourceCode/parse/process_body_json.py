


def process_body_resp(resp):
    item = {}
    data = resp.json()
    item["mid"] = data["mid"]
    item["uid"] = data["user"]["id"]
    item["name"] = data["page_info"]["user"]["screen_name"]
    item["text"] = data["text_raw"]
    item["text_raw"] = data["text"]
    item["reposts_count"] = data["reposts_count"]
    item["comments_count"] = data["comments_count"]
    item["attitudes_count"] = data["attitudes_count"]
    return item