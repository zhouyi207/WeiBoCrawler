# get_main_body.py

import requests
from urllib import parse
from utils.parse_html import get_dataframe_from_html_text
import logging
from rich.progress import track
import pandas as pd

logging.basicConfig(level=logging.INFO)


def get_the_main_body_response(q, kind, p, cookie, timescope=None):
    """
    q表示的是话题；
    kind表示的是类别：综合，实时，热门，高级；
    p表示的页码；
    timescope表示高级的时间，不用高级无需带入 example："2024-03-01-0:2024-03-27-16"
    """
    kind_params_url = {
        "综合": [
            {"q": q, "Refer": "weibo_weibo", "page": p},
            "https://s.weibo.com/weibo",
        ],
        "实时": [
            {
                "q": q,
                "rd": "realtime",
                "tw": "realtime",
                "Refer": "realtime_realtime",
                "page": p,
            },
            "https://s.weibo.com/realtime",
        ],
        "热门": [
            {
                "q": q,
                "xsort": "hot",
                "suball": "1",
                "tw": "hotweibo",
                "Refer": "realtime_hot",
                "page": p,
            },
            "https://s.weibo.com/hot",
        ],
        # 高级中的xsort删除后就是普通的排序
        "高级": [
            {
                "q": q,
                "xsort": "hot",
                "suball": "1",
                "timescope": f"custom:{timescope}",
                "Refer": "g",
                "page": p,
            },
            "https://s.weibo.com/weibo",
        ],
    }

    params, url = kind_params_url[kind]

    headers = {
        "authority": "s.weibo.com",
        "accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
        "accept-language": "zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6",
        "referer": url
        + "?"
        + parse.urlencode(params).replace(
            f'&page={params["page"]}', f'&page={int(params["page"]) - 1}'
        ),
        "sec-ch-ua": '"Chromium";v="116", "Not)A;Brand";v="24", "Microsoft Edge";v="116"',
        "sec-ch-ua-mobile": "?0",
        "sec-ch-ua-platform": '"Windows"',
        "sec-fetch-dest": "document",
        "sec-fetch-mode": "navigate",
        "sec-fetch-site": "same-origin",
        "cookie": cookie,
        "sec-fetch-user": "?1",
        "upgrade-insecure-requests": "1",
        "user-agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.0.0 Safari/537.36 Edg/116.0.1938.69",
    }
    response = requests.get(url, params=params, headers=headers)
    return response


def get_all_main_body(q, kind, cookie, timescope=None):
    # 初始化数据
    data_list = []

    resp = get_the_main_body_response(q, kind, 1, cookie, timescope)
    html_text = resp.text
    try:
        data, total_page = get_dataframe_from_html_text(html_text)
        data_list.append(data)
        logging.info(
            f"话题：{q}，类型：{kind}，解析成功，一共有{total_page:2d}页，准备开始解析..."
        )

        for current_page in track(range(2, total_page + 1), description=f"解析中..."):
            html_text = get_the_main_body_response(
                q, kind, current_page, cookie, timescope
            ).text
            data, total_page = get_dataframe_from_html_text(html_text)
            data_list.append(data)

        data = pd.concat(data_list).reset_index(drop=True)

        logging.info(f"话题：{q}，类型：{kind}，一共有{total_page:2d}页，已经解析完毕！")

        return data
    except Exception as e:
        logging.warning("解析页面失败，请检查你的cookie是否正确！")
        raise ValueError("解析页面失败，请检查你的cookie是否正确！")