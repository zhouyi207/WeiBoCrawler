# get_comments_level_two.py

import requests
import pandas as pd
import json
from dateutil import parser


def get_buildComments_level_two_response(uid, mid, cookie, the_first=True, max_id=None):
    headers = {
        "accept": "application/json, text/plain, */*",
        "accept-language": "zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6",
        "priority": "u=1, i",
        "sec-ch-ua": '"Not/A)Brand";v="8", "Chromium";v="126", "Microsoft Edge";v="126"',
        "sec-ch-ua-mobile": "?0",
        "sec-ch-ua-platform": '"Windows"',
        "sec-fetch-dest": "empty",
        "sec-fetch-mode": "cors",
        "sec-fetch-site": "same-origin",
        "user-agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0",
        "x-requested-with": "XMLHttpRequest",
        "cookie": cookie,
    }
    params = {
        "is_reload": "1",
        "id": f"{mid}",
        "is_show_bulletin": "2",
        "is_mix": "1",
        "fetch_level": "1",
        "max_id": "0",
        "count": "20",
        "uid": f"{uid}",
        "locale": "zh-CN",
    }

    if not the_first:
        params["flow"] = 0
        params["max_id"] = max_id

    response = requests.get(
        "https://weibo.com/ajax/statuses/buildComments", params=params, headers=headers
    )
    return response


def get_rum_level_two_response(buildComments_url, cookie):
    headers = {
        "accept": "application/json, text/plain, */*",
        "accept-language": "zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6",
        "priority": "u=1, i",
        "sec-ch-ua": '"Not/A)Brand";v="8", "Chromium";v="126", "Microsoft Edge";v="126"',
        "sec-ch-ua-mobile": "?0",
        "sec-ch-ua-platform": '"Windows"',
        "sec-fetch-dest": "empty",
        "sec-fetch-mode": "cors",
        "sec-fetch-site": "same-origin",
        "user-agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0",
        "x-requested-with": "XMLHttpRequest",
        "cookie": cookie,
    }
    entry = {"name": buildComments_url}
    files = {
        "entry": (None, json.dumps(entry)),
        "request_id": (None, ""),
    }
    # 这个resp返回值无实际意义，返回值一直是{ok: 1}
    requests.post("https://weibo.com/ajax/log/rum", headers=headers, files=files)


def get_level_two_response(uid, mid, cookie, the_first=True, max_id=None):
    buildComments_resp = get_buildComments_level_two_response(
        uid, mid, cookie, the_first, max_id
    )
    buildComments_url = buildComments_resp.url
    get_rum_level_two_response(buildComments_url, cookie)
    data = pd.DataFrame(buildComments_resp.json()["data"])
    max_id = buildComments_resp.json()["max_id"]
    return max_id, data


def process_time(publish_time):
    publish_time = parser.parse(publish_time)
    publish_time = publish_time.strftime("%y年%m月%d日 %H:%M")
    return publish_time


def process_data(data):
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
    data["发布时间"] = data["发布时间"].map(process_time)
    data["用户主页"] = "https://weibo.com" + data["用户主页"]
    return data


def get_all_level_two(uid, mid, cookie, max_times=15):
    # 初始化
    max_id = ""
    data_lst = []
    max_times = max_times  # 最多只有15页
    try:
        for current_times in range(1, max_times):
            if current_times == 0:
                max_id, data = get_level_two_response(uid=uid, mid=mid, cookie=cookie)
            else:
                max_id, data = get_level_two_response(
                    uid=uid, mid=mid, cookie=cookie, the_first=False, max_id=max_id
                )
            if data.shape[0] != 0:
                data_lst.append(data)
            if max_id == 0:
                break

        if data_lst:
            data = pd.concat(data_lst).reset_index(drop=True)
            data = process_data(data)
            data.insert(0, "main_body_uid", uid)
            data.insert(0, "comments_level_1_mid", mid)
            return data
        else:
            return pd.DataFrame()

    except Exception as e:
        raise ValueError("解析页面失败，请检查你的cookie是否正确！")