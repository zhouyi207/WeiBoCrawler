import pandas as pd
from httpx import Response


def process_comment_resp(resp: Response) -> dict:
    lst = []
    
    for comment in resp.json()["data"]:
        comment["text_raw"] = comment["text"]
        comment["text"] = comment["text_raw"].replace("\n", "")
        comment["text"] = comment["text"].replace(" ", "")
        comment["text"] = comment["text"].replace("\t", "")
        comment["text"] = comment["text"].replace("\r", "")


    data_user = pd.json_normalize(data["user"])
    data_user_col_map = {
        "id": "uid",
        "screen_name": "用户昵称",
        "profile_url": "用户主页",
        "description": "用户描述",
        "location": "用户地理位置",
        "gender": "用户性别",
        "followers_count": "用户粉丝数量",
        "friends_count": "用户关注数量",
        "statuses_count": "用户全部微博",
        "status_total_counter.comment_cnt": "用户累计评论",
        "status_total_counter.repost_cnt": "用户累计转发",
        "status_total_counter.like_cnt": "用户累计获赞",
        "status_total_counter.total_cnt": "用户转评赞",
        "verified_reason": "用户认证信息",
    }

    data_user_col = [col for col in data_user if col in data_user_col_map.keys()]

    data_user = data_user[data_user_col]
    data_user = data_user.rename(columns=data_user_col_map)

    data_main_col_map = {
        "created_at": "发布时间",
        "text": "处理内容",
        "source": "评论地点",
        "mid": "mid",
        "total_number": "回复数量",
        "like_counts": "点赞数量",
        "text_raw": "原生内容",
    }

    data_main_col = [col for col in data if col in data_main_col_map.keys()]

    data_main = data[data_main_col]
    data_main = data_main.rename(columns=data_main_col_map)

    data = pd.concat([data_main, data_user], axis=1)
    data["用户主页"] = "https://weibo.com" + data["用户主页"]
    return data