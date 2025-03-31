import httpx
from copy import deepcopy
from typing import Literal, Optional
from datetime import datetime
from .util import request_headers


def build_list_params(search_for: str, page_index: int, *,  kind : Literal["综合", "实时", "高级"] = "综合", 
                      advanced_kind: Literal["综合", "热度", "原创"] = "综合", time_start: Optional[datetime] = None, time_end: Optional[datetime]=None) -> tuple:
    """构建列表页参数

    Args:
        search_for (str): 需要搜索的内容，如果是话题，需要在 search_for 前后都加上 #.
        page_index (int): 页码.
        kind (Literal[, optional): 搜索类型可以是 综合，实时，高级(添加了综合，热度，原创筛选以及时间). Defaults to "综合".
        advanced_kind (Literal[, optional): 筛选条件，可以是综合，热度，原创. Defaults to "综合".
        time_start (Optional[datetime], optional): 起始时间，最大颗粒度为小时. Defaults to None.
        time_end (Optional[datetime], optional): 结束时间，最大颗粒度为小时. Defaults to None.

    Returns:
        httpx.Response: 返回列表页响应
    """
    url_with_params_dic = {
        "综合":{
            "url" : "https://s.weibo.com/weibo",
            "params": {"q": search_for, "Refer": "weibo_weibo", "page": page_index},
        },
        "实时":{
            "url" : "https://s.weibo.com/realtime",
            "params": {"q": search_for, "rd": "realtime", "tw": "realtime", "Refer": "weibo_realtime", "page": page_index},
        },
        "高级":{
            "url" : "https://s.weibo.com/weibo",
            "params": {"q": search_for, "suball": "1", "Refer": "g", "page": page_index},
        },
    }

    url_with_params = url_with_params_dic[kind]
    if kind == "高级":
        if advanced_kind == "综合":
            url_with_params["params"]["typeall"] = "1"
        if advanced_kind == "热度":
            url_with_params["params"]["xsort"] = "hot"
        if advanced_kind == "原创":
            url_with_params["params"]["scope"] = "ori"

        time_start = time_start.strftime("%Y-%m-%d-%H") if time_start else ""
        time_end = time_end.strftime("%Y-%m-%d-%H") if time_end else ""

        url_with_params["params"]["timescope"] = f"custom:{time_start}:{time_end}"

    headers = request_headers.body_headers

    if url_with_params["params"]["page"] > 1:
        referer_url_with_params = deepcopy(url_with_params)
        referer_url_with_params["params"]["page"] = url_with_params["params"]["page"] - 1
        headers["referer"] = str(httpx.URL(url_with_params["url"], params=referer_url_with_params["params"]))

    url = httpx.URL(url=url_with_params["url"], params=url_with_params["params"])
    return url, headers


def get_list_response(search_for: str, page_index: int, *, client: httpx.Client, kind : Literal["综合", "实时", "高级"] = "综合", 
                      advanced_kind: Literal["综合", "热度", "原创"] = "综合", time_start: Optional[datetime] = None, time_end: Optional[datetime]=None) -> httpx.Response:
    """获取列表页响应

    Args:
        search_for (str): 需要搜索的内容，如果是话题，需要在 search_for 前后都加上 #.
        page_index (int): 页码.
        client (httpx.Client): 客户端.
        kind (Literal[, optional): 搜索类型可以是 综合，实时，高级(添加了综合，热度，原创筛选以及时间). Defaults to "综合".
        advanced_kind (Literal[, optional): 筛选条件，可以是综合，热度，原创. Defaults to "综合".
        time_start (Optional[datetime], optional): 起始时间，最大颗粒度为小时. Defaults to None.
        time_end (Optional[datetime], optional): 结束时间，最大颗粒度为小时. Defaults to None.

    Returns:
        httpx.Response: 返回列表页响应
    """
    url, headers = build_list_params(search_for, page_index, kind=kind, advanced_kind=advanced_kind, time_start=time_start, time_end=time_end)
    response = client.get(url, headers=headers)
    return response


async def get_list_response_asyncio(search_for: str, page_index: int, *,  client: httpx.AsyncClient, kind : Literal["综合", "实时", "高级"] = "综合", 
                      advanced_kind: Literal["综合", "热度", "原创"] = "综合", time_start: Optional[datetime] = None, time_end: Optional[datetime] = None) -> httpx.Response:
    """获取列表页响应(异步)

    Args:
        search_for (str): 需要搜索的内容，如果是话题，需要在 search_for 前后都加上 #.
        page_index (int): 页码.
        client (httpx.AsyncClient): 异步客户端.
        kind (Literal[, optional): 搜索类型可以是 综合，实时，高级(添加了综合，热度，原创筛选以及时间). Defaults to "综合".
        advanced_kind (Literal[, optional): 筛选条件，可以是综合，热度，原创. Defaults to "综合".
        time_start (Optional[datetime], optional): 起始时间，最大颗粒度为小时. Defaults to None.
        time_end (Optional[datetime], optional): 结束时间，最大颗粒度为小时. Defaults to None.

    Returns:
        httpx.Response: 返回列表页响应
    """
    url, headers = build_list_params(search_for, page_index, kind=kind, advanced_kind=advanced_kind, time_start=time_start, time_end=time_end)
    response = await client.get(url, headers=headers)
    return response
